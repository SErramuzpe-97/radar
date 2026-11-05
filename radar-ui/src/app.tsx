import { useEffect, useState } from "preact/hooks";
import { autenticado, cargarSesion, logout, sesionEmail } from "./stores/auth";
import { recargar, recargarStats } from "./stores/negocios";
import { iniciarPolling } from "./stores/jobs";
import { Login } from "./components/Login";
import { TablaNegocios } from "./components/TablaNegocios";
import { FichaNegocio } from "./components/FichaNegocio";
import { PanelJobs } from "./components/PanelJobs";
import { MapaNegocios } from "./components/MapaNegocios";
import { RutaDia } from "./components/RutaDia";
import { PanelStats } from "./components/PanelStats";
import { PanelFuentes } from "./components/PanelFuentes";
import { PanelSeguimiento } from "./components/PanelSeguimiento";

type Vista = "tabla" | "mapa" | "ruta" | "stats" | "jobs" | "fuentes" | "seguimiento";

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
          <button class={vista === "tabla" ? "activo" : ""} onClick={() => { setVista("tabla"); void recargar(); }}>Tabla</button>
          <button class={vista === "mapa" ? "activo" : ""} onClick={() => setVista("mapa")}>Mapa</button>
          <button class={vista === "ruta" ? "activo" : ""} onClick={() => setVista("ruta")}>Ruta del día</button>
          <button class={vista === "stats" ? "activo" : ""} onClick={() => setVista("stats")}>Panel</button>
          <button class={vista === "jobs" ? "activo" : ""} onClick={() => setVista("jobs")}>Buscar empresas</button>
          <button class={vista === "fuentes" ? "activo" : ""} onClick={() => setVista("fuentes")}>Fuentes</button>
          <button class={vista === "seguimiento" ? "activo" : ""} onClick={() => setVista("seguimiento")}>Seguimiento</button>
        </nav>
        <select class="nav-compacta" aria-label="Vista" value={vista} onChange={e => { const v=(e.target as HTMLSelectElement).value as Vista; setVista(v); if(v==="tabla") void recargar(); }}>
          {Object.entries({tabla:"Tabla",mapa:"Mapa",ruta:"Ruta del día",stats:"Panel",jobs:"Buscar empresas",fuentes:"Fuentes",seguimiento:"Seguimiento"}).map(([v,l]) => <option key={v} value={v}>{l}</option>)}
        </select>
        <div class="sesion">
          <span class="muted">{sesionEmail.value}</span>
          <button class="secundario" onClick={() => void logout()}>Cerrar sesión</button>
        </div>
      </header>
      <main class={`vista-${vista}`}>
        {vista === "tabla" && (
          <div class="split">
            <TablaNegocios />
            <FichaNegocio />
          </div>
        )}
        {vista === "mapa" && <div class="mapa-layout"><MapaNegocios /><FichaNegocio /></div>}
        {vista === "ruta" && <RutaDia />}
        {vista === "stats" && <PanelStats />}
        {vista === "jobs" && <PanelJobs />}
        {vista === "fuentes" && <PanelFuentes />}
        {vista === "seguimiento" && <PanelSeguimiento abrirNegocio={() => setVista("tabla")} />}
      </main>
    </div>
  );
}
