import { useState } from "preact/hooks";
import { jobs, error, cargando, lanzarScan, lanzarAudit, recargarJobs, reintentarJob } from "../stores/jobs";

// Ejecución de búsquedas desde la app web (ADR-002): scan/audit lanzan jobs
// en el binario Rust y aquí se ve el progreso por polling.
export function PanelJobs() {
  const [sector, setSector] = useState("");
  const [prioridad, setPrioridad] = useState("");

  const etiquetaEstado = (tipo: string, estado: string) => {
    if (estado === "en_curso") {
      if (tipo === "technical") return "Auditoría técnica";
      if (tipo === "discovery") return "Localizando webs";
      if (tipo === "audit") return "Auditando";
      if (tipo === "scan") return "Buscando";
      if (tipo === "watch") return "Importando";
      if (tipo === "rebuild") return "Reconstruyendo";
      return "En curso";
    }
    return ({ pendiente: "Pendiente", terminado: "Completado", error: "Error" } as Record<string, string>)[estado] ?? estado;
  };

  return (
    <section class="jobs">
      <h2>Buscar empresas</h2>
      <p class="muted">Busca empresas en toda la Comunidad Foral de Navarra. Los nuevos registros se añaden sin duplicar negocios existentes.</p>
      <div class="acciones">
        <select value={sector} onChange={(e) => setSector((e.target as HTMLSelectElement).value)}>
          {["", "hosteleria", "comercio", "servicios", "profesionales", "industria", "todos"].map((s) => (
            <option value={s}>{s || "todos los sectores"}</option>
          ))}
        </select>
        <button disabled={cargando.value || jobs.value.some(j => j.tipo === "rebuild" && ["pendiente","en_curso"].includes(j.estado))} onClick={() => void lanzarScan([], sector, true)}>
          {cargando.value ? "Buscando…" : "Buscar en toda Navarra"}
        </button>
      </div>
      <div class="acciones">
        <select value={prioridad} onChange={(e) => setPrioridad((e.target as HTMLSelectElement).value)}>
          {["", "A", "B", "C"].map((s) => <option value={s}>{s ? `auditar prioridad ${s}` : "auditar todo"}</option>)}
        </select>
        <button disabled={cargando.value || jobs.value.some(j => j.tipo === "rebuild" && ["pendiente","en_curso"].includes(j.estado))} onClick={() => void lanzarAudit(prioridad, null)}>{prioridad ? `Auditar prioridad ${prioridad}` : "Auditar todas las empresas"}</button>
        <button class="secundario" onClick={recargarJobs}>Refrescar</button>
      </div>
      <p class="muted">La auditoría completa no tiene límite fijo: avanza en lotes de 100, con hasta 8 webs simultáneas y pausas breves entre lotes.</p>
      {error.value && <p class="error">{error.value}</p>}

      <table>
        <thead>
          <tr><th>Tipo</th><th>Estado</th><th>Progreso</th><th>Resultado</th><th>Creado</th><th>Error</th></tr>
        </thead>
        <tbody>
          {jobs.value.map((j) => (
            <tr key={j.id}>
              <td>{({rebuild:"Reconstrucción",scan:"Búsqueda",audit:"Auditoría",crawl:"Rastreo",watch:"Altas",technical:"Lighthouse + SiteOne",discovery:"Buscar webs"} as Record<string,string>)[j.tipo] ?? j.tipo}</td>
              <td><span class={`badge badge-${j.estado}`}>{etiquetaEstado(j.tipo, j.estado)}</span></td>
              <td>{j.hechos}/{j.total}{j.tipo === "scan" ? " fases" : ""}</td>
              <td>{j.resumen ? (
                <pre class="job-resumen">{JSON.stringify(j.resumen, null, 0)}</pre>
              ) : "—"}</td>
              <td>{j.creado.slice(0, 19).replace("T", " ")}</td>
              <td class="error">{j.error ?? ""}{j.estado === "error" && <button class="secundario" onClick={() => void reintentarJob(j.id)}>Reintentar</button>}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
