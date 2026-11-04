#!/usr/bin/env bash
# CI local de Radar (0 €, sin servicios externos): valida tareas, compila Rust,
# comprueba drift del contrato OAS y compila la UI con los tipos regenerados.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export PATH="$HOME/.cargo/bin:$PATH"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/radar-ci.XXXXXX")"
trap 'rm -rf "$TMP_DIR"' EXIT

if [[ ! -d radar-ui/node_modules ]]; then
  echo "== Instalar dependencias de UI =="
  (cd radar-ui && npm ci)
fi

echo "== 1/5 tasks: validate =="
scripts/tasks.sh validate

echo "== 2/5 cargo check =="
cargo check --workspace

echo "== 3/5 drift OAS (api/openapi.yaml debe ser exactamente el que genera el binario) =="
cargo run -q -p radar-cli -- api-spec > "$TMP_DIR/openapi.yaml"
diff -u api/openapi.yaml "$TMP_DIR/openapi.yaml"
echo "OAS sin drift"

echo "== 4/5 gen:api (cliente TS) =="
(cd radar-ui && ./node_modules/.bin/openapi-typescript ../api/openapi.yaml -o "$TMP_DIR/schema.ts")
diff -u radar-ui/src/api/schema.ts "$TMP_DIR/schema.ts"
echo "Cliente TS sin drift"

echo "== 5/5 build UI =="
(cd radar-ui && npm run build)

echo "== board =="
scripts/tasks.sh board

echo "CI local OK"
