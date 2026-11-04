import { useEffect, useState } from "preact/hooks";
import { autenticado, cargarSesion } from "./stores/auth";
import { recargar, recargarStats } from "./stores/negocios";
import { iniciarPolling } from "./stores/jobs";
import { Login } from "./components/Login";
import { TablaNegocios } from "./components/TablaNegocios";
import { FichaNegocio } from "./components/FichaNegocio";
import { PanelJobs } from "./components/PanelJobs";
import { MapaNegocios } from "./components/MapaNegocios";

type Vista = "tabla" | "mapa" | "jobs";

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
        {vista === "jobs" && <PanelJobs />}
      </main>
    </div>
  );
}
