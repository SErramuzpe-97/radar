import { signal, computed } from "@preact/signals";
import { api, authHeaders } from "../api/client";
import type { components } from "../api/schema";

export type Negocio = components["schemas"]["Negocio"];
export type PaginaNegocios = components["schemas"]["PaginaNegocios"];
export type NegocioDetalle = components["schemas"]["NegocioDetalle"];
export type Stats = components["schemas"]["Stats"];

export const filtros = signal({
  municipio: "",
  sector: "",
  estado_web: "",
  prioridad: "",
  q: "",
  ordenar: "score",
  dir: "desc",
  pagina: 1,
  tam: 50,
});

export const pagina = signal<PaginaNegocios | null>(null);
export const cargando = signal(false);
export const error = signal<string | null>(null);
export const detalle = signal<NegocioDetalle | null>(null);
export const stats = signal<Stats | null>(null);

export const seleccionadoId = signal<number | null>(null);
export const seleccionado = computed(() => {
  const id = seleccionadoId.value;
  return pagina.value?.items.find((n) => n.id === id) ?? null;
});

function queryLimpia() {
  const f = filtros.value;
  return {
    municipio: f.municipio || undefined,
    sector: f.sector || undefined,
    estado_web: f.estado_web || undefined,
    prioridad: f.prioridad || undefined,
    q: f.q || undefined,
    ordenar: f.ordenar || undefined,
    dir: f.dir || undefined,
    pagina: f.pagina,
    tam: f.tam,
  };
}

export async function recargar() {
  cargando.value = true;
  error.value = null;
  try {
    const { data, error: e } = await api.GET("/api/negocios", {
      params: { query: queryLimpia() },
      headers: authHeaders(),
    });
    if (e || !data) throw new Error((e as { error?: string })?.error ?? "Error cargando negocios");
    pagina.value = data;
  } catch (err) {
    error.value = String(err);
  } finally {
    cargando.value = false;
  }
}

export async function cargarDetalle(id: number) {
  seleccionadoId.value = id;
  const { data, error: e } = await api.GET("/api/negocios/{id}", {
    params: { path: { id } },
    headers: authHeaders(),
  });
  if (!e && data) detalle.value = data;
}

export async function recargarStats() {
  const { data } = await api.GET("/api/stats", { headers: authHeaders() });
  if (data) stats.value = data;
}

export async function guardarPipeline(id: number, body: components["schemas"]["SetPipelineRequest"]) {
  await api.PATCH("/api/negocios/{id}/pipeline", {
    params: { path: { id } },
    body,
    headers: authHeaders(),
  });
  await cargarDetalle(id);
}

export async function registrarEvento(
  id: number,
  body: components["schemas"]["NuevoEvento"],
) {
  await api.POST("/api/negocios/{id}/eventos", {
    params: { path: { id } },
    body,
    headers: authHeaders(),
  });
  await cargarDetalle(id);
}
