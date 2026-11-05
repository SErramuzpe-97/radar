import { useEffect, useState } from "preact/hooks";
import { cargarFuentes, crearFuente, cambiarFuente, lanzarRastreo, resolverCandidata, fusionar, descartarDuplicado, fuentes, resumenFiltro, candidatas, duplicados, cobertura, errorFuentes, type FuenteCandidata, type Duplicado } from "../stores/fuentes";
import type { components } from "../api/schema";

type FormFuente = components["schemas"]["NuevaFuente"];
const fechaHoy = () => new Date().toLocaleDateString("en-CA");
const inicial: FormFuente = { nombre:"", tipo:"semilla", url:"", condiciones:"", atribucion:"", revisada_en:fechaHoy(), profundidad:2, max_paginas_dia:100, habilitada:true };

export function PanelFuentes() {
  const [form, setForm] = useState<FormFuente>(inicial);
  const [mensaje, setMensaje] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [candidataEditando, setCandidataEditando] = useState<number | null>(null);
  useEffect(() => { void cargarFuentes(); }, []);
  const cambiar = (key: keyof FormFuente, value: string | number | boolean) => setForm({ ...form, [key]: value });
  const error = (e: unknown) => setMensaje(String(e));

  const aprobarCandidata = (c: FuenteCandidata) => {
    setForm({ ...inicial, nombre:c.nombre, tipo:"semilla", url:c.url, condiciones:"", atribucion:"", habilitada:false });
    setCandidataEditando(c.id);
    setMensaje("Completa las condiciones y atribución, y guarda la fuente para aprobarla.");
  };

  return <section class="panel-fuentes">
    <div class="kpi-cabecera"><div><h1>Fuentes y cobertura</h1><p class="muted">Rastrea solo fuentes revisadas. Se respeta robots.txt y cada fuente tiene un límite diario de páginas.</p></div><button class="secundario" onClick={() => void cargarFuentes()}>Actualizar</button></div>
    {(mensaje || errorFuentes.value) && <p class="error">{mensaje ?? errorFuentes.value}</p>}
    {resumenFiltro.value && <section class="kpi-panel"><h2>Calidad de los resultados</h2>
      <p>{resumenFiltro.value.examinados} candidatos examinados · <strong>{resumenFiltro.value.admitidos} admitidos</strong> · {resumenFiltro.value.excluidos} excluidos · {resumenFiltro.value.pendientes} pendientes de evidencia</p>
      <p class="muted">El filtro comprueba actividad comercial, ubicación y vínculos de organización. Los casos sin evidencia suficiente quedan fuera de tabla y mapa.</p>
      <ul>{resumenFiltro.value.motivos.map(m => <li key={m.etiqueta}>{({sin_actividad_comercial_demostrada:"Actividad comercial no demostrada",ubicacion_navarra_no_demostrada:"Ubicación en Navarra no demostrada",actividad_cerrada_o_abandonada:"Actividad cerrada o abandonada",red_comercial_grande_con_presencia_corporativa_central:"Gran red con presencia corporativa centralizada",autonomia_comercial_no_demostrada:"Falta evidencia de autonomía comercial",posible_vinculo_corporativo_sin_confirmar:"Posible vínculo corporativo pendiente de contraste",grupo_pequeno_con_dimension_documentada:"Grupo pequeño con dimensión documentada",actividad_local_sin_vinculo_corporativo_detectado:"Actividad local sin vínculo corporativo detectado"} as Record<string,string>)[m.etiqueta] ?? m.etiqueta}: <strong>{m.total}</strong></li>)}</ul>
    </section>}
    <div class="fuentes-grid">
      <section class="kpi-panel"><h2>Fuentes registradas</h2>
        {fuentes.value.map((f) => <article class="fuente-item" key={f.id}>
          <div><strong>{f.nombre}</strong><p class="muted"><a href={f.url.replace("{query}", "negocios+Navarra")} target="_blank" rel="noreferrer">{f.url}</a></p>
            <small>{f.tipo} · {f.max_paginas_dia} páginas/día · revisada {f.revisada_en ?? "sin fecha"}</small>
            {f.atribucion&&<small class="fuente-atribucion">Atribución: {f.atribucion}</small>}
            {f.condiciones&&<small class="fuente-atribucion">Licencia / condiciones: {f.condiciones}</small>}
            {f.error_ultima_ejecucion && <p class="error">{f.error_ultima_ejecucion}</p>}
          </div>
          <div class="acciones"><button disabled={!f.habilitada || busy} onClick={async () => { setBusy(true); try { await lanzarRastreo(f.id); setMensaje("Rastreo añadido a trabajos."); } catch(e) { error(e); } finally { setBusy(false); } }}>Rastrear</button>
            <button class="secundario" onClick={async () => { try { await cambiarFuente(f.id, !f.habilitada); } catch(e) { error(e); } }}>{f.habilitada ? "Deshabilitar" : "Habilitar"}</button></div>
        </article>)}
      </section>
      <section class="kpi-panel"><h2>Añadir fuente pública</h2>
        <div class="form-fuente">
          <input value={form.nombre} placeholder="Nombre de la fuente" onInput={e => cambiar("nombre",(e.target as HTMLInputElement).value)} />
          <select value={form.tipo} onChange={e => cambiar("tipo",(e.target as HTMLSelectElement).value)}>
            <option value="semilla">Portal / directorio</option><option value="dataset">Dataset</option><option value="busqueda">Buscador gratuito</option>
          </select>
          <input value={form.url} placeholder={form.tipo === "busqueda" ? "https://proveedor/buscar?q={query}" : "https://dominio-publico/..."} onInput={e => cambiar("url",(e.target as HTMLInputElement).value)} />
          {form.tipo === "busqueda" && <p class="muted">La URL debe incluir <code>{"{query}"}</code>. Los dominios encontrados quedan pendientes de revisión.</p>}
          <textarea rows={2} value={form.condiciones} placeholder="Condiciones/licencia consultadas" onInput={e => cambiar("condiciones",(e.target as HTMLTextAreaElement).value)} />
          <input value={form.atribucion} placeholder="Atribución requerida" onInput={e => cambiar("atribucion",(e.target as HTMLInputElement).value)} />
          <label>Revisada el <input type="date" value={form.revisada_en} onInput={e => cambiar("revisada_en",(e.target as HTMLInputElement).value)} /></label>
          <label>Profundidad <input type="number" min={0} max={4} value={form.profundidad} onInput={e => cambiar("profundidad",Number((e.target as HTMLInputElement).value))} /></label>
          <label>Máximo páginas/día <input type="number" min={1} max={1000} value={form.max_paginas_dia} onInput={e => cambiar("max_paginas_dia",Number((e.target as HTMLInputElement).value))} /></label>
          <button disabled={busy} onClick={async () => { setBusy(true); try { await crearFuente(form); if(candidataEditando!==null) await resolverCandidata(candidataEditando,"aprobada"); setCandidataEditando(null); setForm(inicial); setMensaje("Fuente guardada."); } catch(e) { error(e); } finally { setBusy(false); } }}>{candidataEditando===null?"Guardar fuente":"Guardar y aprobar dominio"}</button>
        </div>
      </section>
    </div>
    <section class="kpi-panel"><h2>Dominios candidatos por revisar</h2>
      {candidatas.value.length === 0 ? <p class="muted">No hay candidatos pendientes.</p> : candidatas.value.map((c) => <div class="fuente-item" key={c.id}>
        <div><strong>{c.nombre}</strong><p><a href={c.url} target="_blank" rel="noreferrer">{c.url}</a></p><small>Descubierto {c.creada.slice(0,10)}</small></div>
        <div class="acciones"><button onClick={() => aprobarCandidata(c)}>Revisar como fuente</button><button class="secundario" onClick={() => void resolverCandidata(c.id,"descartada")}>Descartar</button></div>
      </div>)}
    </section>
    <section class="kpi-panel"><h2>Posibles fichas duplicadas</h2>
      {duplicados.value.length === 0 ? <p class="muted">No hay coincidencias pendientes.</p> : duplicados.value.map((d) => <FilaDuplicado key={d.id} d={d} onError={error} />)}
    </section>
    <section class="kpi-panel"><h2>Registros conocidos por municipio</h2><p class="muted">Indicador de distribución de las fuentes activas, no un censo del tejido empresarial.</p>
      <div class="coverage-list">{cobertura.value.slice(0,30).map(c => <span key={c.municipio}>{c.municipio}: <strong>{c.negocios}</strong> · auditados {c.auditados}</span>)}</div>
    </section>
  </section>;
}

function FilaDuplicado({ d, onError }: { d: Duplicado; onError: (e: unknown) => void }) {
  return <div class="fuente-item"><div><strong>{d.nombre_a}</strong> ↔ <strong>{d.nombre_b}</strong><p class="muted">{d.razon}</p></div>
    <div class="acciones"><button onClick={() => void fusionar(d,"a").catch(onError)}>Mantener A</button><button onClick={() => void fusionar(d,"b").catch(onError)}>Mantener B</button><button class="secundario" onClick={() => void descartarDuplicado(d.id).catch(onError)}>No son duplicados</button></div>
  </div>;
}
