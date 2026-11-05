import { signal } from "@preact/signals";
import { api, authHeaders } from "../api/client";
import type { components } from "../api/schema";

export type Plantilla = components["schemas"]["PlantillaContacto"];
export type Tarea = components["schemas"]["TareaSeguimiento"];
export const plantillasContacto = signal<Plantilla[]>([]);
export const tareasPendientes = signal<Tarea[]>([]);

export async function cargarPlantillas() {
  const { data } = await api.GET("/api/plantillas", { headers: authHeaders() });
  if (data) plantillasContacto.value = data;
}

export async function cargarTareas() {
  const { data } = await api.GET("/api/tareas", { params: { query: { hasta: "2099-12-31" } }, headers: authHeaders() });
  if (data) tareasPendientes.value = data;
}

export async function crearPlantilla(body: components["schemas"]["NuevaPlantillaContacto"]) {
  const { error } = await api.POST("/api/plantillas", { body, headers: authHeaders() });
  if (error) throw new Error("No se pudo guardar la plantilla");
  await cargarPlantillas();
}
