import { useEffect, useRef } from "preact/hooks";
import L from "leaflet";
import "leaflet/dist/leaflet.css";
import { pagina, cargarDetalle } from "../stores/negocios";

// Mapa Leaflet con teselas libres OSM (atribución obligatoria ODbL).
export function MapaNegocios() {
  const ref = useRef<HTMLDivElement>(null);
  const mapRef = useRef<L.Map | null>(null);

  useEffect(() => {
    if (!ref.current || mapRef.current) return;
    const map = L.map(ref.current).setView([42.8169, -1.6441], 13); // Pamplona
    L.tileLayer("https://tile.openstreetmap.org/{z}/{x}/{y}.png", {
      maxZoom: 19,
      attribution: "© OpenStreetMap contributors",
    }).addTo(map);
    mapRef.current = map;
  }, []);

  useEffect(() => {
    const map = mapRef.current;
    const items = pagina.value?.items ?? [];
    if (!map) return;
    map.eachLayer((layer) => {
      if (layer instanceof L.Marker) map.removeLayer(layer);
    });
    for (const n of items) {
      if (n.lat == null || n.lon == null) continue;
      L.marker([n.lat, n.lon]).addTo(map).bindPopup(n.nombre).on("click", () => cargarDetalle(n.id));
    }
  }, [pagina.value]);

  return <div class="mapa" ref={ref} />;
}
