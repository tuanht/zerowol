#!/usr/bin/env bash
# Builds the server with the dev-fakes feature, starts one throwaway server per theme (own port and
# data dir, so the cases that create schedules or machines can't collide) and runs the Playwright UI
# regression cases of both themes (md = Pixel 7, ios = iPhone 15) in parallel. Each theme's output
# is buffered and printed in order (md, then ios) once both are done; exit status is non-zero if any
# case failed. Servers are stopped on exit, failure and Ctrl-C.
# Needs Node and Playwright (npm i -g playwright && npx playwright install chromium).
#
# Iterate on one case or theme:
#   UI_ONLY="long press" make ui-regression      # cases whose name matches the (case-insensitive) regex
#   UI_THEME=ios make ui-regression              # one theme only (md or ios)
#   UI_ONLY="long press" UI_THEME=ios make ui-regression
# UI_REGRESSION_PORT=18091 is the md server's port; the ios server uses the next one.
set -euo pipefail
cd "$(dirname "$0")/.."

PORT=${UI_REGRESSION_PORT:-18091}
THEMES=(md ios)
if [ -n "${UI_THEME:-}" ]; then
  case "$UI_THEME" in md|ios) THEMES=("$UI_THEME") ;; *) echo "UI_THEME must be md or ios" >&2; exit 2 ;; esac
fi
cargo build -p zerowol --features dev-fakes
BIN=${CARGO_TARGET_DIR:-target}/debug/zerowol
TMP=$(mktemp -d)
PIDS=()
cleanup() {
  trap '' INT TERM
  if [ "${#PIDS[@]}" -gt 0 ]; then
    kill "${PIDS[@]}" 2>/dev/null || true
    wait "${PIDS[@]}" 2>/dev/null || true
  fi
  rm -rf "$TMP"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

# Start a server per theme. TZ unset: the "no tz prompt" case needs a server with no timezone to fall
# back to, whatever the caller's environment (the fake seed then saves Europe/Berlin).
i=0
for t in "${THEMES[@]}"; do
  port=$((PORT + i)); i=$((i + 1))
  mkdir -p "$TMP/$t"
  env -u TZ ZEROWOL_DEV_FAKES=1 ZEROWOL_DATA="$TMP/$t" ZEROWOL_PORT=$port ZEROWOL_BIND=127.0.0.1 "$BIN" >"$TMP/$t.server.log" 2>&1 &
  PIDS+=($!)
done
i=0
for t in "${THEMES[@]}"; do
  port=$((PORT + i)); i=$((i + 1))
  for _ in $(seq 50); do
    curl -fs "http://127.0.0.1:$port/healthz" >/dev/null 2>&1 && break
    sleep 0.2
  done
  curl -fs "http://127.0.0.1:$port/healthz" >/dev/null || { cat "$TMP/$t.server.log"; echo "$t server did not start" >&2; exit 1; }
done

# Run the themes concurrently, each against its own server.
RUNNERS=()
i=0
for t in "${THEMES[@]}"; do
  port=$((PORT + i)); i=$((i + 1))
  node scripts/ui-regression.mjs "http://127.0.0.1:$port" "$t" >"$TMP/$t.out" 2>&1 &
  RUNNERS+=($!)
  PIDS+=($!)
done
rc=0
i=0
for t in "${THEMES[@]}"; do
  wait "${RUNNERS[$i]}" || rc=1
  i=$((i + 1))
done
for t in "${THEMES[@]}"; do
  echo "=== $t ==="
  cat "$TMP/$t.out"
done
exit $rc
