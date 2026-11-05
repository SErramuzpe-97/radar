import { useEffect, useState } from "preact/hooks";
import { filtros, pagina, cargando, error, recargar, programarRecarga, cancelarCargaLista, cargarDetalle, seleccionadoId, actualizarDescartados } from "../stores/negocios";
import { RUBROS, TIPOS_ENTIDAD, rubroNombre } from "../rubros";
import { Desplegable } from "./Desplegable";

const SECTORES = ["", "hosteleria", "comercio", "servicios", "profesionales", "industria"];
const ESTADOS = ["", "CON_WEB", "SIN_WEB", "SOLO_REDES", "WEB_ROTA", "WEB_APARCADA", "WEB_OBSOLETA", "WEB_MEJORABLE", "WEB_OK"];
const PRIORIDADES = ["", "A", "B", "C"];
const ESTADOS_PIPELINE: [string,string][] = [["", "Todos los estados de seguimiento"],["nuevo","Nuevo"],["contactado","Contactado"],["visita_hecha","Visita hecha"],["interesado","Interesado"],["no_interesado","No interesado"],["propuesta","Propuesta"],["cerrado","Cerrado"]];

function prioridadClase(p: string | undefined) {
  return p === "A" ? "potencial-alto" : p === "B" ? "potencial-medio" : "potencial-bajo";
}

const SERVICIOS: Record<string, string> = {
  creacion_web: "Web", landing: "Landing", seo_on_page: "SEO", hosting: "Hosting",
};
const PIPELINE: Record<string, string> = {
  nuevo: "Nuevo", contactado: "Contactado", visita_hecha: "Visita hecha", interesado: "Interesado",
  no_interesado: "No interesado", propuesta: "Propuesta", cerrado: "Cerrado",
};
const WEB_ESTADOS: Record<string, [string,string]> = {
  confirmada:["Web confirmada por ti","potencial-alto"], verificada:["Web verificada automáticamente","potencial-alto"], sugerida:["Web sugerida","potencial-medio"], sin_localizar:["Web no localizada","potencial-sin-datos"],
};
const AUDITORIA_ESTADOS: Record<string, [string,string]> = {
  realizada:["Auditada","potencial-alto"], en_curso:["En curso","estado-audit-en_curso"], pendiente:["Auditoría pendiente","badge-pendiente"], error:["Error de auditoría","badge-error"],
};

