import { filtros, pagina, cargando, error, recargar, cargarDetalle, seleccionadoId } from "../stores/negocios";

const SECTORES = ["", "hosteleria", "comercio", "servicios", "profesionales", "industria"];
const ESTADOS = ["", "SIN_WEB", "SOLO_REDES", "WEB_ROTA", "WEB_APARCADA", "WEB_OBSOLETA", "WEB_MEJORABLE", "WEB_OK"];
const PRIORIDADES = ["", "A", "B", "C"];

function prioridadClase(p: string | undefined) {
  return p === "A" ? "prio-a" : p === "B" ? "prio-b" : "prio-c";
}

export function TablaNegocios() {
  const f = filtros.value;
  const p = pagina.value;

  return (
    <section class="tabla">
      <div class="filtros">
        <input placeholder="Buscar nombre…" value={f.q} onInput={(e) => ((filtros.value = { ...f, q: (e.target as HTMLInputElement).value, pagina: 1 }), recargar())} />
        <input placeholder="Municipio…" value={f.municipio} onInput={(e) => ((filtros.value = { ...f, municipio: (e.target as HTMLInputElement).value, pagina: 1 }), recargar())} />
        <select value={f.sector} onChange={(e) => ((filtros.value = { ...f, sector: (e.target as HTMLSelectElement).value, pagina: 1 }), recargar())}>
          {SECTORES.map((s) => <option value={s}>{s || "todos los sectores"}</option>)}
        </select>
        <select value={f.estado_web} onChange={(e) => ((filtros.value = { ...f, estado_web: (e.target as HTMLSelectElement).value, pagina: 1 }), recargar())}>
          {ESTADOS.map((s) => <option value={s}>{s || "todos los estados"}</option>)}
        </select>
        <select value={f.prioridad} onChange={(e) => ((filtros.value = { ...f, prioridad: (e.target as HTMLSelectElement).value, pagina: 1 }), recargar())}>
          {PRIORIDADES.map((s) => <option value={s}>{s ? `prioridad ${s}` : "todas"}</option>)}
        </select>
        <select value={f.ordenar} onChange={(e) => ((filtros.value = { ...f, ordenar: (e.target as HTMLSelectElement).value }), recargar())}>
          <option value="score">ordenar por score</option>
          <option value="nombre">por nombre</option>
          <option value="municipio">por municipio</option>
          <option value="fecha">por fecha</option>
        </select>
      </div>

      {error.value && <p class="error">{error.value}</p>}
      {cargando.value && <p class="muted">cargando…</p>}

      <table>
        <thead>
          <tr>
            <th>Nombre</th><th>Sector</th><th>Municipio</th><th>Web</th><th>Score</th><th>Prioridad</th>
          </tr>
        </thead>
        <tbody>
          {p?.items.map((n) => {
            const ultima = null as null; // el score/prioridad se muestran desde el detalle al seleccionar
            void ultima;
            return (
              <tr
                key={n.id}
                class={seleccionadoId.value === n.id ? "seleccionada" : ""}
                onClick={() => cargarDetalle(n.id)}
              >
                <td>{n.nombre}</td>
                <td>{n.sector ?? "—"}</td>
                <td>{n.municipio}</td>
                <td>{n.web ? <a href={n.web} target="_blank" rel="noreferrer" onClick={(e) => e.stopPropagation()}>web</a> : "—"}</td>
                <td>—</td>
                <td><span class={prioridadClase(undefined)}>—</span></td>
              </tr>
            );
          })}
        </tbody>
      </table>

      {p && (
        <div class="paginacion">
          <button disabled={f.pagina <= 1} onClick={() => ((filtros.value = { ...f, pagina: f.pagina - 1 }), recargar())}>←</button>
          <span>
            página {p.pagina} de {Math.max(1, Math.ceil(p.total / p.tam))} ({p.total} negocios)
          </span>
          <button disabled={f.pagina * f.tam >= p.total} onClick={() => ((filtros.value = { ...f, pagina: f.pagina + 1 }), recargar())}>→</button>
        </div>
      )}
    </section>
  );
}
