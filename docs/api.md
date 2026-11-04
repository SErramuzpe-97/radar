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

## Desarrollo

```bash
# terminal 1: API (modo dev imprime los códigos OTP en el log)
cargo run -p radar-cli -- serve --puerto 7800
# terminal 2: UI con proxy a 127.0.0.1:7800
cd radar-ui && npm run dev
```
