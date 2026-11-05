import { useEffect, useState } from "preact/hooks";
import { cargarTareas, cargarPlantillas, crearPlantilla, tareasPendientes, plantillasContacto } from "../stores/crm";
import { cargarDetalle } from "../stores/negocios";

export function PanelSeguimiento({ abrirNegocio }: { abrirNegocio: () => void }) {
  const [nombre,setNombre]=useState("");
  const [canal,setCanal]=useState("email");
  const [asunto,setAsunto]=useState("");
  const [cuerpo,setCuerpo]=useState("Hola {{nombre}},\\n\\nTe escribo para…");
  const [mensaje,setMensaje]=useState<string|null>(null);
  const [desde,setDesde]=useState(""); const [hasta,setHasta]=useState("");
  const [prioridad,setPrioridad]=useState(""); const [servicio,setServicio]=useState(""); const [estado,setEstado]=useState("");
  useEffect(()=>{void Promise.all([cargarTareas(),cargarPlantillas()]);},[]);
  const tareas=tareasPendientes.value.filter(t=>(!desde||t.fecha>=desde)&&(!hasta||t.fecha<=hasta)&&(!prioridad||t.prioridad===prioridad)&&(!estado||t.estado===estado)&&(!servicio||(t.servicios??[]).includes(servicio)));
  return <section class="panel-kpis">
    <div class="kpi-cabecera"><div><h1>Seguimiento comercial</h1><p class="muted">Filtra las próximas acciones por fecha, prioridad, servicio y estado. Se editan desde cada ficha.</p></div><button class="secundario" onClick={()=>void Promise.all([cargarTareas(),cargarPlantillas()])}>Actualizar</button></div>
    {mensaje&&<p class="muted">{mensaje}</p>}
    <section class="kpi-panel"><h2>Pendientes</h2>
      <div class="filtros-tareas"><label>Desde <input type="date" value={desde} onInput={e=>setDesde((e.target as HTMLInputElement).value)} /></label><label>Hasta <input type="date" value={hasta} onInput={e=>setHasta((e.target as HTMLInputElement).value)} /></label><select value={prioridad} onChange={e=>setPrioridad((e.target as HTMLSelectElement).value)}><option value="">Todas las prioridades</option><option>A</option><option>B</option><option>C</option></select><select value={servicio} onChange={e=>setServicio((e.target as HTMLSelectElement).value)}><option value="">Todos los servicios</option><option value="creacion_web">Creación web</option><option value="landing">Landing</option><option value="seo_on_page">SEO on-page</option><option value="hosting">Hosting</option></select><select value={estado} onChange={e=>setEstado((e.target as HTMLSelectElement).value)}><option value="">Todos los estados</option>{["nuevo","contactado","visita_hecha","interesado","no_interesado","propuesta","cerrado"].map(x=><option value={x}>{x}</option>)}</select></div>
      {tareas.length===0?<p class="muted">No hay seguimientos que coincidan con estos filtros.</p>:<ul class="tareas-lista">{tareas.map(t=><li key={t.negocio_id}>
        <button class="link-button" onClick={async()=>{await cargarDetalle(t.negocio_id);abrirNegocio();}}><strong>{t.nombre}</strong> · {t.municipio}</button>
        <span>{t.fecha}: {t.siguiente_accion}</span><span class={`badge ${t.prioridad==="A"?"potencial-alto":t.prioridad==="B"?"potencial-medio":"potencial-bajo"}`}>{t.prioridad??"Sin prioridad"}</span><span>{t.estado}</span>
      </li>)}</ul>}
    </section>
    <section class="kpi-panel"><h2>Plantillas de contacto</h2>
      <ul>{plantillasContacto.value.map(p=><li key={p.id}><strong>{p.nombre}</strong> · {p.canal}{p.asunto?` · ${p.asunto}`:""}</li>)}</ul>
      <h3>Nueva plantilla</h3><div class="form-fuente">
        <input value={nombre} placeholder="Nombre de plantilla" onInput={e=>setNombre((e.target as HTMLInputElement).value)} />
        <select value={canal} onChange={e=>setCanal((e.target as HTMLSelectElement).value)}><option value="email">Email</option><option value="telefono">Teléfono</option><option value="otro">Otro</option></select>
        <input value={asunto} placeholder="Asunto opcional" onInput={e=>setAsunto((e.target as HTMLInputElement).value)} />
        <textarea rows={6} value={cuerpo} onInput={e=>setCuerpo((e.target as HTMLTextAreaElement).value)} />
        <p class="muted">Variables admitidas: <code>{"{{nombre}}"}</code>. Las plantillas solo preparan borradores; no envían mensajes.</p>
        <button disabled={!nombre.trim()||!cuerpo.trim()} onClick={async()=>{try{await crearPlantilla({nombre,canal,servicio:null,asunto:asunto||null,cuerpo});setNombre("");setMensaje("Plantilla guardada.");}catch(e){setMensaje(String(e));}}}>Guardar plantilla</button>
      </div>
    </section>
  </section>;
}
