import { signal } from "@preact/signals";
import { api, authHeaders } from "../api/client";
import type { components } from "../api/schema";

export type Fuente = components["schemas"]["Fuente"];
export type FuenteCandidata = components["schemas"]["FuenteCandidata"];
export type Duplicado = components["schemas"]["CandidatoDuplicado"];
export type Cobertura = components["schemas"]["CoberturaMunicipio"];
export const fuentes = signal<Fuente[]>([]);
export const candidatas = signal<FuenteCandidata[]>([]);
export const duplicados = signal<Duplicado[]>([]);
export const cobertura = signal<Cobertura[]>([]);
export const resumenFiltro = signal<components["schemas"]["ResumenClasificacion"] | null>(null);
export const errorFuentes = signal<string | null>(null);
const headers = () => ({ headers: authHeaders() });

export async function cargarFuentes() {
  errorFuentes.value = null;
  const [f, c, d, cov, clasif] = await Promise.all([
    api.GET("/api/fuentes", headers()),
    api.GET("/api/fuentes/candidatas", headers()),
    api.GET("/api/duplicados", headers()),
    api.GET("/api/cobertura", headers()),
    api.GET("/api/clasificacion", headers()),
  ]);
  if (f.data) fuentes.value = f.data;
  if (c.data) candidatas.value = c.data;
  if (d.data) duplicados.value = d.data;
  if (cov.data) cobertura.value = cov.data;
  if (clasif.data) resumenFiltro.value = clasif.data;
}

export async function crearFuente(body: components["schemas"]["NuevaFuente"]) {
  const r = await api.POST("/api/fuentes", { body, ...headers() });
  const failure = (r as unknown as { error?: { error?: string } }).error;
  if (failure) throw new Error(failure.error ?? "No se pudo guardar la fuente");
  await cargarFuentes();
}

export async function cambiarFuente(id: number, habilitada: boolean) {
  const r = await api.PATCH("/api/fuentes/{id}", { params: { path: { id } }, body: { habilitada }, ...headers() });
  if (r.error) throw new Error("No se pudo cambiar la fuente");
  await cargarFuentes();
}

export async function lanzarRastreo(fuente_id: number) {
  const fuente=fuentes.value.find(f=>f.id===fuente_id);
  const r = fuente?.tipo === "dataset" && fuente.url.endsWith("/navarra-latest.osm.pbf")
    ? await api.POST("/api/jobs/scan", {body:{navarra:true},...headers()})
    : fuente?.tipo === "dataset" && fuente.url === "https://www.boe.es/datosabiertos/api/borme/sumario"
    ? await api.POST("/api/jobs/watch", {body:{fuente:"borme"},...headers()})
    : await api.POST("/api/jobs/crawl", { body: { fuente_id }, ...headers() });
  const failure = (r as unknown as { error?: { error?: string } }).error;
  if (failure) throw new Error(failure.error ?? "No se pudo lanzar el rastreo");
}

export async function resolverCandidata(id: number, estado: "aprobada" | "descartada") {
  const r = await api.PATCH("/api/fuentes/candidatas/{id}", { params: { path: { id } }, body: { estado }, ...headers() });
  if (r.error) throw new Error("No se pudo actualizar la candidata");
  await cargarFuentes();
}

export async function fusionar(duplicado: Duplicado, conservar: "a" | "b") {
  const conservar_id = conservar === "a" ? duplicado.negocio_a : duplicado.negocio_b;
  const fusionar_id = conservar === "a" ? duplicado.negocio_b : duplicado.negocio_a;
  const r = await api.POST("/api/duplicados/{id}/fusionar", { params: { path: { id: duplicado.id } }, body: { conservar_id, fusionar_id }, ...headers() });
  if (r.error) throw new Error("No se pudo fusionar las fichas");
  await cargarFuentes();
}

export async function descartarDuplicado(id: number) {
  const r = await api.DELETE("/api/duplicados/{id}", { params: { path: { id } }, ...headers() });
  if (r.error) throw new Error("No se pudo descartar la sugerencia");
  await cargarFuentes();
}
