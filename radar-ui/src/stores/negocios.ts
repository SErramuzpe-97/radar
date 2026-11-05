import { signal, computed } from "@preact/signals";
import { api, authHeaders } from "../api/client";
import type { components } from "../api/schema";
import { transicionar } from "../transicion";

export type Negocio = components["schemas"]["Negocio"];
export type PaginaNegocios = components["schemas"]["PaginaNegocios"];
export type NegocioDetalle = components["schemas"]["NegocioDetalle"];
export type Stats = components["schemas"]["Stats"];

export const filtros = signal({
  municipio: "",
  sector: "",
  rubro: "",
  tipo_entidad: "",
  estado_web: "",
  estado_auditoria_tecnica: "",
  estado_pipeline: "",
  prioridad: "",
  q: "",
  ordenar: "score",
  dir: "desc",
  pagina: 1,
  tam: 50,
  mostrar_descartados: false,
});

export const pagina = signal<PaginaNegocios | null>(null);
export type ZonaMapa = components["schemas"]["ZonaMapa"];
export type PuntoMapa = components["schemas"]["PuntoMapa"];
export type VistaMapa = ZonaMapa & { zoom: number };
export const negociosMapa = signal<components["schemas"]["RespuestaMapa"] | null>(null);
export const cargandoMapa = signal(false);
export const errorMapa = signal<string | null>(null);
export const cargando = signal(false);
export const error = signal<string | null>(null);
export const detalle = signal<NegocioDetalle | null>(null);
export const stats = signal<Stats | null>(null);
let listaRequest = 0;
let listaController: AbortController | null = null;
let listaTimer: number | undefined;
let mapaRequest = 0;
let mapaController: AbortController | null = null;
let vistaMapa: VistaMapa | null = null;

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
    rubro: f.rubro || undefined,
    tipo_entidad: f.tipo_entidad || undefined,
    estado_web: f.estado_web || undefined,
    estado_auditoria_tecnica: f.estado_auditoria_tecnica || undefined,
    estado_pipeline: f.estado_pipeline || undefined,
    prioridad: f.prioridad || undefined,
    q: f.q || undefined,
    ordenar: f.ordenar || undefined,
    dir: f.dir || undefined,
    pagina: f.pagina,
    tam: f.tam,
    mostrar_descartados: f.mostrar_descartados || undefined,
  };
}

export function cancelarCargaLista() {
  window.clearTimeout(listaTimer);
  listaTimer = undefined;
  ++listaRequest;
  listaController?.abort();
  listaController = null;
  cargando.value = false;
}

export function programarRecarga() {
  cancelarCargaLista();
  cargando.value = true;
  error.value = null;
  listaTimer = window.setTimeout(() => { void recargar(); }, 250);
}

export async function recargar() {
  cancelarCargaLista();
  const request = ++listaRequest;
  const controller = new AbortController();
  listaController = controller;
  cargando.value = true;
  error.value = null;
  try {
    const { data, error: e } = await api.GET("/api/negocios", {
      params: { query: queryLimpia() },
      headers: authHeaders(),
      signal: controller.signal,
    });
    if (e || !data) throw new Error((e as { error?: string })?.error ?? "Error cargando negocios");
    if (request === listaRequest) pagina.value = data;
  } catch (err) {
    if (!controller.signal.aborted && request === listaRequest) error.value = String(err);
  } finally {
    if (request === listaRequest) {
      cargando.value = false;
      listaController = null;
    }
  }
}

export function cancelarCargaMapa() {
  ++mapaRequest;
  mapaController?.abort();
  mapaController = null;
  cargandoMapa.value = false;
}

export function desactivarMapa() {
  cancelarCargaMapa();
  vistaMapa = null;
  negociosMapa.value = null;
}

export function queryFiltrosMapa() {
  const { pagina: _pagina, tam: _tam, ordenar: _ordenar, dir: _dir, ...query } = queryLimpia();
  return query;
}

