import { useEffect, useState } from "preact/hooks";
import { autenticado, cargarSesion } from "./stores/auth";
import { recargar, recargarStats } from "./stores/negocios";
import { iniciarPolling } from "./stores/jobs";
import { Login } from "./components/Login";
import { TablaNegocios } from "./components/TablaNegocios";
import { FichaNegocio } from "./components/FichaNegocio";
import { PanelJobs } from "./components/PanelJobs";
import { MapaNegocios } from "./components/MapaNegocios";
import { RutaDia } from "./components/RutaDia";
import { PanelStats } from "./components/PanelStats";

type Vista = "tabla" | "mapa" | "ruta" | "stats" | "jobs";

export function App() {
  const [vista, setVista] = useState<Vista>("tabla");

  useEffect(() => {
    cargarSesion();
  }, []);

  useEffect(() => {
    if (!autenticado.value) return;
    recargar();
    recargarStats();
    iniciarPolling();
  }, [autenticado.value]);

  if (!autenticado.value) return <Login />;

  return (
    <div class="app">
      <header class="topbar">
        <strong>Radar Navarra</strong>
        <nav>
          <button class={vista === "tabla" ? "activo" : ""} onClick={() => setVista("tabla")}>Tabla</button>
          <button class={vista === "mapa" ? "activo" : ""} onClick={() => setVista("mapa")}>Mapa</button>
          <button class={vista === "ruta" ? "activo" : ""} onClick={() => setVista("ruta")}>Ruta del día</button>
          <button class={vista === "stats" ? "activo" : ""} onClick={() => setVista("stats")}>Panel</button>
          <button class={vista === "jobs" ? "activo" : ""} onClick={() => setVista("jobs")}>Búsquedas</button>
        </nav>
      </header>
      <main>
        {vista === "tabla" && (
          <div class="split">
            <TablaNegocios />
            <FichaNegocio />
          </div>
        )}
        {vista === "mapa" && <MapaNegocios />}
        {vista === "ruta" && <RutaDia />}
        {vista === "stats" && <PanelStats />}
        {vista === "jobs" && <PanelJobs />}
      </main>
    </div>
  );
}
