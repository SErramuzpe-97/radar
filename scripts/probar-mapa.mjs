/**
 * Regresión de filtros y mapa con 55.000 empresas en una base temporal independiente.
 * Requiere un binario release recién construido y Chrome/Puppeteer del worker.
 * No envía emails ni consulta mapas o webs externos.
 * Uso: node scripts/probar-mapa.mjs
 */
import assert from 'node:assert/strict';
import {execFileSync, spawn} from 'node:child_process';
import {once} from 'node:events';
import fs from 'node:fs/promises';
import {createRequire} from 'node:module';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {setTimeout as esperar} from 'node:timers/promises';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bin = process.env.RADAR_MAPA_TEST_BIN || path.join(root, 'target/release/radar-cli');
const runtime = process.env.RADAR_DEMO_RUNTIME || '/opt/radar-audit';
const requireRuntime = createRequire(path.join(runtime, 'package.json'));
const puppeteer = requireRuntime('puppeteer-core');
const temporal = await fs.mkdtemp(path.join(os.tmpdir(), 'radar-mapa-'));
const db = path.join(temporal, 'prueba.db');
let servidor;
let browser;
let logServidor = '';
try {
  execFileSync(bin, ['--db', db, 'targets'], {stdio: 'pipe'});
  execFileSync('python3', ['-c', `
import sqlite3,sys,hashlib,datetime
c=sqlite3.connect(sys.argv[1])
ahora=datetime.datetime.now(datetime.timezone.utc)
fecha=ahora.isoformat()
c.executemany("INSERT INTO negocios(nombre,municipio,sector,rubro,lat,lon,fuente,fuente_id,primera_vista) VALUES(?,?,?,'alimentacion',?,?,'test',?,?)",[
    (f'Empresa {i:05}', 'Pamplona', 'comercio',
     42.8 if i<55 else 42.0+(i//250)*0.005,
     -1.6 if i<55 else -2.5+(i%250)*0.006, str(i),fecha) for i in range(55000)])
c.execute("INSERT INTO web_estado(negocio_id,estado) SELECT id,'pendiente' FROM negocios")
c.execute("INSERT INTO oportunidades_servicio(negocio_id,servicio,score,motivo,fecha) SELECT id,'creacion_web',80,'Prueba',? FROM negocios",(fecha,))
# Mil trabajos terminados y sus informes: la antigua consulta los recorría por cada empresa.
# No hay trabajos pendientes que puedan activar el worker o visitas externas.
c.execute("UPDATE web_estado SET estado='confirmada',origen='confirmacion_usuario',url_confirmada='https://empresa-'||negocio_id||'.example/' WHERE negocio_id<=1000")
c.execute("INSERT INTO jobs(id,tipo,estado,params,creado,actualizado) SELECT 'audit-'||id,'technical','terminado',json_object('negocio_id',id,'url','https://empresa-'||id||'.example/'),?,? FROM negocios WHERE id<=1000",(fecha,fecha))
c.execute("INSERT INTO auditorias_tecnicas(id,job_id,negocio_id,url,motor,dispositivo,estado,fecha) SELECT 'report-'||id,'audit-'||id,id,'https://empresa-'||id||'.example/','lighthouse','mobile',CASE WHEN id%2=0 THEN 'completo' ELSE 'error' END,? FROM negocios WHERE id<=1000",(fecha,))
c.execute("INSERT INTO auth_codes(email,hash,creado,expira) VALUES(?,?,?,?)",('mapa-test@example.com',hashlib.sha256(b'123456').hexdigest(),fecha,(ahora+datetime.timedelta(minutes=10)).isoformat()))
c.commit()
`, db]);
  const puertoLibre = net.createServer();
  puertoLibre.listen(0, '127.0.0.1');
  await once(puertoLibre, 'listening');
  const puerto = puertoLibre.address().port;
  await new Promise(resolve => puertoLibre.close(resolve));
  const origen = `http://127.0.0.1:${puerto}`;
  servidor = spawn(bin, ['--db', db, 'serve', '--puerto', String(puerto)], {
    env: {...process.env, RADAR_AUTH_EMAILS: 'mapa-test@example.com'},
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  for (const stream of [servidor.stdout, servidor.stderr]) stream.on('data', data => { logServidor = (logServidor + data).slice(-8000); });
  let listo = false;
  for (let i = 0; i < 600; i++) {
    if (servidor.exitCode !== null || servidor.signalCode !== null) throw new Error(`Servidor terminado: ${logServidor}`);
    try { listo = (await fetch(`${origen}/openapi.json`)).ok; } catch {}
    if (listo) break;
    await esperar(100);
  }
  assert.ok(listo, `El servidor no arrancó: ${logServidor}`);
  const sesion = await fetch(`${origen}/api/auth/verify`, {
    method: 'POST', headers: {'content-type': 'application/json'},
    body: JSON.stringify({email: 'mapa-test@example.com', code: '123456'}),
  });
  assert.equal(sesion.status, 200);
  const cookie = sesion.headers.get('set-cookie').split(';')[0];
  const inicio = performance.now();
  const respuesta = await fetch(`${origen}/api/negocios/mapa?sur=41.9&oeste=-2.6&norte=43.3&este=-0.8&zoom=19`, {headers: {cookie}});
  const texto = await respuesta.text();
  const mapa = JSON.parse(texto);
  const tiempoApi = Math.round(performance.now() - inicio);
  assert.equal(respuesta.status, 200);
  assert.equal(mapa.total, 55000);
  assert.equal(mapa.items.reduce((n, p) => n + p.cantidad, 0), 55000);
  assert.ok(mapa.items.length <= 825);
  assert.ok(Buffer.byteLength(texto) < 400000);

  const tiemposFiltros = [];
  for (const [estado, total] of [['realizada',500], ['pendiente',54000], ['error',500], ['en_curso',0]]) {
    const inicioFiltro = performance.now();
    const r = await fetch(`${origen}/api/negocios?estado_auditoria_tecnica=${estado}&tam=50`, {headers: {cookie}});
    assert.equal(r.status, 200);
    const datos = await r.json();
    const ms = Math.round(performance.now() - inicioFiltro);
    assert.equal(datos.total, total);
    assert.ok(datos.items.length <= 50);
    assert.ok(datos.items.every(n => n.estado_auditoria_tecnica === estado));
    assert.ok(ms < 2000, `Filtro ${estado} demasiado lento: ${ms} ms`);
    tiemposFiltros.push({estado, total, ms});
  }

  browser = await puppeteer.launch({
    executablePath: process.env.RADAR_DEMO_CHROME || path.join(runtime, 'chrome/chrome-linux64/chrome'),
    headless: true,
    args: ['--no-sandbox', '--disable-dev-shm-usage', '--no-first-run'],
  });
  const resultados = [];
  for (const width of [1440, 390]) {
    console.log(`Comprobando mapa a ${width}px…`);
    const page = await browser.newPage();
    const errores = [];
    const consultas = [];
    const consultasLista = [];
    let retrasarProximaLista = false;
    page.on('pageerror', err => errores.push(String(err)));
    page.on('request', request => {
      const url = new URL(request.url());
      if (url.pathname === '/api/negocios/mapa') consultas.push(url);
      if (url.pathname === '/api/negocios') consultasLista.push(url);
    });
    await page.setRequestInterception(true);
    page.on('request', request => {
      if (new URL(request.url()).pathname === '/api/negocios' && retrasarProximaLista) {
        retrasarProximaLista = false;
        void esperar(500).then(() => request.continue()).catch(() => {});
      }
      else if (request.url().startsWith(origen)) void request.continue();
      else void request.abort();
    });
    await page.setViewport({width, height: 1000, deviceScaleFactor: 1});
    const [nombreCookie, valorCookie] = cookie.split('=');
    await browser.setCookie({name: nombreCookie, value: valorCookie, domain: '127.0.0.1', path: '/', httpOnly: true});
    await page.goto(origen, {waitUntil: 'networkidle0'});
    await page.waitForSelector('.app');
    assert.equal(consultas.length, 0, 'La tabla no debe descargar el mapa');
    const filtroAuditoria = 'select[aria-label="Filtrar estado de auditoría"]';
    const consultaAuditadas = page.waitForResponse(r => new URL(r.url()).pathname === '/api/negocios' && new URL(r.url()).searchParams.get('estado_auditoria_tecnica') === 'realizada');
    await page.select(filtroAuditoria, 'realizada');
    assert.equal((await consultaAuditadas).status(), 200);
    await page.waitForFunction(() => document.querySelector('.tabla .paginacion')?.textContent.includes('(500 negocios)') && !document.querySelector('.tabla-estado'));
    assert.ok(await page.$$eval('.tabla tbody tr', filas => filas.length > 0 && filas.length <= 50 && filas.every(f => f.textContent.includes('Auditada'))));
    const peticionesAntesDeEscribir = consultasLista.length;
    const busqueda = 'input[placeholder="Buscar nombre…"]';
    await page.type(busqueda, 'Empresa 00001', {delay: 15});
    await page.waitForFunction(() => document.querySelector('.tabla .paginacion')?.textContent.includes('(1 negocios)') && !document.querySelector('.tabla-estado'));
    assert.equal(consultasLista.length - peticionesAntesDeEscribir, 1, 'Escribir agrupa las pulsaciones en una sola petición');
    await page.focus(busqueda);
    await page.keyboard.down('Control');
    await page.keyboard.press('A');
    await page.keyboard.up('Control');
    await page.keyboard.press('Backspace');
    await page.select(filtroAuditoria, '');
    await page.waitForFunction(() => document.querySelector('.tabla .paginacion')?.textContent.includes('(55000 negocios)') && !document.querySelector('.tabla-estado'));
    // Retener una petición anterior y comprobar que un filtro nuevo puede terminar
    // y permanecer visible sin esperar ni ser sustituido por la respuesta antigua.
    retrasarProximaLista = true;
    const retenida = page.waitForRequest(r => new URL(r.url()).pathname === '/api/negocios' && new URL(r.url()).searchParams.get('estado_auditoria_tecnica') === 'realizada');
    await page.select(filtroAuditoria, 'realizada');
    await retenida;
    await page.select(filtroAuditoria, 'error');
    await page.waitForFunction(() => !document.querySelector('.tabla-estado') && [...document.querySelectorAll('.tabla tbody tr')].length > 0 && [...document.querySelectorAll('.tabla tbody tr')].every(f => f.textContent.includes('Error de auditoría')));
    await esperar(600);
    assert.ok(await page.$$eval('.tabla tbody tr', filas => filas.every(f => f.textContent.includes('Error de auditoría'))));
    assert.equal(await page.$$eval('.tabla .error', errores => errores.length), 0);
    await page.select(filtroAuditoria, '');
    await page.waitForFunction(() => document.querySelector('.tabla .paginacion')?.textContent.includes('(55000 negocios)') && !document.querySelector('.tabla-estado'));
    await page.select('.nav-compacta', 'mapa');
    await page.waitForSelector('.radar-cluster');
    await page.waitForFunction(() => document.querySelector('.mapa-cuenta')?.textContent.includes('55.000'));
    const marcadores = await page.$$eval('.leaflet-marker-icon', elementos => elementos.length);
    assert.ok(marcadores > 0 && marcadores <= 825);
    assert.ok(consultas.every(q => ['sur', 'oeste', 'norte', 'este', 'zoom'].every(p => q.searchParams.has(p))));
    const zonaInicial = consultas.at(-1).search;
    const marcadorAnterior = await page.$('.radar-cluster');
    const actualizado = page.waitForResponse(r => r.url().includes('/api/negocios/mapa?') && r.status() === 200);
    await page.click('.leaflet-control-zoom-in');
    await actualizado;
    await page.waitForFunction(el => !el.isConnected, {}, marcadorAnterior);
    await marcadorAnterior.dispose();
    await page.waitForFunction(() => !document.querySelector('.leaflet-zoom-anim'));
    assert.notEqual(consultas.at(-1).search, zonaInicial, 'El zoom solicita una nueva zona');
    await page.waitForSelector('.radar-cluster');
    const acercado = page.waitForResponse(r => r.url().includes('/api/negocios/mapa?') && r.status() === 200);
    await page.$eval('.radar-cluster', el => el.click());
    await acercado;
    await page.waitForSelector('.mapa-zona-lista button');
    assert.ok(await page.$$eval('.mapa-zona-lista button', elementos => elementos.length <= 20));
    const primeraPagina = await page.$eval('.mapa-zona-lista', el => el.textContent);
    await page.$$eval('.mapa-empresas-zona button', botones => botones.find(b => b.textContent === 'Siguiente').click());
    await page.waitForFunction(() => document.querySelector('.mapa-empresas-zona .paginacion')?.textContent.includes('Página 2'));
    await page.waitForFunction(anterior => { const lista = document.querySelector('.mapa-zona-lista'); return lista && lista.textContent !== anterior; }, {}, primeraPagina);
    await page.click('.mapa-zona-lista button');
    await page.waitForSelector('.ficha-abierta h2');
    // Cerrar la ficha antes de filtrar en la vista móvil, donde ocupa la pantalla.
    await page.$eval('.cerrar-ficha', el => el.click());
    const campo = 'input[aria-label="Filtrar municipios del mapa"]';
    await page.type(campo, 'Municipio inexistente', {delay: 15});
    await page.waitForFunction(() => document.querySelector('.mapa-cuenta')?.textContent.startsWith('0 '));
    assert.equal(await page.$$eval('.leaflet-marker-icon', elementos => elementos.length), 0);
    await page.select('.nav-compacta', 'tabla');
    const consultasAlSalir = consultas.length;
    await esperar(2200);
    assert.equal(consultas.length, consultasAlSalir, 'Al salir se detiene la carga del mapa');
    assert.deepEqual(errores, []);
    resultados.push({ancho: width, marcadores, consultas: consultas.length, filtros: 'ok', busquedaAgrupada: 'ok', cancelacion: 'ok', paginacion: 'ok', ficha: 'ok', errores: 0});
    await page.close();
  }
  console.log(JSON.stringify({empresas: mapa.total, elementosApi: mapa.items.length, bytes: Buffer.byteLength(texto), tiempoApiMs: tiempoApi, filtrosApi: tiemposFiltros, navegador: resultados}, null, 2));
} catch (error) {
  if (browser) {
    for (const page of await browser.pages()) {
      if (page.url() === 'about:blank') continue;
      console.error(await page.evaluate(() => document.body.innerText));
      await fs.mkdir(path.join(root, 'tmp'), {recursive: true});
      await page.screenshot({path: path.join(root, 'tmp/mapa-regresion-error.png')});
    }
  }
  throw error;
} finally {
  if (browser) await browser.close();
  if (servidor && servidor.exitCode === null && servidor.signalCode === null) {
    servidor.kill('SIGTERM');
    await once(servidor, 'exit');
  }
  await fs.rm(temporal, {recursive: true, force: true});
}
