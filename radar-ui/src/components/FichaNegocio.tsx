import { useState } from "preact/hooks";
import { detalle, guardarPipeline, registrarEvento, recargar } from "../stores/negocios";

const ESTADOS_PIPELINE = ["nuevo", "contactado", "visita_hecha", "interesado", "no_interesado", "propuesta", "cerrado"];
const TIPOS_EVENTO = ["visita", "llamada", "email", "reunion", "oferta"];

export function FichaNegocio() {
  const d = detalle.value;
  const [nuevoEstado, setNuevoEstado] = useState("contactado");
  const [tipoEv, setTipoEv] = useState("visita");
  const [notas, setNotas] = useState("");

  if (!d) return <aside class="ficha muted">Selecciona un negocio de la tabla…</aside>;

  const n = d.negocio;
  const ultima = d.auditorias[0];
  const pipeline = d.pipeline;

  return (
    <aside class="ficha">
      <h2>{n.nombre}</h2>
      <p class="muted">
        {n.categoria ?? "—"} · {n.sector ?? "—"} · {n.municipio}
      </p>
      {n.direccion && <p>📍 {n.direccion}</p>}
      {n.telefono && <p>📞 {n.telefono}</p>}
      {n.email && <p>✉️ {n.email}</p>}
      {n.web && <p>🌐 <a href={n.web} target="_blank" rel="noreferrer">{n.web}</a></p>}

      {ultima && (
        <div class="auditoria">
          <h3>Última auditoría</h3>
          <p>
            <strong>{ultima.estado_web}</strong> · score {ultima.score} · prioridad {ultima.prioridad}
          </p>
          <p class="muted">{ultima.fecha}</p>
        </div>
      )}

      <div class="pipeline">
        <h3>Pipeline</h3>
        <p>Estado: <strong>{pipeline?.estado ?? "nuevo"}</strong></p>
        <div class="acciones">
          <select value={nuevoEstado} onChange={(e) => setNuevoEstado((e.target as HTMLSelectElement).value)}>
            {ESTADOS_PIPELINE.map((s) => <option value={s}>{s}</option>)}
          </select>
          <button
            onClick={async () => {
              await guardarPipeline(n.id, { estado: nuevoEstado as never });
              await recargar();
            }}
          >
            Guardar estado
          </button>
        </div>
      </div>

      <div class="eventos">
        <h3>Registrar actividad</h3>
        <div class="acciones">
          <select value={tipoEv} onChange={(e) => setTipoEv((e.target as HTMLSelectElement).value)}>
            {TIPOS_EVENTO.map((s) => <option value={s}>{s}</option>)}
          </select>
          <input placeholder="notas…" value={notas} onInput={(e) => setNotas((e.target as HTMLInputElement).value)} />
          <button
            onClick={async () => {
              await registrarEvento(n.id, { tipo: tipoEv, notas: notas || null });
              setNotas("");
            }}
          >
            Registrar
          </button>
        </div>
        <ul>
          {d.eventos.map((ev) => (
            <li key={ev.id}>
              <strong>{ev.tipo}</strong> · {ev.fecha.slice(0, 16).replace("T", " ")}
              {ev.notas && <span class="muted"> — {ev.notas}</span>}
            </li>
          ))}
        </ul>
      </div>
    </aside>
  );
}
