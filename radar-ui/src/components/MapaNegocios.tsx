import { useEffect, useRef, useState } from "preact/hooks";
import L from "leaflet";
import "leaflet/dist/leaflet.css";
import { cargandoMapa, errorMapa, filtros, negociosMapa, recargarMapa, cargarDetalle,
  cancelarCargaMapa, desactivarMapa, queryFiltrosMapa, cargarNegociosZona } from "../stores/negocios";
import type { PuntoMapa, ZonaMapa, PaginaNegocios } from "../stores/negocios";
import { RUBROS, TIPOS_ENTIDAD } from "../rubros";
import { Desplegable } from "./Desplegable";
import { transicionar } from "../transicion";

const SECTORES = ["", "hosteleria", "comercio", "servicios", "profesionales", "industria"];
const ICONOS: Record<string, string> = {
  hosteleria: "🍽️", comercio: "🛍️", servicios: "🛠️", profesionales: "💼", industria: "🏭",
};
const NOMBRES: Record<string, string> = {
  hosteleria: "Hostelería", comercio: "Comercio", servicios: "Servicios", profesionales: "Profesionales", industria: "Industria",
};

function clasePotencial(score?: number | null) {
  if (score == null) return "sin-datos";
  return score >= 70 ? "alto" : score >= 50 ? "medio" : "bajo";
}

function iconoNegocio(n: PuntoMapa) {
  const score = n.score_potencial;
  const sector = n.sector ?? "otro";
  const html = `<span class="marker-potential ${clasePotencial(score)}"><span class="marker-sector">${ICONOS[sector] ?? "📍"}</span></span>`;
  return L.divIcon({ className: "radar-marker", html, iconSize: [48, 58], iconAnchor: [24, 32], popupAnchor: [0, -28] });
}

