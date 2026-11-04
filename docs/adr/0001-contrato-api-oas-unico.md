# ADR-001 — Contrato API único en OAS con generación determinista de código

**Estado:** aceptado · **Fecha:** 04/11/2026

## Contexto

Radar es un binario Rust que sirve una API REST y un dashboard Preact. Hace falta un contrato
único entre ambos lados, y el usuario pide que la comunicación se haga **mediante OAS con
generación determinista y automática de código de consumo** por parte de Preact y de Rust.

## Decisión

- Las anotaciones `utoipa` en `radar-cli` son la fuente de verdad del contrato.
- `radar api-spec > api/openapi.yaml` **genera** el contrato OpenAPI 3.1 commiteado
  (`api/openapi.yaml`). Nunca si edita a mano.
- El binario sirve el mismo contrato en `GET /openapi.json`.
- La UI genera su cliente tipado con `openapi-typescript` (`npm run gen:api` →
  `radar-ui/src/api/schema.ts`) y consume la API con `openapi-fetch`.
- `scripts/ci-local.sh` comprueba el **drift**: si `api/openapi.yaml` no coincide con lo que
genera el binario, falla.

Flujo determinista:

```
código Rust (utoipa) --api-spec--> api/openapi.yaml --openapi-typescript--> radar-ui/src/api/schema.ts --> openapi-fetch (cliente TS tipado)
        |                                                                                                        ^
        +-- sirve /openapi.json y los endpoints /api/* ----------------------------------------------------------+
```

## Consecuencias

- Cambiar la API exige tocar las anotaciones, regenerar el OAS y regenerar el cliente TS.
- Cero dependencias externas de pago; todo local y reproducible.
- El contrato es legible por humanos (YAML commiteado) y por máquinas (TS generado).
