import { useState } from "preact/hooks";
import { jobs, error, lanzarScan, lanzarAudit, recargarJobs } from "../stores/jobs";

// Ejecución de búsquedas desde la app web (ADR-002): scan/audit lanzan jobs
// en el binario Rust y aquí se ve el progreso por polling.
export function PanelJobs() {
  const [municipio, setMunicipio] = useState("Pamplona");
  const [sector, setSector] = useState("");
  const [prioridad, setPrioridad] = useState("A");

  return (
    <section class="jobs">
      <h2>Ejecutar búsquedas</h2>
      <div class="acciones">
        <input value={municipio} onInput={(e) => setMunicipio((e.target as HTMLInputElement).value)} placeholder="Municipio" />
        <select value={sector} onChange={(e) => setSector((e.target as HTMLSelectElement).value)}>
          {["", "hosteleria", "comercio", "servicios", "profesionales", "industria", "todos"].map((s) => (
            <option value={s}>{s || "todos los sectores"}</option>
          ))}
        </select>
        <button onClick={() => lanzarScan(municipio, sector)}>Scan (Overpass → SQLite)</button>
      </div>
      <div class="acciones">
        <select value={prioridad} onChange={(e) => setPrioridad((e.target as HTMLSelectElement).value)}>
          {["", "A", "B", "C"].map((s) => <option value={s}>{s ? `auditar prioridad ${s}` : "auditar todo"}</option>)}
        </select>
        <button onClick={() => lanzarAudit(prioridad, null)}>Auditar webs</button>
        <button class="secundario" onClick={recargarJobs}>Refrescar</button>
      </div>
      {error.value && <p class="error">{error.value}</p>}

      <table>
        <thead>
          <tr><th>Tipo</th><th>Estado</th><th>Progreso</th><th>Creado</th><th>Error</th></tr>
        </thead>
        <tbody>
          {jobs.value.map((j) => (
            <tr key={j.id}>
              <td>{j.tipo}</td>
              <td>{j.estado}</td>
              <td>{j.hechos}/{j.total}</td>
              <td>{j.creado.slice(0, 19).replace("T", " ")}</td>
              <td class="error">{j.error ?? ""}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