// Leaflet recibe únicamente los puntos/agrupaciones de la zona visible.
export function MapaNegocios() {
  const ref = useRef<HTMLDivElement>(null);
  const mapRef = useRef<L.Map | null>(null);
  const markersRef = useRef<L.LayerGroup | null>(null);
  const programarCargaRef = useRef<() => void>(() => {});
  const [zonaSeleccionada, setZonaSeleccionada] = useState<ZonaMapa | null>(null);
  const [filtrosMapaAbiertos] = useState(() => !window.matchMedia("(max-width: 1199px), (any-pointer: coarse) and (max-width: 1600px)").matches);
  const f = filtros.value;
  const respuesta = negociosMapa.value;
  const claveFiltros = JSON.stringify(queryFiltrosMapa());

  useEffect(() => {
    if (!ref.current || mapRef.current) return;
    const map = L.map(ref.current, {
      minZoom: 3, maxZoom: 19,
      maxBounds: [[-85.05112878, -180], [85.05112878, 180]],
      maxBoundsViscosity: 1,
    });
    map.fitBounds([[41.88, -2.52], [43.34, -0.72]], { padding: [12, 12] });
    L.tileLayer("https://tile.openstreetmap.org/{z}/{x}/{y}.png", {
      maxZoom: 19,
      noWrap: true,
      attribution: "© OpenStreetMap contributors",
    }).addTo(map);
    mapRef.current = map;
    markersRef.current = L.layerGroup().addTo(map);
    let timer: number | undefined;
    const cancelar = () => {
      window.clearTimeout(timer);
      cancelarCargaMapa();
    };
    const cargarZona = () => {
      const b = map.getBounds();
      void recargarMapa({
        sur: Math.max(-85.05112878, b.getSouth()),
        oeste: Math.max(-180, b.getWest()),
        norte: Math.min(85.05112878, b.getNorth()),
        este: Math.min(180, b.getEast()),
        zoom: map.getZoom(),
      });
    };
    const programar = () => {
      cancelar();
      timer = window.setTimeout(cargarZona, 180);
    };
    programarCargaRef.current = programar;
    map.on("movestart", cancelar);
    map.on("moveend", programar);
    const observer = new ResizeObserver(() => map.invalidateSize({ pan: false }));
    observer.observe(ref.current);
    const frame = requestAnimationFrame(() => { map.invalidateSize({ pan: false }); programar(); });
    return () => {
      cancelar();
      cancelAnimationFrame(frame);
      desactivarMapa();
      observer.disconnect();
      programarCargaRef.current = () => {};
      map.remove();
      mapRef.current = null;
      markersRef.current = null;
    };
  }, []);

  useEffect(() => {
    setZonaSeleccionada(null);
    markersRef.current?.clearLayers();
    programarCargaRef.current();
  }, [claveFiltros]);

  useEffect(() => {
    const map = mapRef.current;
    const layer = markersRef.current;
    if (!map || !layer) return;
    layer.clearLayers();
    // El servidor acota esta lista a 825 elementos, independientemente del total.
    for (const n of respuesta?.items ?? []) {
      if (n.cantidad === 1 && n.id != null) {
        const marker = L.marker([n.lat, n.lon], { icon: iconoNegocio(n), title: n.nombre ?? "Empresa" });
        const tooltip = document.createElement("span");
        tooltip.textContent = `${n.nombre} · ${n.score_potencial == null ? "sin auditar" : `${n.score_potencial}/100`}`;
        marker.bindTooltip(tooltip);
        marker.on("click", () => { void cargarDetalle(n.id!); });
        marker.addTo(layer);
        continue;
      }
      const cluster = L.marker([n.lat, n.lon], {
        icon: L.divIcon({ className: "radar-cluster", html: `<span class="cluster-count ${clasePotencial(n.score_potencial)}">${n.cantidad}</span>`, iconSize: [46, 46], iconAnchor: [23, 23] }),
        title: `${n.cantidad} empresas`,
      });
      cluster.bindTooltip(`${n.cantidad} empresas en esta zona. Pulsa para explorar.`);
      cluster.on("click", () => {
        const z = n.zona;
        // La lista permite abrir también empresas que comparten coordenadas.
        transicionar(() => setZonaSeleccionada(z));
        if (map.getZoom() < 19) {
          const bounds = L.latLngBounds([[z.sur, z.oeste], [z.norte, z.este]]);
          map.fitBounds(bounds, { padding: [36, 36], maxZoom: Math.min(19, map.getZoom() + 3) });
        }
      });
      cluster.addTo(layer);
    }
  }, [respuesta]);

  return (
    <section class="mapa-panel">
      <Desplegable className="mapa-filtros-panel" titulo="Filtros" inicialAbierto={filtrosMapaAbiertos}>
       <div class="mapa-controles">
        <input aria-label="Filtrar municipios del mapa" placeholder="Municipio…" value={f.municipio}
          onInput={(e) => { filtros.value = { ...filtros.value, municipio: (e.target as HTMLInputElement).value }; }} />
        <select aria-label="Filtrar sector del mapa" value={f.sector}
          onChange={(e) => { filtros.value = { ...filtros.value, sector: (e.target as HTMLSelectElement).value }; }}>
          {SECTORES.map((s) => <option value={s}>{s ? NOMBRES[s] : "Todos los sectores"}</option>)}
        </select>
        <select aria-label="Filtrar rubro del mapa" value={f.rubro}
          onChange={(e) => { filtros.value = { ...filtros.value, rubro: (e.target as HTMLSelectElement).value }; }}>
          {RUBROS.map(([v, label]) => <option value={v}>{v ? label : "Todos los rubros"}</option>)}
        </select>
        <select aria-label="Filtrar tipo de entidad del mapa" value={f.tipo_entidad}
          onChange={(e) => { filtros.value = { ...filtros.value, tipo_entidad: (e.target as HTMLSelectElement).value }; }}>
          {TIPOS_ENTIDAD.map(([v, label]) => <option value={v}>{label}</option>)}
        </select>
        <select aria-label="Filtrar prioridad del mapa" value={f.prioridad}
          onChange={(e) => { filtros.value = { ...filtros.value, prioridad: (e.target as HTMLSelectElement).value }; }}>
          <option value="">Todas las prioridades</option><option value="A">Potencial alto</option><option value="B">Potencial medio</option><option value="C">Potencial bajo</option>
        </select>
        <button class="secundario" disabled={cargandoMapa.value} onClick={() => programarCargaRef.current()}>
          {cargandoMapa.value ? "Actualizando…" : "Actualizar mapa"}
        </button>
       </div>
      </Desplegable>
      <div class="mapa-resumen muted">{respuesta?.total.toLocaleString("es-ES") ?? "…"} empresas en la zona visible</div>
      {errorMapa.value && <p class="error" role="alert">{errorMapa.value}</p>}
      <div class="mapa-leyenda">
        <span><i class="leyenda-alto" /> Potencial alto</span>
        <span><i class="leyenda-medio" /> Medio</span>
        <span><i class="leyenda-bajo" /> Bajo</span>
        <span><i class="leyenda-sin-datos" /> Sin auditar</span>
        <span class="muted">El icono indica el sector</span>
      </div>
      {zonaSeleccionada && <EmpresasZona key={JSON.stringify(zonaSeleccionada) + claveFiltros} zona={zonaSeleccionada} claveFiltros={claveFiltros} cerrar={() => setZonaSeleccionada(null)} />}
      <div class="mapa-visualizacion">
        <div class="mapa" ref={ref} />
        {cargandoMapa.value && <div class="mapa-estado" role="status" aria-live="polite"><span class="carga-spinner"/><span>Cargando esta zona…</span></div>}
        {!cargandoMapa.value && respuesta?.total === 0 && !errorMapa.value && <div class="mapa-estado" role="status">No hay empresas con ubicación en esta zona y con estos filtros.</div>}
      </div>
    </section>
  );
}

