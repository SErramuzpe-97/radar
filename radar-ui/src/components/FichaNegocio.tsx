import { useCallback, useEffect, useRef, useState } from "preact/hooks";
import { detalle, seleccionadoId, guardarPipeline, registrarEvento, recargar } from "../stores/negocios";
import { WebNegocio } from "./WebNegocio";
import { AuditoriaTecnica, type Device, type Engine } from "./AuditoriaTecnica";
import { DiagnosticoWeb } from "./DiagnosticoWeb";
import type { components } from "../api/schema";
import { rubroNombre } from "../rubros";
import { transicionar } from "../transicion";

const ESTADOS: Record<string, string> = { nuevo:"Nuevo", contactado:"Contactado", visita_hecha:"Visita hecha", interesado:"Interesado", no_interesado:"No interesado", propuesta:"Propuesta", cerrado:"Cerrado" };
const ACTIVIDADES: Record<string, string> = { visita:"Visita", llamada:"Llamada", email:"Email", reunion:"Reunión", oferta:"Oferta" };
const SERVICIOS: Record<string, string> = { creacion_web:"Creación web", landing:"Landing", seo_on_page:"SEO on-page", hosting:"Hosting" };

export function FichaNegocio({ origen }: { origen: "tabla" | "mapa" }) {
  const d = detalle.value;
  const [tab,setTab]=useState("ficha");
  const [auditEngines,setAuditEngines]=useState<string[]>([]);
  const [deviceByEngine,setDeviceByEngine]=useState<Record<Engine,Device>>({lighthouse:"mobile",siteone:"mobile"});
  const ref = useRef<HTMLElement>(null);
  const [estado, setEstado] = useState("nuevo");
  const [tipo, setTipo] = useState("visita");
  const [notas, setNotas] = useState("");
  const [guardando, setGuardando] = useState<"estado" | "actividad" | null>(null);
  const [mensaje, setMensaje] = useState("");
  const [fallo, setFallo] = useState(false);
  const cerrar = () => transicionar(() => { detalle.value = null; seleccionadoId.value = null; });
  useEffect(() => { setEstado(d?.pipeline?.estado ?? "nuevo"); }, [d?.negocio.id, d?.pipeline?.estado]);
  useEffect(() => { setNotas(""); setMensaje(""); setTab("ficha"); setAuditEngines([]); setDeviceByEngine({lighthouse:"mobile",siteone:"mobile"}); ref.current?.querySelector(".ficha-contenido")?.scrollTo(0,0); }, [d?.negocio.id]);
  const actualizarMotores = useCallback((motores: string[]) => setAuditEngines(motores), []);
  useEffect(() => { if (["lighthouse","siteone"].includes(tab) && !auditEngines.includes(tab)) setTab("auditoria"); }, [tab,auditEngines]);
  useEffect(() => {
    if (!d || !window.matchMedia("(max-width: 1199px), (any-pointer: coarse) and (max-width: 1600px)").matches) return;
    const anterior = document.activeElement as HTMLElement | null;
    ref.current?.focus();
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") cerrar();
      if (e.key === "Tab") {
        const elementos = ref.current?.querySelectorAll<HTMLElement>('button:not(:disabled),select,input,textarea,a[href]');
        if (!elementos?.length) return;
        const primero = elementos[0], ultimo = elementos[elementos.length-1];
        if (e.shiftKey && (document.activeElement === primero || document.activeElement === ref.current)) { e.preventDefault(); ultimo.focus(); }
        else if (!e.shiftKey && document.activeElement === ultimo) { e.preventDefault(); primero.focus(); }
      }
    };
    document.addEventListener("keydown", key);
    return () => { document.removeEventListener("keydown", key); anterior?.focus(); };
  }, [d?.negocio.id]);
  if (!d) return <aside class="ficha ficha-vacia muted">Selecciona un negocio para ver su ficha…</aside>;
  const n = d.negocio;
  const rubroEvidence = (n.evidencia_rubro ?? {}) as Record<string, any>;
  const visitEvidence = rubroEvidence.visitas as Record<string, any> | undefined;
  const feeEvidence = rubroEvidence.tarifa as Record<string, any> | undefined;
  const webEvidence = rubroEvidence.presencia_web as Record<string, any> | undefined;
  const ultima = d.auditorias[0];
  const servicios = (n.oportunidades ?? []).filter(o => o.score > 0);
  const guardar = async (accion: "estado" | "actividad") => {
    setGuardando(accion); setMensaje(""); setFallo(false);
    try {
      if (accion === "estado") {
        await guardarPipeline(n.id, { estado: estado as components["schemas"]["EstadoPipeline"] });
        await recargar();
      } else { await registrarEvento(n.id, { tipo, notas: notas.trim() || null }); setNotas(""); }
      setMensaje(accion === "estado" ? "Estado guardado." : "Actividad guardada.");
    } catch (e) { setFallo(true); setMensaje(String(e)); }
    finally { setGuardando(null); }
  };
  return <aside class="ficha ficha-abierta" ref={ref} tabIndex={-1} aria-label={`Ficha de ${n.nombre}`}>
    <header class="ficha-cabecera"><button class="secundario cerrar-ficha" onClick={cerrar} aria-label={`Volver a ${origen === "tabla" ? "la tabla" : "el mapa"}`}>← <span>Volver a {origen === "tabla" ? "tabla" : "mapa"}</span></button>
    <div class="ficha-tamano"><button type="button" class="secundario" onClick={() => document.documentElement.style.removeProperty("--radar-ficha-width")}>Reset tamaño</button></div>
    <h2>{n.nombre}</h2>
    <div class="ficha-tabs" role="tablist" aria-label="Contenido de la ficha">{[["ficha","Ficha"],["auditoria","Auditoría"],["diagnostico","Diagnóstico web"],...(auditEngines.includes("lighthouse")?[["lighthouse","Lighthouse"]]:[]),...(auditEngines.includes("siteone")?[["siteone","SiteOne"]]:[])].map(([v,l])=><button key={v} role="tab" aria-selected={tab===v} onClick={()=>transicionar(()=>setTab(v))} class={tab===v?"":"secundario"}>{l}</button>)}</div></header>
    <div class={`ficha-contenido ${tab==="lighthouse"||tab==="siteone"?"informe-activo":""}`}>
    {tab==="ficha"&&<>
    <div class="ficha-etiquetas"><span class="badge">{rubroNombre(n.rubro ?? n.sector ?? n.categoria ?? "otro")}</span>{(rubroEvidence as any).tipo_entidad&&<span class="badge">{(rubroEvidence as any).tipo_entidad}</span>}{n.score_potencial != null && <span class={`badge ${n.score_potencial >= 70 ? "potencial-alto" : n.score_potencial >= 50 ? "potencial-medio" : "potencial-bajo"}`}>Potencial {n.score_potencial}/100 · {n.prioridad_comercial}</span>}<span class={`badge pipeline-${d.pipeline?.estado ?? "nuevo"}`}>{ESTADOS[d.pipeline?.estado ?? "nuevo"]}</span></div>
    <p class="ficha-ubicacion">{n.municipio}{n.direccion ? " · "+n.direccion : ""}</p>
    {(n.telefono||n.email)&&<div class="ficha-contactos">{n.telefono&&<a href={`tel:${n.telefono}`}>📞 {n.telefono}</a>}{n.email&&<a href={`mailto:${n.email}`}>✉️ {n.email}</a>}</div>}
    {rubroEvidence.perfil_web && <p class="muted">Sugerencia web: página centrada en carta, menú y reservas.</p>}
    {visitEvidence && <section class="oportunidades"><h3>Visitas y tarifa</h3><p>{visitEvidence.estado === "visitas_guiadas_indicadas" ? "Visitas guiadas indicadas por la fuente." : visitEvidence.estado === "actividad_turistica_registrada" ? "Actividad turística registrada en fuente oficial; no confirma por sí sola visitas disponibles." : "La categoría de la fuente indica un lugar de visita; no confirma por sí sola visitas efectivas."} <small class="muted">Confianza {visitEvidence.confianza ?? "sin evaluar"}.</small></p><p>{feeEvidence?.estado === "declarada_de_pago" ? "La fuente indica que es de pago." : feeEvidence?.estado === "precio_declarado" ? `Precio indicado: ${feeEvidence.valor}.` : feeEvidence?.estado === "declarada_gratuita" ? "La fuente indica entrada gratuita." : "La tarifa no aparece en la fuente."} <small class="muted">Confianza {feeEvidence?.confianza ?? "sin evidencia"}.</small></p><small class="muted">Presencia web: {webEvidence?.estado === "registrada_en_osm" || webEvidence?.estado === "registrada_en_fuente" ? "URL incluida en la fuente" : "sin URL en la fuente; esto no demuestra que no exista"} (confianza {webEvidence?.confianza ?? "baja"}).</small></section>}
    <WebNegocio/>
    {ultima && <section class="auditoria"><h3>Última auditoría</h3><p><strong>{ultima.estado_web==="SIN_WEB"?"Web no localizada":ultima.estado_web==="NO_VERIFICABLE"?"No verificable temporalmente":ultima.estado_web.replaceAll("_", " ")}</strong> · {new Date(ultima.fecha).toLocaleString("es-ES")}</p></section>}
    <section class="oportunidades"><h3>Servicios que pueden encajar</h3><div class="servicios-tags">
      {servicios.length ? servicios.map(o => <span key={o.servicio} class={`badge ${o.score >= 70 ? "potencial-alto" : o.score >= 50 ? "potencial-medio" : "potencial-bajo"}`} title={o.motivo}>{SERVICIOS[o.servicio]} · {o.score}/100</span>) : <span class="muted">Sin encaje suficiente detectado todavía.</span>}
    </div></section>
    <section class="seguimiento-ficha"><h3>Seguimiento</h3>
      <p>Estado: <span class={`badge pipeline-${d.pipeline?.estado ?? "nuevo"}`}>{ESTADOS[d.pipeline?.estado ?? "nuevo"]}</span></p>
      <form class="cambio-estado" onSubmit={e => { e.preventDefault(); void guardar("estado"); }}>
        <select aria-label="Cambiar estado" value={estado} onChange={e => setEstado((e.target as HTMLSelectElement).value)}>{Object.entries(ESTADOS).map(([v,l]) => <option key={v} value={v}>{l}</option>)}</select>
        <button disabled={guardando !== null}>{guardando === "estado" ? "Guardando…" : "Guardar estado"}</button>
      </form>
      <h4>Registrar actividad</h4>
      <form class="actividad-form" onSubmit={e => { e.preventDefault(); void guardar("actividad"); }}>
        <select aria-label="Tipo de actividad" value={tipo} onChange={e => setTipo((e.target as HTMLSelectElement).value)}>{Object.entries(ACTIVIDADES).map(([v,l]) => <option key={v} value={v}>{l}</option>)}</select>
        <textarea aria-label="Notas de la actividad" rows={5} placeholder="Notas de la actividad…" value={notas} onInput={e => setNotas((e.target as HTMLTextAreaElement).value)} />
        <button disabled={guardando !== null}>{guardando === "actividad" ? "Guardando…" : "Guardar actividad"}</button>
      </form>
      {mensaje && <p role="status" class={fallo ? "error" : "muted"}>{mensaje}</p>}
      {d.eventos.length > 0 && <div class="historial"><h4>Histórico de actividad</h4><ul>{d.eventos.map(ev => <li key={ev.id}><strong>{ACTIVIDADES[ev.tipo] ?? ev.tipo}</strong> · {new Date(ev.fecha).toLocaleString("es-ES")}{ev.notas && <p>{ev.notas}</p>}</li>)}</ul></div>}
    </section>
    </>}
    {tab==="diagnostico"&&<DiagnosticoWeb id={n.id} web={d.web_info.url_confirmada} name={n.nombre}/>}
    <AuditoriaTecnica id={n.id} web={d.web_info.url_confirmada} mode={tab==="auditoria"?"summary":tab==="lighthouse"||tab==="siteone"?tab:null} onAvailabilityChange={actualizarMotores} deviceByEngine={deviceByEngine} onDeviceChange={(engine,device)=>setDeviceByEngine(current=>({...current,[engine]:device}))}/>
    </div>
    <div class="ficha-resize-handle" role="separator" aria-orientation="vertical" aria-label="Cambiar ancho de la ficha" tabIndex={0}
      onPointerDown={e => {
        if (window.matchMedia("(max-width: 899px)").matches) return;
        const startX = e.clientX;
        const startWidth = ref.current?.getBoundingClientRect().width ?? Math.min(640, Math.max(320, window.innerWidth * 0.25));
        const move = (event: PointerEvent) => {
          const max = Math.max(360, window.innerWidth - 360);
          const width = Math.min(max, Math.max(320, startWidth + startX - event.clientX));
          document.documentElement.style.setProperty("--radar-ficha-width", `${width}px`);
        };
        const done = () => { window.removeEventListener("pointermove", move); window.removeEventListener("pointerup", done); };
        window.addEventListener("pointermove", move);
        window.addEventListener("pointerup", done, { once: true });
      }} />
  </aside>;
}
