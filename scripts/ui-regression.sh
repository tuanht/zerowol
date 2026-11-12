#!/usr/bin/env bash
# Builds the server with the dev-fakes feature, starts it on a throwaway data
# directory and runs the Playwright UI regression cases against it.
# Needs Node and Playwright (npm i -g playwright && npx playwright install chromium).
set -euo pipefail
cd "$(dirname "$0")/.."

PORT=${UI_REGRESSION_PORT:-18091}
cargo build -p zerowol --features dev-fakes
BIN=${CARGO_TARGET_DIR:-target}/debug/zerowol
TMP=$(mktemp -d)
trap 'kill "${PID:-}" 2>/dev/null || true; rm -rf "$TMP"' EXIT

ZEROWOL_DEV_FAKES=1 ZEROWOL_DATA=$TMP ZEROWOL_PORT=$PORT ZEROWOL_BIND=127.0.0.1 "$BIN" >"$TMP/server.log" 2>&1 &
PID=$!
for _ in $(seq 50); do
  curl -fs "http://127.0.0.1:$PORT/healthz" >/dev/null 2>&1 && break
  sleep 0.2
done
curl -fs "http://127.0.0.1:$PORT/healthz" >/dev/null || { cat "$TMP/server.log"; echo "server did not start" >&2; exit 1; }

node scripts/ui-regression.mjs "http://127.0.0.1:$PORT"