export async function cargarNegociosZona(zona: ZonaMapa, numero: number, signal: AbortSignal) {
  const { data, error: e } = await api.GET("/api/negocios", {
    params: { query: { ...queryFiltrosMapa(), ...zona, pagina: numero, tam: 20 } },
    headers: authHeaders(),
    signal,
  });
  if (e || !data) throw new Error((e as { error?: string })?.error ?? "Error cargando empresas de la zona");
  return data;
}

export async function recargarMapa(vista?: VistaMapa) {
  if (vista) vistaMapa = vista;
  // Las actualizaciones de fichas/jobs no descargan mapas que nadie está viendo.
  if (!vistaMapa) return;
  cancelarCargaMapa();
  const request = ++mapaRequest;
  const controller = new AbortController();
  mapaController = controller;
  cargandoMapa.value = true;
  errorMapa.value = null;
  try {
    const { data, error: e } = await api.GET("/api/negocios/mapa", {
      params: { query: { ...queryFiltrosMapa(), ...vistaMapa } },
      headers: authHeaders(),
      signal: controller.signal,
    });
    if (e || !data) throw new Error((e as { error?: string })?.error ?? "Error cargando negocios del mapa");
    if (request === mapaRequest) negociosMapa.value = data;
  } catch (err) {
    if (!controller.signal.aborted && request === mapaRequest) errorMapa.value = String(err);
  } finally {
    if (request === mapaRequest) {
      cargandoMapa.value = false;
      mapaController = null;
    }
  }
}

export async function cargarDetalle(id: number) {
  seleccionadoId.value = id;
  const { data, error: e } = await api.GET("/api/negocios/{id}", {
    params: { path: { id } },
    headers: authHeaders(),
  });
  if (!e && data && seleccionadoId.value === id) transicionar(() => { detalle.value = data; });
  else if (e && seleccionadoId.value === id) transicionar(() => { detalle.value=null; seleccionadoId.value=null; error.value="La ficha ya no está disponible. Actualiza los resultados."; });
}

export async function recargarStats() {
  const { data } = await api.GET("/api/stats", { headers: authHeaders() });
  if (data) stats.value = data;
}

export async function guardarPipeline(id: number, body: components["schemas"]["SetPipelineRequest"]) {
  const { error: e } = await api.PATCH("/api/negocios/{id}/pipeline", {
    params: { path: { id } },
    body,
    headers: authHeaders(),
  });
  if (e) throw new Error((e as { error?: string }).error ?? "No se pudo guardar el estado");
  if (seleccionadoId.value === id) await cargarDetalle(id);
}

export async function guardarEstadoOportunidad(id: number, servicio: components["schemas"]["ServicioWeb"], estado: components["schemas"]["EstadoOportunidad"]) {
  await api.PATCH("/api/negocios/{id}/oportunidades/{servicio}", {
    params: { path: { id, servicio } },
    body: { estado },
    headers: authHeaders(),
  });
  await Promise.all([cargarDetalle(id), recargar(), recargarMapa()]);
}

export async function actualizarDescartados(ids: number[], descartado: boolean) {
  const { error: e } = await api.PATCH("/api/negocios/descartados", {
    body: { ids, descartado },
    headers: authHeaders(),
  });
  if (e) throw new Error((e as { error?: string }).error ?? "No se pudo actualizar la selección");
  await Promise.all([recargar(), recargarMapa(), recargarStats()]);
}

export async function registrarEvento(
  id: number,
  body: components["schemas"]["NuevoEvento"],
) {
  const { error: e } = await api.POST("/api/negocios/{id}/eventos", {
    params: { path: { id } },
    body,
    headers: authHeaders(),
  });
  if (e) throw new Error((e as { error?: string }).error ?? "No se pudo guardar la actividad");
  if (seleccionadoId.value === id) await cargarDetalle(id);
}

export async function guardarBorrador(id: number, body: components["schemas"]["NuevoBorrador"]) {
  const { error: e } = await api.POST("/api/negocios/{id}/borradores", {
    params: { path: { id } }, body, headers: authHeaders(),
  });
  if (e) throw new Error("No se pudo guardar el borrador");
  await cargarDetalle(id);
}
