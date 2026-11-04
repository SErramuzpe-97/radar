import { useState } from "preact/hooks";
import { pagina, recargar, type Negocio } from "../stores/negocios";

function xmlSeguro(texto: string) {
  return texto.replace(/[&<>\"']/g, (caracter) => ({
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&apos;",
  })[caracter] ?? caracter);
}

function descargarGpx(negocios: Negocio[]) {
  const waypoints = negocios.map((negocio) =>
    `  <wpt lat="${negocio.lat}" lon="${negocio.lon}"><name>${xmlSeguro(negocio.nombre)}</name><desc>${xmlSeguro(`${negocio.direccion ?? ""} · ${negocio.municipio}`)}</desc></wpt>`,
  ).join("\n");
  const gpx = `<?xml version="1.0" encoding="UTF-8"?>\n<gpx version="1.1" creator="Radar Navarra" xmlns="http://www.topografix.com/GPX/1/1">\n${waypoints}\n</gpx>\n`;
  const url = URL.createObjectURL(new Blob([gpx], { type: "application/gpx+xml;charset=utf-8" }));
  const enlace = document.createElement("a");
  enlace.href = url;
  enlace.download = "radar-ruta-del-dia.gpx";
  enlace.click();
  URL.revokeObjectURL(url);
}

function enlaceMaps(negocios: Negocio[]) {
  const punto = (negocio: Negocio) => `${negocio.lat},${negocio.lon}`;
  const origen = punto(negocios[0]);
  const destino = punto(negocios[negocios.length - 1]);
  const parametros = new URLSearchParams({ api: "1", origin: origen, destination: destino });
  if (negocios.length > 2) parametros.set("waypoints", negocios.slice(1, -1).map(punto).join("|"));
  return `https://www.google.com/maps/dir/?${parametros.toString()}`;
}

export function RutaDia() {
  const [orden, setOrden] = useState<number[]>([]);
  const negocios = pagina.value?.items ?? [];
  const conCoordenadas = negocios.filter((n) => Number.isFinite(n.lat) && Number.isFinite(n.lon));
  const seleccionados = orden
    .map((id) => conCoordenadas.find((negocio) => negocio.id === id))
    .filter((negocio): negocio is Negocio => Boolean(negocio));

  function alternar(id: number) {
    setOrden((actual) => actual.includes(id) ? actual.filter((seleccionado) => seleccionado !== id) : [...actual, id]);
  }

  return (
    <section class="ruta-dia">
      <div class="ruta-cabecera">
        <div>
          <h1>Ruta del día</h1>
          <p class="muted">Elige paradas de los negocios de la página actual. El orden de selección define el recorrido.</p>
        </div>
        <button class="secundario" onClick={() => recargar()}>Actualizar resultados</button>
      </div>

      {negocios.length === 0 ? (
        <p class="muted">No hay negocios cargados. Vuelve a la tabla para cargar resultados.</p>
      ) : (
        <>
          <p class="muted">{conCoordenadas.length} de {negocios.length} negocios tienen coordenadas.</p>
          <div class="ruta-lista">
            {negocios.map((negocio) => {
              const tieneCoordenadas = Number.isFinite(negocio.lat) && Number.isFinite(negocio.lon);
              const posicion = orden.indexOf(negocio.id);
              return (
                <label class={`ruta-parada ${!tieneCoordenadas ? "sin-coordenadas" : ""}`} key={negocio.id}>
                  <input
                    type="checkbox"
                    checked={posicion !== -1}
                    disabled={!tieneCoordenadas}
                    onChange={() => alternar(negocio.id)}
                  />
                  {posicion !== -1 && <span class="ruta-numero">{posicion + 1}</span>}
                  <span class="ruta-nombre">{negocio.nombre}<small>{negocio.municipio}{negocio.direccion ? ` · ${negocio.direccion}` : ""}</small></span>
                  {!tieneCoordenadas && <span class="muted">sin coordenadas</span>}
                </label>
              );
            })}
          </div>

          <div class="ruta-acciones">
            <span class="muted">{seleccionados.length} paradas seleccionadas</span>
            <button disabled={seleccionados.length === 0} onClick={() => descargarGpx(seleccionados)}>Descargar GPX</button>
            {seleccionados.length > 0 && (
              <a class="boton" href={enlaceMaps(seleccionados)} target="_blank" rel="noreferrer">Abrir ruta en Google Maps</a>
            )}
            {orden.length > 0 && <button class="secundario" onClick={() => setOrden([])}>Vaciar ruta</button>}
          </div>
          {seleccionados.length > 9 && <p class="muted">Google Maps puede limitar el número de paradas según el dispositivo. El GPX incluye todas las seleccionadas.</p>}
        </>
      )}
    </section>
  );
}
