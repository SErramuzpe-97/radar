# ADR-003 — Preact + Signals + TypeScript en la UI

**Estado:** aceptado · **Fecha:** 04/11/2026

## Decisión

- **Preact** (no React): mismo modelo de componentes con runtime de ~3 KB, ideal para un
  bundle embebido en el binario (`cargo build --release` → una sola pieza).
- **@preact/signals** para el estado global: stores (`stores/auth.ts`, `stores/negocios.ts`,
  `stores/jobs.ts`) con `signal`/`computed`; los componentes reaccionan solos.
- **TypeScript estricto** (`strict: true`) con Vite + `@preact/preset-vite`.
- El cliente HTTP es **openapi-fetch con tipos generados desde el OAS** (ADR-001):
  nada de fetch a mano sin tipos.
- Leaflet con teselas libres (OSM con atribución ODbL u OpenFreeMap) para el mapa.

## Consecuencias

- Bundle pequeño (≈58 KB gzip en la build actual) y arranque inmediato.
- Cambios de estado (login, filtros, jobs) se propagan solos por signals.
- Cualquier error de contrato API se detecta en `tsc` antes de desplegar.
