import { signal } from "@preact/signals";
import { api, authHeaders } from "../api/client";
import type { components } from "../api/schema";
import { recargar, recargarStats } from "./negocios";

export type Job = components["schemas"]["Job"];

export const jobs = signal<Job[]>([]);
export const cargando = signal(false);
export const error = signal<string | null>(null);

export async function recargarJobs() {
  const { data, error: e } = await api.GET("/api/jobs", { headers: authHeaders() });
  if (e) error.value = (e as { error?: string }).error ?? "Error";
  else if (data) jobs.value = data;
}

export async function lanzarScan(municipio: string, sector: string) {
  cargando.value = true;
  error.value = null;
  try {
    const { error: e } = await api.POST("/api/jobs/scan", {
      body: { municipio, sector: sector || null },
      headers: authHeaders(),
    });
    if (e) throw new Error((e as { error?: string }).error ?? "Error lanzando scan");
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
    await recargarJobs();
  } catch (err) {
    error.value = String(err);
  } finally {
    cargando.value = false;
  }
}

// Polling mientras haya jobs en curso; al terminar refresca tabla y stats.
let timer: number | null = null;
export function iniciarPolling() {
  if (timer !== null) return;
  timer = window.setInterval(async () => {
    await recargarJobs();
    const enCurso = jobs.value.some((j) => j.estado === "en_curso" || j.estado === "pendiente");
    if (!enCurso && jobs.value.length > 0) {
      await recargar();
      await recargarStats();
    }
  }, 2000);
}
