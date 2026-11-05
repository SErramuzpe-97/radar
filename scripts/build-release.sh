#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/radar-ui"
npm ci
npm run build
cd "$ROOT"
cargo build --release -p radar-cli
