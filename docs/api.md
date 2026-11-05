# API de Radar (contrato-first con OAS)

La API entre Rust y Preact se define **una sola vez** y todo el código de consumo se genera
de forma determinista (ADR-001).

## Flujo

```
1. Editas las anotaciones utoipa en crates/radar-cli/src (api.rs, auth.rs)
2. cargo run -p radar-cli -- api-spec > api/openapi.yaml     # genera el contrato OAS (OpenAPI 3.1)
3. cd radar-ui && npm run gen:api                            # openapi-typescript -> src/api/schema.ts
4. npm run build                                             # tsc + vite (falla si el contrato cambió sin regenerar)
```

El binario sirve el mismo contrato en `GET /openapi.json`. `scripts/ci-local.sh` verifica
que `api/openapi.yaml` no tenga drift con lo que genera el binario.

## Endpoints

| Método | Ruta | Descripción |
|---|---|---|
| POST | `/api/auth/request-code` | Pide código de acceso para un email (allowlist) |
| POST | `/api/auth/verify` | Verifica el código y crea sesión (cookie `radar_session`) |
| GET | `/api/auth/me` | Sesión actual |
| POST | `/api/auth/logout` | Cierra la sesión |
| GET | `/api/negocios` | Lista paginada con filtros (municipio, sector, estado_web, prioridad, q, ordenar, dir) |
| GET | `/api/negocios/mapa` | Puntos y agrupaciones de la zona visible, hasta 825 elementos |
| GET | `/api/negocios/{id}` | Ficha completa (auditorías, pipeline, eventos) |
| PATCH | `/api/negocios/{id}/pipeline` | Actualiza estado del pipeline CRM |
| GET/POST | `/api/negocios/{id}/eventos` | Lista / registra eventos (visita, llamada…) |
| POST | `/api/jobs/scan` | **Lanza búsqueda** (Overpass) desde la web → 202 |
| POST | `/api/jobs/audit` | Lanza auditoría web masiva → 202 |
| POST | `/api/jobs/watch` | Vigila constituciones de Navarra en BORME/BOE (últimos 7 días; CSV opcional) → 202 |
| GET | `/api/jobs`, `/api/jobs/{id}` | Estado y progreso de los jobs |
| GET | `/api/export?formato=csv|geojson|gpx` | Export para rutas de visita |
| GET | `/api/stats` | KPIs del panel |
| GET | `/openapi.json` | Contrato OAS servido por el binario |

Todo `/api/*` salvo `/api/auth/*` exige sesión (ADR-005).

## Mapa por zonas

`GET /api/negocios/mapa?sur=41.9&oeste=-2.6&norte=43.4&este=-0.7&zoom=8`
acepta los filtros comerciales de la tabla y devuelve `{items, total, zona}`.
Cada elemento contiene posición, cantidad, potencial máximo y extensión de la agrupación.
Los puntos individuales incluyen también ID, nombre y sector; la ficha completa se consulta
solo al abrirla. `total` cuenta todas las empresas geolocalizadas de la zona, sin recortar
los resultados a 25.000. Las empresas sin coordenadas se consultan en la tabla.

Los cuatro límites son opcionales como conjunto; sin ellos se obtiene una vista general.
Una zona parcial, invertida o fuera de Web Mercator devuelve HTTP 400. La cuadrícula adapta
su tamaño al zoom y a la extensión para mantener como máximo 825 elementos por respuesta,
incluso con todo el mundo solicitado al zoom máximo. La consulta utiliza un índice por
admisibilidad y coordenadas y no enriquece ni modifica negocios al leer el mapa.

La UI recarga al terminar un movimiento y al cambiar filtros, espera 180 ms para agrupar
eventos y cancela peticiones anteriores. Al salir del mapa detiene su carga. Pulsar una
agrupación acerca la vista y abre una lista de 20 empresas por página, accesible también
cuando todas comparten coordenadas. Esa lista usa los mismos cuatro límites en
`GET /api/negocios`.

Regresión: `cargo test --workspace` y, con el release recién construido y el runtime de
Chrome/Puppeteer del worker disponible, `node scripts/probar-mapa.mjs`. Este último crea
55.000 empresas en una base temporal y comprueba API, zoom, filtros, paginación y fichas
en navegador de escritorio y móvil, sin enviar emails ni visitar servicios externos.

## Desarrollo

La lista aplica los filtros y la ordenación sobre IDs, pagina y solo entonces carga las
fichas de esa página. Los empates se resuelven por ID para mantener una paginación estable.
Los filtros de auditoría utilizan índices por negocio/URL en trabajos técnicos e informes;
las empresas sin URL confirmada no requieren consultar su historial técnico. Se conserva
la precedencia de trabajos en curso, pendientes y último informe de la URL actual.

La UI espera 250 ms al escribir nombre o municipio, cancela la petición anterior y descarta
respuestas antiguas. El indicador de carga permite seguir usando los controles y desplazando
la tabla. Las consultas de la lista se ejecutan fuera de los hilos del runtime asíncrono.

`node scripts/probar-mapa.mjs` cubre también los cuatro filtros de auditoría con 55.000
empresas y 1.000 trabajos/informes técnicos, búsquedas agrupadas y cambios rápidos de filtro
con una petición anterior retenida artificialmente. La base usada es temporal e independiente.

```bash
# terminal 1: API (modo dev imprime los códigos OTP en el log)
cargo run -p radar-cli -- serve --puerto 7800
# terminal 2: UI con proxy a 127.0.0.1:7800
cd radar-ui && npm run dev
```
