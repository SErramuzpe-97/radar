import { useEffect } from "preact/hooks";
import { recargarStats, stats } from "../stores/negocios";

const NOMBRES: Record<string, string> = {
  SIN_WEB: "Sin web",
  SOLO_REDES: "Solo redes sociales",
  WEB_ROTA: "Web rota",
  WEB_APARCADA: "Web aparcada",
  WEB_OBSOLETA: "Web obsoleta",
  WEB_MEJORABLE: "Web mejorable",
  WEB_OK: "Web correcta",
  A: "Prioridad A",
  B: "Prioridad B",
  C: "Prioridad C",
  nuevo: "Nuevo",
  contactado: "Contactado",
  visita_hecha: "Visita realizada",
  interesado: "Interesado",
  no_interesado: "No interesado",
  propuesta: "Propuesta enviada",
  cerrado: "Cerrado",
};

function Distribucion({ titulo, datos }: { titulo: string; datos: { etiqueta: string; total: number }[] }) {
  const maximo = Math.max(1, ...datos.map((dato) => dato.total));
  return (
    <section class="kpi-panel">
      <h2>{titulo}</h2>
      {datos.length === 0 ? <p class="muted">Sin datos todavía.</p> : (
        <ul class="kpi-barras">
          {datos.map((dato) => (
            <li key={dato.etiqueta}>
              <div class="kpi-etiqueta"><span>{NOMBRES[dato.etiqueta] ?? dato.etiqueta}</span><strong>{dato.total}</strong></div>
              <div class="kpi-pista"><span style={{ width: `${Math.max(2, dato.total / maximo * 100)}%` }} /></div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

export function PanelStats() {
  useEffect(() => { void recargarStats(); }, []);
  const datos = stats.value;

  return (
    <section class="panel-kpis">
      <div class="kpi-cabecera">
        <div><h1>Panel de actividad</h1><p class="muted">Resumen del descubrimiento, auditorías y seguimiento comercial.</p></div>
        <button class="secundario" onClick={() => void recargarStats()}>Actualizar</button>
      </div>
      {!datos ? <p class="muted">Cargando indicadores…</p> : (
        <>
          <div class="kpi-resumen">
            <article class="kpi-total"><span class="muted">Negocios registrados</span><strong>{datos.total_negocios}</strong></article>
            <article class="kpi-total"><span class="muted">Eventos en los últimos 7 días</span><strong>{datos.eventos_ultimos_7_dias}</strong></article>
          </div>
          <div class="kpi-grid">
            <Distribucion titulo="Embudo del pipeline" datos={datos.por_pipeline} />
            <Distribucion titulo="Estado de la web" datos={datos.por_estado_web} />
            <Distribucion titulo="Prioridad comercial" datos={datos.por_prioridad} />
          </div>
        </>
      )}
    </section>
  );
}
