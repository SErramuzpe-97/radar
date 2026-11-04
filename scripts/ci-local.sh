#!/usr/bin/env bash
# CI local de Radar (0 €, sin servicios externos): valida tareas, compila Rust,
# comprueba drift del contrato OAS y compila la UI con los tipos regenerados.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export PATH="$HOME/.cargo/bin:$PATH"

echo "== 1/5 tasks: validate =="
scripts/tasks.sh validate

echo "== 2/5 cargo check =="
cargo check --workspace

echo "== 3/5 drift OAS (api/openapi.yaml debe ser exactamente el que genera el binario) =="
cargo run -q -p radar-cli -- api-spec > /tmp/radar-openapi-check.yaml
diff -u api/openapi.yaml /tmp/radar-openapi-check.yaml && echo "OAS sin drift"

echo "== 4/5 gen:api (cliente TS) =="
(cd radar-ui && npm run gen:api)
diff -q radar-ui/src/api/schema.ts <(cd radar-ui && npm run gen:api >/dev/null 2>&1 && cat src/api/schema.ts) >/dev/null 2>&1 || true

echo "== 5/5 build UI =="
(cd radar-ui && npm run build)

echo "== board =="
scripts/tasks.sh board

echo "CI local OK"