function EmpresasZona({ zona, claveFiltros, cerrar }: { zona: ZonaMapa; claveFiltros: string; cerrar: () => void }) {
  const [numero, setNumero] = useState(1);
  const [datos, setDatos] = useState<PaginaNegocios | null>(null);
  const [cargando, setCargando] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const claveZona = JSON.stringify(zona);
  useEffect(() => {
    const controller = new AbortController();
    setCargando(true);
    setDatos(null);
    setError(null);
    cargarNegociosZona(zona, numero, controller.signal)
      .then((pagina) => { if (!controller.signal.aborted) setDatos(pagina); })
      .catch((err) => { if (!controller.signal.aborted) setError(String(err)); })
      .finally(() => { if (!controller.signal.aborted) setCargando(false); });
    return () => controller.abort();
  }, [claveZona, claveFiltros, numero]);
  return <div class="mapa-empresas-zona">
    <div class="mapa-zona-cabecera"><strong>Empresas de esta zona{datos ? ` (${datos.total.toLocaleString("es-ES")})` : ""}</strong><button class="secundario" onClick={cerrar}>Cerrar lista</button></div>
    {cargando && <p role="status">Cargando empresas…</p>}
    {error && <p class="error" role="alert">{error}</p>}
    {datos && <>
      <div class="mapa-zona-lista">{datos.items.map((n) => <button key={n.id} class="secundario" onClick={() => void cargarDetalle(n.id)}>{n.nombre}<small>{n.municipio}{n.direccion ? ` · ${n.direccion}` : ""}</small></button>)}</div>
      <div class="paginacion"><button class="secundario" disabled={numero <= 1} onClick={() => setNumero(numero - 1)}>Anterior</button><span>Página {numero} de {Math.max(1, Math.ceil(datos.total / datos.tam))}</span><button class="secundario" disabled={numero * datos.tam >= datos.total} onClick={() => setNumero(numero + 1)}>Siguiente</button></div>
    </>}
  </div>;
}
