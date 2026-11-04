# Board de tareas — Radar

Generado con `scripts/tasks.sh board`. No editar a mano.

## backlog

| ID | Título | Asignado | Fase | Deps |
|---|---|---|---|---|

## ready

| ID | Título | Asignado | Fase | Deps |
|---|---|---|---|---|

## in_progress

| ID | Título | Asignado | Fase | Deps |
|---|---|---|---|---|
| T-017 | watch BORME/OpenMercantil: leads de apertura (score 90, prioridad A) | vibe | F3 |  |

## blocked

| ID | Título | Asignado | Fase | Deps |
|---|---|---|---|---|

## done

| ID | Título | Asignado | Fase | Deps |
|---|---|---|---|---|
| T-001 | Contrato OAS único generado desde Rust (utoipa) -> api/openapi.yaml | vibe | F1 |  |
| T-002 | Generación determinista del cliente TS en Preact (openapi-typescript) + check de drift | vibe | F1 | T-001 |
| T-003 | Store SQLite + esquema (negocios, auditorias, pipeline, eventos, jobs, auth_codes, sesiones, meta) | vibe | F1 |  |
| T-004 | Fuente Overpass: scan por municipio y sector con upsert | agent:codex | F1 | T-003 |
| T-005 | Auditor web HTTP (1 GET por web, timeout 8s, UA identificable, señales §6.2) | vibe | F1 | T-003 |
| T-006 | Clasificación §6.1 + scoring §7 + prioridad A/B/C | vibe | F1 | T-005 |
| T-007 | CLI: scan / audit / export (csv|gpx|geojson) / watch / serve / api-spec | vibe | F1 | T-005 T-006 |
| T-008 | API REST (axum+utoipa): negocios, pipeline, eventos, stats, export, jobs | vibe | F1 | T-001 T-003 |
| T-009 | Jobs asíncronos lanzados desde la app web: POST /api/jobs/scan|audit|watch + GET /api/jobs/{id} | vibe | F2 | T-007 T-008 |
| T-010 | radar-ui base: Vite + Preact + TS estricto + @preact/signals + cliente OAS generado | vibe | F2 | T-002 |
| T-011 | UI tabla priorizada: filtros (municipio/sector/estado/prioridad/q), orden y paginación | vibe | F2 | T-008 T-010 |
| T-012 | UI ficha de negocio: auditoría, pipeline (cambiar estado) y eventos (registrar actividad) | vibe | F2 | T-011 |
| T-013 | UI panel de búsquedas: lanzar scan/audit desde la web y ver progreso (polling) | vibe | F2 | T-009 T-010 |
| T-014 | UI mapa Leaflet con teselas libres OSM (atribución ODbL) y pines por negocio | vibe | F3 | T-011 |
| T-015 | Export GPX + vista "ruta del día" en la UI (selección manual -> GPX/enlace Maps) | agent:codex | F3 | T-014 T-008 |
| T-016 | Panel KPIs: embudo del pipeline, distribución por estado web/prioridad, eventos de la semana | agent:codex | F3 | T-008 T-011 |
| T-018 | Embebido de la UI en el binario (rust-embed) para un único ejecutable | agent:codex | F2 | T-008 T-010 |
| T-019 | Tests de contrato y API: spec servida == api/openapi.yaml, flujos auth, CRUD | agent:codex | F1 | T-002 T-008 |
| T-020 | Sistema de tareas multi-agente (tasks/, scripts/tasks.sh, board.md, validate) | vibe | F0 |  |
| T-021 | ADRs + actualización del documento de diseño con los detalles nuevos (OAS, jobs desde web, Signals, duckdns+OTP, tareas) | vibe | F0 | T-001 |
| T-022 | Auth OTP por email: allowlist (4 emails), códigos hasheados en SQLite, expiración 10 min, rate limit, SMTP vía env, sesión JWT en cookie HttpOnly + middleware | vibe | F2 | T-001 T-003 |
| T-023 | UI login email -> código de 6 dígitos (Signals) + guard de rutas | vibe | F2 | T-022 T-010 |
| T-024 | Exposición duckdns radar-navarra.duckdns.org: script de IP dinámica + cron + Caddy con Let's Encrypt (port-forward 80/443) | agent:codex | F2 | T-022 |
| T-025 | Endurecimiento de la exposición: bind 127.0.0.1, cookie Secure tras proxy, headers de seguridad, CORS mismo origen | agent:codex | F2 | T-022 T-024 |
| T-026 | Documentación de despliegue duckdns: cuenta, dominio, token, router, Caddyfile, SMTP, checklist | unassigned | F2 | T-024 |
| T-027 | CI local: scripts/ci-local.sh (tasks validate + cargo check + drift OAS + gen:api + npm build) | agent:codex | F0 | T-002 T-019 T-020 |

