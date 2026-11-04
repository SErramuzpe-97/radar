# ADR-002 — Las búsquedas se ejecutan desde la app web (jobs asíncronos)

**Estado:** aceptado · **Fecha:** 04/11/2026

## Contexto

El plan original preveía lanzar `scan`/`audit` solo por CLI. El requisito nuevo es que
**desde la app web se puedan ejecutar las búsquedas**.

## Decisión

La API expone endpoints de job que ejecutan el mismo núcleo (`radar-core`) que la CLI:

- `POST /api/jobs/scan` `{municipio, sector?}` → 202 + `Job`
- `POST /api/jobs/audit` `{prioridad?, limite?}` → 202 + `Job`
- `POST /api/jobs/watch` `{fuente, csv?}` → 202 + `Job`
- `GET /api/jobs` y `GET /api/jobs/{id}` → estado y progreso (`hechos/total`)

Los jobs se persisten en SQLite (tabla `jobs`) y corren en tareas tokio en background.
La UI hace polling cada 2 s y refresca tabla y stats al terminar. CLI y web comparten
el mismo código: no hay duplicación.

## Consecuencias

- El dashboard es operativo por sí mismo: lanzar un scan de Pamplona/hostelería es un botón.
- El estado del job sobrevive a reinicios (queda en SQLite).
- Los jobs largos (audit masiva) no bloquean la API.
