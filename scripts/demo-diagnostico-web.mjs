/**
 * Prueba de viabilidad local: Puppeteer + axe-core, con HTML de demostración.
 * No visita webs de clientes ni modifica la base de Radar.
 * Ejecutar como usuario sin privilegios con acceso a Chrome y al runtime del worker.
 */
import fs from 'node:fs/promises';
import path from 'node:path';
import {createRequire} from 'node:module';
import os from 'node:os';

const runtime = process.env.RADAR_DEMO_RUNTIME || '/opt/radar-audit';
const requireRuntime = createRequire(path.join(runtime, 'package.json'));
const puppeteer = requireRuntime('puppeteer-core');
const axe = requireRuntime('axe-core');
const outputIndex = process.argv.indexOf('--output');
if (outputIndex >= 0 && !process.argv[outputIndex + 1]) throw new Error('Indica la carpeta después de --output');
const output = outputIndex >= 0
  ? path.resolve(process.argv[outputIndex + 1])
  : await fs.mkdtemp(path.join(os.tmpdir(), 'radar-diagnostico-demo-'));
await fs.mkdir(output, {recursive: true, mode: 0o700});
const chromePath = process.env.RADAR_DEMO_CHROME || path.join(runtime, 'chrome/chrome-linux64/chrome');
const escape = value => String(value ?? '').replace(/[&<>"']/g, char => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));

function fixture(fixed) {
  return `<!doctype html><html lang="es"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Demostración local de diagnóstico</title>
  <style>*{box-sizing:border-box}body{margin:0;background:#eef2f8;color:#17243a;font:17px/1.6 system-ui}header{padding:20px 24px;background:#102b4c;color:white}header p{margin:0}main{margin:24px 16px;padding:28px;width:1040px;background:white;border-radius:16px}h1{font-size:32px;line-height:1.2}h2{font-size:22px}.fila{display:flex;align-items:end;justify-content:space-between;gap:24px}.campo{display:flex;flex-direction:column}input{font:inherit;padding:12px;width:290px;max-width:100%;border:1px solid #54657d;border-radius:6px}button{width:50px;height:50px;border:0;border-radius:8px;background:#1749b8;color:white}svg{width:24px;height:24px}a{color:#1643a4}.nota{border-left:4px solid #1749b8;padding-left:16px} ${fixed ? 'main{width:calc(100% - 32px);max-width:1040px}.fila{flex-wrap:wrap}.campo{max-width:100%}h1{font-size:clamp(24px,5vw,32px)}' : ''}</style></head>
  <body><header><p>Radar · Demostración técnica local</p></header><main><h1>Una web que facilite el contacto</h1><p>Esta página es ficticia. Sirve para comprobar el análisis y no representa a ningún negocio.</p><p><a href="#contacto">Ir al bloque de contacto</a></p><section id="contacto"><h2>Solicita información</h2><div class="fila"><div class="campo">${fixed ? '<label for="correo">Correo electrónico</label>' : '<span>Correo electrónico</span>'}<input id="correo" type="email" autocomplete="email"></div><button type="button" ${fixed ? 'aria-label="Solicitar información"' : ''}><svg aria-hidden="true" viewBox="0 0 24 24"><path fill="currentColor" d="M3 11h13l-5-5 1.5-1.5L20 12l-7.5 7.5L11 18l5-5H3z"/></svg></button></div><p class="nota">La versión ${fixed ? 'corregida adapta el ancho y añade nombres accesibles.' : 'inicial tiene un ancho fijo y controles sin nombre accesible.'}</p></section></main></body></html>`;
}

const browser = await puppeteer.launch({
  executablePath: chromePath, headless: true,
  args: ['--disable-dev-shm-usage', '--renderer-process-limit=1', '--no-first-run'],
});
const checks = [];
try {
  for (const fixed of [false, true]) {
    for (const width of [390, 1366]) {
      const page = await browser.newPage();
      try {
        // La demostración no necesita red; las peticiones externas quedan bloqueadas.
        await page.setRequestInterception(true);
        page.on('request', request => {
          void (/^(data:|about:)/.test(request.url()) ? request.continue() : request.abort());
        });
        await page.setViewport({width, height: 844, deviceScaleFactor: 1, isMobile: width < 600, hasTouch: width < 600});
        await page.setContent(fixture(fixed), {waitUntil: 'load'});
        const geometry = await page.evaluate(() => ({
          viewport: document.documentElement.clientWidth,
          inner_width: window.innerWidth,
          content: Math.max(document.documentElement.scrollWidth, document.body.scrollWidth),
          overflow: Math.max(document.documentElement.scrollWidth, document.body.scrollWidth) > document.documentElement.clientWidth + 2,
        }));
        await page.evaluate(axe.source);
        const accessibility = await page.evaluate(async () => {
          const results = await axe.run(document, {runOnly: {type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa']}});
          return {
            violations: results.violations.map(rule => ({code: rule.id, impact: rule.impact, title: rule.help, references: rule.tags, help_url: rule.helpUrl, nodes: rule.nodes.map(node => ({selector: node.target, html: node.html, explanation: node.failureSummary}))})),
            incomplete_rules: results.incomplete.map(rule => rule.id),
            passed_rules: results.passes.length,
          };
        });
        const name = `${fixed ? 'corregida' : 'inicial'}-${width}.png`;
        await page.screenshot({path: path.join(output, name), fullPage: false});
        checks.push({fixture: fixed ? 'corregida' : 'inicial', width, geometry, accessibility, screenshot: name});
      } finally { await page.close(); }
    }
  }
  // Son las condiciones que la prueba de viabilidad pretende demostrar.
  const initial = checks.find(check => check.fixture === 'inicial' && check.width === 390);
  const corrected = checks.find(check => check.fixture === 'corregida' && check.width === 390);
  const initialDesktop = checks.find(check => check.fixture === 'inicial' && check.width === 1366);
  if (!initial.geometry.overflow || corrected.geometry.overflow || initialDesktop.geometry.overflow) throw new Error('El caso de adaptación móvil no produjo el resultado esperado');
  for (const code of ['label', 'button-name']) {
    if (!initial.accessibility.violations.some(rule => rule.code === code) || corrected.accessibility.violations.some(rule => rule.code === code)) throw new Error(`No se pudo contrastar el caso de accesibilidad ${code}`);
  }
  const metadata = {
    demonstration: true,
    measured_at: new Date().toISOString(),
    source: 'HTML ficticio incorporado en scripts/demo-diagnostico-web.mjs; sin visitas a webs externas',
    environment: {node: process.version, chrome: await browser.version(), axe: axe.version, puppeteer: requireRuntime('puppeteer-core/package.json').version},
    checks,
    limitations: ['Comprobaciones automáticas de una página de demostración en Chromium. No certifican accesibilidad.', 'La versión corregida es una fixture programada, no una propuesta aplicada a una web real.', 'No se han medido SEO, rendimiento ni entrega de formularios.'],
  };
  await fs.writeFile(path.join(output, 'resultado.json'), JSON.stringify(metadata, null, 2));
  const sections = [];
  for (const check of checks) {
    const png = await fs.readFile(path.join(output, check.screenshot));
    const violations = check.accessibility.violations;
    sections.push(`<section><h2>${check.fixture === 'inicial' ? 'Versión inicial' : 'Versión corregida'} · ${check.width} px</h2><p>Contenido: <strong>${check.geometry.content} px</strong> · viewport: <strong>${check.geometry.viewport} px</strong>. ${check.geometry.overflow ? 'Se observa desbordamiento horizontal.' : 'Sin desbordamiento horizontal en esta medición.'}</p><p>${violations.length} reglas de accesibilidad con incidencias. ${check.accessibility.incomplete_rules.length} reglas necesitan revisión adicional.</p>${violations.length ? `<ul>${violations.map(rule => `<li><strong>${escape(rule.code)}</strong>: ${escape(rule.title)}. ${rule.nodes.length} elementos. Selector: <code>${escape(rule.nodes[0]?.selector.join(' '))}</code></li>`).join('')}</ul>` : '<p>No se han detectado infracciones en las reglas y el estado analizados. Queda pendiente la revisión manual.</p>'}<img alt="Captura de la página ficticia, versión ${check.fixture}, ancho ${check.width} píxeles" src="data:image/png;base64,${png.toString('base64')}" /></section>`);
  }
  const html = `<!doctype html><html lang="es"><head><meta charset="utf-8"><title>Radar · Demostración del diagnóstico web</title><style>body{max-width:1050px;margin:32px auto;padding:0 24px;color:#18283f;font:16px/1.55 system-ui}h1{font-size:30px}.aviso{padding:16px;border-left:5px solid #b36509;background:#fff5df}section{margin:28px 0;padding-top:16px;border-top:1px solid #ccd6e3}img{display:block;max-width:100%;max-height:650px;object-fit:contain;object-position:left top;border:1px solid #ccd6e3}code{overflow-wrap:anywhere}footer{font-size:12px;color:#40516b}@media print{body{margin:0;font-size:12px}section{break-before:page}img{max-height:550px}h1{font-size:24px}}</style></head><body><h1>Diagnóstico web · Prueba de viabilidad</h1><p class="aviso"><strong>Demostración con una página ficticia.</strong> No es una auditoría de un cliente. Las medidas y capturas corresponden al HTML local de esta prueba.</p><p>Se contrasta un diseño de ancho fijo con una versión adaptable y controles con nombres accesibles. El botón es inerte: no se envía ningún formulario.</p><p>Fecha: ${escape(metadata.measured_at)} · ${escape(metadata.environment.chrome)} · axe-core ${escape(axe.version)}.</p>${sections.join('')}<footer>${metadata.limitations.map(escape).join('<br>')}</footer></body></html>`;
  await fs.writeFile(path.join(output, 'diagnostico-demo.html'), html);
  const pdfPage = await browser.newPage();
  try {
    await pdfPage.setRequestInterception(true);
    pdfPage.on('request', request => { void (/^(data:|about:)/.test(request.url()) ? request.continue() : request.abort()); });
    await pdfPage.setContent(html, {waitUntil: 'load'});
    await pdfPage.pdf({path: path.join(output, 'diagnostico-demo.pdf'), format: 'A4', printBackground: true, margin: {top: '15mm', right: '14mm', bottom: '15mm', left: '14mm'}});
  } finally { await pdfPage.close(); }
  console.log(JSON.stringify({output, demonstration: true, checks: checks.map(check => ({fixture: check.fixture, width: check.width, horizontal_overflow: check.geometry.overflow, accessibility_rules_failed: check.accessibility.violations.map(rule => rule.code)}))}, null, 2));
} finally { await browser.close(); }
