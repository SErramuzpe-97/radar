import { useEffect, useMemo, useState } from "preact/hooks";
import { consultarAuditoria, consultarRevisionHallazgo, guardarRevisionHallazgo } from "../stores/webs";
import { VisitaDiagnostico } from "./VisitaDiagnostico";
import type { components } from "../api/schema";
import { categoriaEs, errorAuditoriaEs, estadoAuditoriaEs, gravedadEs, traducirHallazgo } from "../diagnostico-es";

type Report = components["schemas"]["AuditoriaTecnica"];
type Device = "mobile" | "desktop";
type CategoryScore = { category: string; value: number };
const DEVICES: [Device, string][] = [["mobile", "Móvil"], ["desktop", "Escritorio"]];
const SCORE_CATEGORIES = ["accessibility", "best-practices", "performance", "seo"];
const SCORE_ALIASES: Record<string, string[]> = {
  accessibility: ["accessibility"],
  "best-practices": ["best-practices", "best_practices"],
  performance: ["performance"],
  seo: ["seo", "search-engine-optimization"],
};
const REVISION_ES: Record<string, string> = { pendiente: "Pendiente de contraste", confirmado: "Confirmado", descartado: "Descartado", resuelto: "Resuelto" };
const SEVERITY_RANK: Record<string, number> = { critical: 0, high: 1, medium: 2, low: 3, info: 4 };
function issueKey(issue: any) { return [issue.code, issue.url, issue.selector, issue.category === "responsive" ? issue.viewport?.width : ""].join("|"); }
const motorNombre = (motor: string) => motor === "lighthouse" ? "Lighthouse" : motor === "siteone" ? "SiteOne" : "Radar";
const esc = (value: unknown) => String(value ?? "").replace(/[&<>"']/g, char => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[char]!));
function base64(blob: Blob) { return new Promise<string>((resolve, reject) => { const reader = new FileReader(); reader.onload = () => resolve(String(reader.result).split(",")[1] || ""); reader.onerror = () => reject(reader.error); reader.readAsDataURL(blob); }); }
function normalizedScore(report: Report, category: string): number | null {
  const result = report.resultado as any;
  const scores = result?.scores ?? {};
  const key = (SCORE_ALIASES[category] ?? [category]).find(candidate => typeof scores[candidate] === "number");
  if (!key) return null;
  const raw = scores[key] as number;
  return Math.max(0, Math.min(100, Math.round(result.score_scale === 10 ? raw * 10 : raw)));
}

export function DiagnosticoWeb({ id, web, name }: { id: number; web?: string | null; name: string }) {
  const [reports, setReports] = useState<Report[]>([]);
  const [device, setDevice] = useState<Device>("mobile");
  const [status, setStatus] = useState("Cargando diagnóstico…");
  const [error, setError] = useState("");
  const [openGroups, setOpenGroups] = useState<Record<string, boolean>>({});
  const [selection, setSelection] = useState<{ key: string; keys: string[] } | null>(null);
  const [reviews, setReviews] = useState<Record<string, { estado: string; motivo?: string }>>({});

  useEffect(() => {
    let active = true;
    const load = async () => {
      try {
        const data = await consultarAuditoria(id);
        if (!active) return;
        setReports((data.informes ?? []).filter(report => !web || report.url === web));
        setStatus(data.job?.estado && ["pendiente", "en_curso"].includes(data.job.estado) ? estadoAuditoriaEs(data.job.estado) : "");
      } catch {
        if (active) {
          setStatus("No se pudo cargar el diagnóstico");
          setError("No se pudieron cargar los resultados. Comprueba la conexión e inténtalo de nuevo.");
        }
      }
    };
    void load();
    return () => { active = false; };
  }, [id, web]);

  useEffect(() => {
    let active = true;
    consultarRevisionHallazgo(id)
      .then(items => { if (active) setReviews(Object.fromEntries(items.map(item => [item.report_id + "|" + item.finding_key, { estado: item.estado, motivo: item.motivo ?? undefined }]))); })
      .catch(problem => { if (active) setError(errorAuditoriaEs(problem)); });
    return () => { active = false; };
  }, [id]);

  useEffect(() => {
    if (!reports.length) return;
    if (!reports.some(report => report.dispositivo === device) && reports.some(report => report.dispositivo !== device)) {
      setDevice(reports.some(report => report.dispositivo === "mobile") ? "mobile" : "desktop");
    }
  }, [reports, device]);

  const deviceReports = useMemo(() => {
    const latestByEngine = new Map<string, Report>();
    for (const report of reports.filter(item => {
      if (item.dispositivo !== device) return false;
      const result = item.resultado as any;
      return Object.keys(result?.scores ?? {}).length > 0 || (result?.issues?.length ?? 0) > 0;
    })) {
      const previous = latestByEngine.get(report.motor);
      if (!previous || report.fecha > previous.fecha) latestByEngine.set(report.motor, report);
    }
    return [...latestByEngine.values()];
  }, [reports, device]);

  const categoryScores = useMemo<CategoryScore[]>(() => SCORE_CATEGORIES.flatMap(category => {
    const values = deviceReports.map(report => normalizedScore(report, category)).filter((value): value is number => value !== null);
    return values.length ? [{ category, value: Math.min(...values) }] : [];
  }), [deviceReports]);

  const findings = useMemo(() => {
    const all: any[] = [];
    for (const report of deviceReports) {
      for (const item of (report.resultado as any)?.issues ?? []) {
        all.push({ ...item, _motor: report.motor, _device: report.dispositivo, _fecha: report.fecha, _version: report.version, _estado: report.estado, _reportId: report.id });
      }
    }
    const seen = new Set<string>();
    return all.filter(item => {
      const key = issueKey(item);
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    }).sort((a, b) => {
      const scoreA = categoryScores.find(score => score.category === a.category)?.value ?? 101;
      const scoreB = categoryScores.find(score => score.category === b.category)?.value ?? 101;
      return scoreA - scoreB || (SEVERITY_RANK[a.severity] ?? 5) - (SEVERITY_RANK[b.severity] ?? 5);
    });
  }, [deviceReports, categoryScores]);

  const selectionKey = `radar-dossier-${id}-${device}-${deviceReports.map(report => report.id).sort().join(",")}`;
  useEffect(() => {
    if (!deviceReports.length) { setSelection({ key: selectionKey, keys: [] }); return; }
    try {
      const previous = JSON.parse(localStorage.getItem(selectionKey) || "null");
      setSelection({ key: selectionKey, keys: Array.isArray(previous) ? previous : findings.slice(0, 5).map(issueKey) });
    } catch {
      setSelection({ key: selectionKey, keys: findings.slice(0, 5).map(issueKey) });
    }
  }, [selectionKey, findings]);
  useEffect(() => {
    if (selection?.key === selectionKey) localStorage.setItem(selectionKey, JSON.stringify(selection.keys));
  }, [selection, selectionKey]);
  const selected = selection?.key === selectionKey ? selection.keys : [];
  const setSelected = (update: (current: string[]) => string[]) => setSelection(current => ({ key: selectionKey, keys: update(current?.key === selectionKey ? current.keys : []) }));

  const exportDossier = async (print = false) => {
    try {
      const chosen = findings.filter(item => selected.includes(issueKey(item))).slice(0, 5);
      if (!chosen.length) throw new Error("Selecciona al menos un hallazgo para el dossier.");
      const cards = chosen.map(item => {
        const translated = traducirHallazgo(item);
        return `<article><h3>${esc(translated.titulo)}</h3><p><b>Prioridad:</b> ${esc(gravedadEs(item.severity))} · <b>Categoría:</b> ${esc(categoriaEs(item.category))}</p><p>${esc(translated.explicacion)}</p><p><b>Siguiente paso:</b> ${esc(translated.siguientePaso)}</p><small>${esc(item.url || web)} · ${esc(motorNombre(item._motor))} / ${esc(item._device === "mobile" ? "móvil" : "escritorio")}</small><details><summary>Ver detalle técnico original</summary><p>${esc(translated.original)}</p><p>${esc(item.details || "")}</p><p>${esc(item.recommendation || "")}</p></details></article>`;
      }).join("");
      const font = "data:font/woff2;base64," + await base64(await (await fetch("/fonts/albert-sans.woff2")).blob());
      const html = `<!doctype html><html lang="es"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Diagnóstico web · ${esc(name)}</title><style>@font-face{font-family:Albert Sans;src:url("${font}")}body{font:16px/1.5 Albert Sans,Arial,sans-serif;color:#1e2428;background:#f6f5ec;max-width:900px;margin:32px auto;padding:0 20px}header{border-bottom:2px solid #9ca59e;padding-bottom:18px}h1{font-size:2rem}article{border:1px solid #d2d3c3;border-radius:10px;padding:16px;margin:16px 0;background:#fbfaf3;break-inside:avoid}small,.muted{color:#414d4d}figure{margin:0}img{max-height:560px;max-width:260px;border:1px solid #d2d3c3}figcaption{font-size:.85rem;color:#414d4d;text-align:center}@media print{body{margin:10mm auto}a{color:inherit}}</style><body><header><p>Radar Navarra · Diagnóstico web</p><h1>${esc(name)}</h1><p><a href="${esc(web)}">${esc(web)}</a></p><p>Generado ${new Date().toLocaleString("es-ES")} · ${esc(deviceReports.length)} informes disponibles · ${chosen.length} hallazgos seleccionados</p></header><h2>Hallazgos para revisar</h2>${cards}<h2>Capturas para enseñar</h2><p class="muted">La presentación descargable incluye las pruebas visuales que hayas conservado en Preparar visita.</p><footer><p>Documento de trabajo; no es una certificación de accesibilidad, posicionamiento ni resultados comerciales. Alcance limitado a las páginas y perfiles registrados en Radar.</p></footer></body></html>`;
      if (print) {
        const win = window.open("", "_blank");
        if (!win) throw new Error("El navegador bloqueó la ventana de impresión. Permite ventanas emergentes para Radar.");
        win.document.open(); win.document.write(html); win.document.close(); win.focus();
        win.addEventListener("load", () => win.print(), { once: true });
      } else {
        const blob = new Blob([html], { type: "text/html;charset=utf-8" });
        const url = URL.createObjectURL(blob); const anchor = document.createElement("a");
        anchor.href = url; anchor.download = `diagnostico-web-${id}.html`; anchor.click();
        setTimeout(() => URL.revokeObjectURL(url), 1000);
      }
    } catch (problem) { setError(errorAuditoriaEs(problem)); }
  };

  const groups = new Map<string, any[]>();
  for (const item of findings) {
    const category = item.category || "best-practices";
    groups.set(category, [...(groups.get(category) || []), item]);
  }
  const hasDeviceReports = reports.some(report => report.dispositivo === device);

  if (!web) return <section class="diagnostico-web"><h3>Diagnóstico web</h3><p class="muted">Confirma una web propia para analizarla. Los enlaces sugeridos no se auditan hasta que los confirmes.</p></section>;

  return <section class="diagnostico-web">
    <header class="diagnostico-cabecera">
      <h3>Diagnóstico web</h3>
      <div class="diagnostico-web-toolbar">
        <a class="diagnostico-web-badge" href={web} target="_blank" rel="noreferrer" title="Abrir la web analizada"><span aria-hidden="true">↗</span> {web}</a>
        <div class="diagnostico-dispositivos" role="tablist" aria-label="Dispositivo del diagnóstico">
          {DEVICES.map(([value, label]) => <button type="button" key={value} role="tab" aria-selected={device === value} class={device === value ? "activo" : "secundario"} onClick={() => setDevice(value)}>{label}</button>)}
        </div>
      </div>
    </header>
    {status && <p class="muted diagnostico-estado">{status}</p>}
    {error && <p class="error">{error}</p>}
    {!reports.length ? <div class="diagnostico-vacio"><p>Aún no hay mediciones para la web confirmada.</p><p class="muted">Las webs confirmadas entran en la cola de auditoría automática. También puedes abrir Auditoría y lanzar una revisión manual.</p></div> : !hasDeviceReports ? <p class="diagnostico-vacio muted">Todavía no hay informes para {device === "mobile" ? "móvil" : "escritorio"}.</p> : <>
      <div class="diagnostico-resumen" aria-label="Puntuación por categoría">
        {categoryScores.map(({ category, value }) => <article class={`diagnostico-puntuacion ${value >= 80 ? "puntuacion-alta" : value < 50 ? "puntuacion-baja" : ""}`} key={category}>
          <span>{categoriaEs(category)}</span><strong>{value}/100</strong><div class="diagnostico-puntuacion-barra"><i style={{ width: `${value}%` }} /></div>
        </article>)}
      </div>
      <VisitaDiagnostico id={id} name={name} web={web} />
      <div class="acciones diagnostico-export">
        <button disabled={!findings.length || !selected.length} onClick={() => void exportDossier()}>Descargar selección de hallazgos HTML</button>
        <button class="secundario" disabled={!findings.length || !selected.length} onClick={() => void exportDossier(true)}>Imprimir / guardar PDF</button>
      </div>
      {findings.length ? Array.from(groups.entries()).map(([category, items]) => <section class="diagnostico-grupo" key={category}>
        <button type="button" class="diagnostico-grupo-titulo" aria-expanded={Boolean(openGroups[category])} aria-controls={`hallazgos-${id}-${category}`} onClick={() => setOpenGroups(current => ({ ...current, [category]: !current[category] }))}>
          <span>{categoriaEs(category)}</span><span class="muted">({items.length})</span><svg class="diagnostico-grupo-icono" viewBox="0 0 20 20" aria-hidden="true"><path d="m5 7.5 5 5 5-5" /></svg>
        </button>
        <div id={`hallazgos-${id}-${category}`} class={`diagnostico-grupo-contenido ${openGroups[category] ? "abierto" : ""}`} aria-hidden={!openGroups[category]} inert={!openGroups[category]}>
          <div class="diagnostico-grupo-interior">
        {items.slice(0, 30).map((item: any, index: number) => {
          const reviewKey = item._reportId + "|" + issueKey(item);
          const review = reviews[reviewKey] || { estado: "pendiente" };
          const translated = traducirHallazgo(item);
          const checked = selected.includes(issueKey(item));
          return <article class="diagnostico-hallazgo" key={issueKey(item) + index}>
            <div class="diagnostico-hallazgo-cabecera"><strong>{translated.titulo}</strong><span class={`diagnostico-gravedad gravedad-${item.severity || "info"}`}>{gravedadEs(item.severity)}</span></div>
            <div class="diagnostico-revision">
              <label>Revisión humana <select value={review.estado} onChange={async event => {
                const estado = event.currentTarget.value; const motivo = review.motivo || undefined;
                try { const saved = await guardarRevisionHallazgo(id, { report_id: item._reportId, finding_key: issueKey(item), estado, motivo }); setReviews(old => ({ ...old, [reviewKey]: { estado: saved.estado, motivo: saved.motivo ?? undefined } })); }
                catch { setError("No se pudo guardar la revisión. Inténtalo de nuevo."); }
              }}><option value="pendiente">Pendiente de contraste</option><option value="confirmado">Confirmado</option><option value="descartado">Descartado</option><option value="resuelto">Resuelto</option></select></label>
              {review.estado === "descartado" && <input aria-label="Motivo del descarte" placeholder="Motivo del descarte" value={review.motivo || ""} onChange={event => setReviews(old => ({ ...old, [reviewKey]: { ...review, motivo: event.currentTarget.value } }))} onBlur={async () => {
                try { await guardarRevisionHallazgo(id, { report_id: item._reportId, finding_key: issueKey(item), estado: review.estado, motivo: review.motivo }); }
                catch { setError("No se pudo guardar el motivo. Inténtalo de nuevo."); }
              }} />}
              <label class="diagnostico-incluir"><input type="checkbox" checked={checked} disabled={!checked && selected.length >= 5} onChange={event => {
                const key = issueKey(item);
                setSelected(current => event.currentTarget.checked ? current.includes(key) ? current : [...current, key] : current.filter(value => value !== key));
              }} />Incluir en la presentación</label>
            </div>
            <p>{translated.explicacion}</p><p class="muted">Siguiente paso: {translated.siguientePaso}</p>
            <small class="muted">{item.url || web} · {motorNombre(item._motor)} / {item._device === "mobile" ? "móvil" : "escritorio"} · {REVISION_ES[review.estado] || "Pendiente de contraste"}</small>
            <details class="diagnostico-original"><summary>Ver detalle técnico original</summary><p>{translated.original}</p>{item.details && <p>{item.details}</p>}{item.recommendation && <p>{item.recommendation}</p>}<small>Código: {item.code || "—"}</small></details>
          </article>;
        })}
          </div>
        </div>
      </section>) : <p>No se detectaron incidencias en los resultados disponibles. La cobertura incompleta y los avisos siguen sujetos a revisión.</p>}
    </>}
  </section>;
}
