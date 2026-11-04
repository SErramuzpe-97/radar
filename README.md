# Radar Navarra

Programa local (binario Rust + dashboard Preact) que genera y gestiona la lista de negocios
navarros a los que ofrecer servicios web: descubre candidatos (OSM/BORME), audita su web,
los puntúa 0–100, los sitúa en un mapa y hace de mini-CRM. Todo local, 0 €.

- **Diseño completo:** [docs/diseno-sistema-radar.md](docs/diseno-sistema-radar.md)
- **Decisiones (ADR):** [docs/adr/](docs/adr/)
- **API (OAS):** [api/openapi.yaml](api/openapi.yaml) · guía en [docs/api.md](docs/api.md)
- **Exposición duckdns + login por código:** [docs/exposicion-duckdns.md](docs/exposicion-duckdns.md)
- **Tareas del proyecto:** [tasks/board.md](tasks/board.md) · sistema en [tasks/README.md](tasks/README.md)

## Estructura

```
api/openapi.yaml        Contrato OpenAPI 3.1 (GENERADO desde Rust con utoipa; no editar a mano)
crates/radar-core       Fuentes (Overpass/BORME), auditor web, scoring, store SQLite, export
crates/radar-cli        Binario radar: CLI + API REST (axum) + auth OTP + sirve la UI
radar-ui/               Dashboard Preact + Signals + TS (cliente generado desde el OAS)
deploy/Caddyfile        Proxy TLS (Let's Encrypt) para radar-navarra.duckdns.org
scripts/tasks.sh        Sistema de tareas (claim/done/validate/board)
scripts/duckdns-update.sh  IP dinámica duckdns
scripts/ci-local.sh     Verificación local completa (tareas + cargo + drift OAS + build UI)
tasks/                  Tareas T-*.md (una por archivo) + board.md generado
docs/                   Diseño, ADRs, guías
```

## Arranque rápido (local)

```bash
# 1. Contrato OAS + cliente TS (solo tras cambiar la API)
cargo run -p radar-cli -- api-spec > api/openapi.yaml
cd radar-ui && npm install && npm run gen:api && cd ..

# 2. API + dashboard (el binario sirve la UI en / si existe radar-ui/dist)
cargo run -p radar-cli -- serve --puerto 7800
# o en dos terminales, con hot-reload de la UI:
#   cargo run -p radar-cli -- serve --puerto 7800
#   cd radar-ui && npm run dev        # http://127.0.0.1:5173 (proxy a :7800)

# 3. Probar login (sin SMTP configurado, el código sale en el log del servidor)
# 4. Lanzar búsquedas desde la web (pestaña "Búsquedas") o por CLI:
cargo run -p radar-cli -- scan --municipio Pamplona --sector hosteleria
cargo run -p radar-cli -- audit --prioridad A
cargo run -p radar-cli -- export --formato csv --prioridad A
cargo run -p radar-cli -- watch --fuente borme                 # BORME oficial, últimos 7 días
cargo run -p radar-cli -- watch --fuente borme --fecha 2026-10-09 # día concreto/backfill
```

## API contract-first (ADR-001)

Rust define el contrato con `utoipa` y lo sirve en `/openapi.json`; `radar api-spec`
genera `api/openapi.yaml`; `npm run gen:api` (openapi-typescript) genera
`radar-ui/src/api/schema.ts` y la UI consume la API con `openapi-fetch` tipado.
Las búsquedas se ejecutan desde la web vía `POST /api/jobs/scan|audit|watch` (ADR-002).

## Acceso del equipo (ADR-005)

Expuesto en `https://radar-navarra.duckdns.org` (duckdns + Caddy + Let's Encrypt, 0 €).
Login con **email + código de 6 dígitos enviado por email**. Emails autorizados:
hello@carlotacarrillo.eu, sergio.erramuzpe@gmail.com, pezonetis@gmail.com, duarmeler@gmail.com
(variable `RADAR_AUTH_EMAILS` para cambiarlos). Detalles en
[docs/exposicion-duckdns.md](docs/exposicion-duckdns.md).

## Trabajar en el proyecto (varias personas/agentes)

```bash
scripts/tasks.sh list --estado ready     # tareas disponibles
scripts/tasks.sh claim T-019 agent:mi-agente
# ... trabajar ...
scripts/tasks.sh done T-019
scripts/tasks.sh validate && scripts/tasks.sh board
```

Reglas en [tasks/README.md](tasks/README.md). Una tarea por archivo en `tasks/`, asignación
única, dependencias explícitas.

## Verificación local completa

```bash
scripts/ci-local.sh    # tasks validate + cargo check + drift OAS + gen:api + npm build
```
