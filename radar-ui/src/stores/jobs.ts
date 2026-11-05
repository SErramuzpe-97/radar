import { signal } from "@preact/signals";
import { api, authHeaders } from "../api/client";
import type { components } from "../api/schema";
import { recargar, recargarMapa, recargarStats } from "./negocios";

export type Job = components["schemas"]["Job"];

export const jobs = signal<Job[]>([]);
export const cargando = signal(false);
export const error = signal<string | null>(null);
let actualizarDatosAlFinal = false;

export async function recargarJobs() {
  const { data, error: e } = await api.GET("/api/jobs", { headers: authHeaders() });
  if (e) error.value = (e as { error?: string }).error ?? "Error";
  else if (data) jobs.value = data;
}

export async function lanzarScan(municipios: string[], sector: string, navarra = false) {
  cargando.value = true;
  error.value = null;
  try {
    const { error: e } = await api.POST("/api/jobs/scan", {
      body: { navarra, municipios, sector: sector || null },
      headers: authHeaders(),
    });
    if (e) throw new Error((e as { error?: string }).error ?? "Error lanzando scan");
    actualizarDatosAlFinal = true;
    await recargarJobs();
  } catch (err) {
    error.value = String(err);
  } finally {
    cargando.value = false;
  }
}

export async function lanzarAudit(prioridad: string, limite: number | null) {
  cargando.value = true;
  error.value = null;
  try {
    const { error: e } = await api.POST("/api/jobs/audit", {
      body: { prioridad: prioridad || null, limite },
      headers: authHeaders(),
    });
    if (e) throw new Error((e as { error?: string }).error ?? "Error lanzando auditoría");
    actualizarDatosAlFinal = true;
    await recargarJobs();
  } catch (err) {
    error.value = String(err);
  } finally {
    cargando.value = false;
  }
}

export async function reintentarJob(id: string) {
  cargando.value = true;
  error.value = null;
  try {
    const { error: e } = await api.POST("/api/jobs/{id}/retry", {
      params: { path: { id } },
      headers: authHeaders(),
    });
    if (e) throw new Error((e as { error?: string }).error ?? "Error reintentando job");
    await recargarJobs();
  } catch (err) {
    error.value = String(err);
  } finally {
    cargando.value = false;
  }
}

// Actualiza el historial cada 2 s y los datos solo una vez al terminar los jobs.
let timer: number | null = null;
export function iniciarPolling() {
  if (timer !== null) return;
  timer = window.setInterval(async () => {
    await recargarJobs();
    const enCurso = jobs.value.some((j) => j.estado === "en_curso" || j.estado === "pendiente");
    if (enCurso) actualizarDatosAlFinal = true;
    else if (actualizarDatosAlFinal) {
      actualizarDatosAlFinal = false;
      await Promise.all([recargar(), recargarMapa(), recargarStats()]);
    }
  }, 2000);
}
