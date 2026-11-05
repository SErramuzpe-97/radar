import { useEffect, useState } from "preact/hooks";
import { autenticado, cargarSesion, logout, sesionEmail } from "./stores/auth";
import { recargar, recargarStats, detalle } from "./stores/negocios";
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
  const negocioAbierto = Boolean(detalle.value) && (vista === "tabla" || vista === "mapa");

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
    <div class={`app ${negocioAbierto ? "negocio-abierto" : ""}`}>
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
            <FichaNegocio origen="tabla" />
          </div>
        )}
        {vista === "mapa" && <div class="mapa-layout"><MapaNegocios /><FichaNegocio origen="mapa" /></div>}
        {vista === "ruta" && <RutaDia />}
        {vista === "stats" && <PanelStats />}
        {vista === "jobs" && <PanelJobs />}
        {vista === "fuentes" && <PanelFuentes />}
        {vista === "seguimiento" && <PanelSeguimiento abrirNegocio={() => setVista("tabla")} />}
      </main>
      <nav class="nav-inferior" aria-label="Navegación principal">
        <button type="button" class={vista === "tabla" ? "activo" : ""} aria-current={vista === "tabla" ? "page" : undefined} onClick={() => { setVista("tabla"); void recargar(); }}>
          <span class="nav-inferior-icono" aria-hidden="true"><svg viewBox="0 0 24 24"><rect x="3.5" y="4" width="17" height="16" rx="2"/><path d="M3.5 9h17M9 9v11"/></svg></span><span>Tabla</span>
        </button>
        <button type="button" class={vista === "mapa" ? "activo" : ""} aria-current={vista === "mapa" ? "page" : undefined} onClick={() => setVista("mapa")}>
          <span class="nav-inferior-icono" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="m3.5 6 5-2 7 2 5-2v14l-5 2-7-2-5 2V6Z"/><path d="M8.5 4v14M15.5 6v14"/><circle cx="12" cy="10" r="2.2"/></svg></span><span>Mapa</span>
        </button>
        <button type="button" onClick={() => void logout()}>
          <span class="nav-inferior-icono" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="M10 4H5.5A1.5 1.5 0 0 0 4 5.5v13A1.5 1.5 0 0 0 5.5 20H10"/><path d="M14 8l4 4-4 4M8 12h10"/></svg></span><span>Cerrar sesión</span>
        </button>
      </nav>
    </div>
  );
}