export function TablaNegocios() {
  useEffect(() => () => cancelarCargaLista(), []);
  const [seleccionados, setSeleccionados] = useState<number[]>([]);
  const [filtrosAbiertos, setFiltrosAbiertos] = useState(() => !window.matchMedia("(max-width: 1199px), (any-pointer: coarse) and (max-width: 1600px)").matches);
  const f = filtros.value;
  const p = pagina.value;
  const filas = p?.items ?? [];

  const procesarSeleccion = async () => {
    try {
      await actualizarDescartados(seleccionados, !f.mostrar_descartados);
      setSeleccionados([]);
    } catch (e) { error.value = String(e); }
  };

  const filtrosActivos = [f.q, f.municipio, f.sector, f.rubro, f.tipo_entidad, f.estado_web,
    f.estado_auditoria_tecnica, f.estado_pipeline, f.prioridad].filter(Boolean).length;

  return (
    <section class="tabla">
      <div class="seleccion-barra">
        <label><input type="checkbox" checked={f.mostrar_descartados} onChange={(e) => {
          setSeleccionados([]);
          filtros.value = { ...filtros.value, mostrar_descartados: (e.target as HTMLInputElement).checked, pagina: 1 };
          void recargar();
        }} />Ver solo negocios descartados</label>
        <span class="muted">{seleccionados.length} seleccionados</span>
        <button class={f.mostrar_descartados ? "secundario" : "peligro-suave"} disabled={!seleccionados.length || seleccionados.length > 500}
          onClick={() => void procesarSeleccion()}>
          {f.mostrar_descartados ? "Restaurar seleccionados" : "Descartar seleccionados"}
        </button>
      </div>
      <Desplegable className="filtros-panel" titulo="Filtros" inicialAbierto={filtrosAbiertos} indicador={filtrosActivos > 0 && <span class="filtros-activos">{filtrosActivos} activos</span>}>
      <div class="filtros">
        <input placeholder="Buscar nombre…" value={f.q} onInput={(e) => ((filtros.value = { ...filtros.value, q: (e.target as HTMLInputElement).value, pagina: 1 }), programarRecarga())} />
        <input placeholder="Municipio…" value={f.municipio} onInput={(e) => ((filtros.value = { ...filtros.value, municipio: (e.target as HTMLInputElement).value, pagina: 1 }), programarRecarga())} />
        <select value={f.sector} onChange={(e) => ((filtros.value = { ...filtros.value, sector: (e.target as HTMLSelectElement).value, pagina: 1 }), recargar())}>
          {SECTORES.map((s) => <option value={s}>{s || "todos los sectores"}</option>)}
        </select>
        <select aria-label="Filtrar rubro" value={f.rubro} onChange={(e) => ((filtros.value = { ...filtros.value, rubro: (e.target as HTMLSelectElement).value, pagina: 1 }), recargar())}>
          {RUBROS.map(([v, label]) => <option value={v}>{label}</option>)}
        </select>
        <select aria-label="Filtrar tipo de entidad" value={f.tipo_entidad} onChange={(e) => ((filtros.value = { ...filtros.value, tipo_entidad: (e.target as HTMLSelectElement).value, pagina: 1 }), recargar())}>
          {TIPOS_ENTIDAD.map(([v, label]) => <option value={v}>{label}</option>)}
        </select>
        <select aria-label="Filtrar estado web" value={f.estado_web} onChange={(e) => ((filtros.value = { ...filtros.value, estado_web: (e.target as HTMLSelectElement).value, pagina: 1 }), recargar())}>
          {ESTADOS.map((s) => <option value={s}>{s || "Todos los estados web"}</option>)}
        </select>
        <select aria-label="Filtrar estado de auditoría" value={f.estado_auditoria_tecnica} onChange={(e) => ((filtros.value = { ...filtros.value, estado_auditoria_tecnica: (e.target as HTMLSelectElement).value, pagina: 1 }), recargar())}>
          <option value="">Todas las auditorías</option><option value="realizada">Auditadas</option><option value="pendiente">Pendientes</option><option value="en_curso">En curso</option><option value="error">Con error</option>
        </select>
        <select aria-label="Filtrar estado de seguimiento" value={f.estado_pipeline} onChange={(e) => ((filtros.value = { ...filtros.value, estado_pipeline: (e.target as HTMLSelectElement).value, pagina: 1 }), recargar())}>
          {ESTADOS_PIPELINE.map(([v,l]) => <option value={v}>{l}</option>)}
        </select>
        <select value={f.prioridad} onChange={(e) => ((filtros.value = { ...filtros.value, prioridad: (e.target as HTMLSelectElement).value, pagina: 1 }), recargar())}>
          {PRIORIDADES.map((s) => <option value={s}>{s ? `prioridad ${s}` : "todas"}</option>)}
        </select>
        <select value={f.ordenar} onChange={(e) => ((filtros.value = { ...filtros.value, ordenar: (e.target as HTMLSelectElement).value }), recargar())}>
          <option value="score">ordenar por score</option>
          <option value="nombre">por nombre</option>
          <option value="municipio">por municipio</option>
          <option value="fecha">por fecha</option>
        </select>
        <button class="secundario actualizar-tabla" disabled={cargando.value} onClick={() => void recargar()}>
          {cargando.value ? "Actualizando…" : "Actualizar tabla"}
        </button>
      </div>
      </Desplegable>

      {error.value && <p class="error">{error.value}</p>}
      {cargando.value && !p && <p class="muted">Cargando negocios…</p>}

      <div class="tabla-resultados">
      <div class="tabla-cuerpo" role="region" aria-label="Resultados de negocios" aria-busy={cargando.value} tabIndex={0}>
      <table>
        <thead>
          <tr>
            <th><input type="checkbox" aria-label="Seleccionar todos los negocios de esta página" checked={filas.length > 0 && filas.every((n) => seleccionados.includes(n.id))}
              onChange={(e) => {
                const checked = (e.target as HTMLInputElement).checked;
                setSeleccionados(checked ? [...new Set([...seleccionados, ...filas.map((n) => n.id)])] : seleccionados.filter((id) => !filas.some((n) => n.id === id)));
              }} /></th>
            <th>Nombre</th><th>Sector</th><th>Municipio</th><th>Web / auditoría</th><th>Potencial</th><th>Prioridad</th><th>Servicios</th><th>Seguimiento</th>
          </tr>
        </thead>
        <tbody>
          {p?.items.map((n) => {
            const prioridad = n.prioridad_comercial ?? undefined;
            const evidencia = n.evidencia_rubro as { tarifa?: { estado?: string } } | null | undefined;
            const tarifa = evidencia?.tarifa?.estado;
            return (
              <tr
                key={n.id}
                tabIndex={0}
                onKeyDown={e => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); void cargarDetalle(n.id); } }}
                class={`${seleccionadoId.value === n.id ? "seleccionada" : ""} ${n.descartado ? "fila-descartada" : ""}`}
                onClick={() => cargarDetalle(n.id)}
              >
                <td onClick={(e) => e.stopPropagation()}><input type="checkbox" aria-label={`Seleccionar ${n.nombre}`} checked={seleccionados.includes(n.id)} onChange={(e) => {
                  const checked = (e.target as HTMLInputElement).checked;
                  setSeleccionados(checked ? [...new Set([...seleccionados, n.id])] : seleccionados.filter((id) => id !== n.id));
                }} /></td>
                <td>{n.nombre}{n.descartado && <span class="badge descartado-badge">Descartado</span>}</td>
                <td>{n.sector ?? "—"}{n.rubro && <small class="rubro-detalle">{rubroNombre(n.rubro)}</small>}{n.rubro === "turismo_cultural" && n.evidencia_rubro && <small class="rubro-detalle">{tarifa === "declarada_de_pago" || tarifa === "precio_declarado" ? "Visita y tarifa indicadas" : tarifa === "declarada_gratuita" ? "Visita indicada · gratuita" : "Visita indicada · tarifa sin dato"}</small>}</td>
                <td>{n.municipio}</td>
                <td><div class="estado-web-auditoria"><span class={`badge ${WEB_ESTADOS[n.estado_web_lista??"sin_localizar"]?.[1]??"potencial-sin-datos"}`}>{WEB_ESTADOS[n.estado_web_lista??"sin_localizar"]?.[0]??"Web no localizada"}</span><span class={`badge ${AUDITORIA_ESTADOS[n.estado_auditoria_tecnica??"pendiente"]?.[1]??"badge-pendiente"}`}>{AUDITORIA_ESTADOS[n.estado_auditoria_tecnica??"pendiente"]?.[0]??"Auditoría pendiente"}</span>{n.web&&<a href={n.web} target="_blank" rel="noreferrer" onClick={(e)=>e.stopPropagation()}>Abrir web</a>}</div></td>
                <td>{n.score_potencial == null ? <span class="badge potencial-sin-datos">Sin auditar</span> : <span class={`badge ${prioridadClase(prioridad)}`}>{n.score_potencial}/100</span>}</td>
                <td>{prioridad ? <span class={`badge ${prioridadClase(prioridad)}`}>{prioridad}</span> : "—"}</td>
                <td><div class="servicios-tags">{(n.oportunidades ?? []).filter((o) => o.score >= 45).map((o) => <span class={`servicio-tag ${prioridadClase(o.score >= 70 ? "A" : o.score >= 50 ? "B" : "C")}`} title={o.motivo}>{SERVICIOS[o.servicio] ?? o.servicio}</span>)}</div></td>
                <td><span class={`badge pipeline-${n.estado_pipeline ?? "nuevo"}`}>{PIPELINE[n.estado_pipeline ?? "nuevo"] ?? "Nuevo"}</span></td>
              </tr>
            );
          })}
        </tbody>
      </table>
      </div>

      {p && (
        <div class="paginacion">
          <button disabled={f.pagina <= 1} onClick={() => ((filtros.value = { ...filtros.value, pagina: f.pagina - 1 }), recargar())}>←</button>
          <span>
            Página {p.pagina} de {Math.max(1, Math.ceil(p.total / p.tam))} ({p.total} negocios)
          </span>
          <button disabled={f.pagina * f.tam >= p.total} onClick={() => ((filtros.value = { ...filtros.value, pagina: f.pagina + 1 }), recargar())}>→</button>
        </div>
      )}
      {cargando.value && <div class="tabla-estado" role="status" aria-live="polite"><span class="carga-spinner"/><span>Actualizando resultados…</span></div>}
      </div>
    </section>
  );
}
