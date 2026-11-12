#!/usr/bin/env bash
# End-to-end smoke test: server + agent on loopback, shutdown via the API.
# The machine's agent port is fixed at 9009 (the API has no port field), so
# this script fails early if that port is already in use.
set -euo pipefail
cd "$(dirname "$0")/.."

AGENT_PORT=9009
if (exec 3<>"/dev/tcp/127.0.0.1/$AGENT_PORT") 2>/dev/null; then
  echo "SMOKE FAIL: port $AGENT_PORT is busy; free it and retry" >&2
  exit 1
fi

cargo build --release -p zerowol -p zerowol-agent
SERVER=target/release/zerowol
AGENT=target/release/zerowol-agent

TMP=$(mktemp -d)
PIDS=""
cleanup() {
  for p in $PIDS; do kill "$p" 2>/dev/null || true; done
  rm -rf "$TMP"
}
trap cleanup EXIT

# free port for the server
PORT=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])')
BASE="http://127.0.0.1:$PORT"
mkdir "$TMP/data"

ZEROWOL_PORT=$PORT ZEROWOL_BIND=127.0.0.1 ZEROWOL_DATA="$TMP/data" "$SERVER" >"$TMP/server.log" 2>&1 &
PIDS="$PIDS $!"
for _ in $(seq 50); do
  curl -fs "$BASE/healthz" >/dev/null 2>&1 && break
  sleep 0.1
done
curl -fs "$BASE/healthz" >/dev/null || { echo "SMOKE FAIL: server did not start" >&2; cat "$TMP/server.log" >&2; exit 1; }

RESP=$(curl -fs -X POST "$BASE/api/v1/machines" -H 'content-type: application/json' \
  -d '{"name":"smoke","mac":"aa:bb:cc:dd:ee:ff","ip":"127.0.0.1","agent":true}')
ID=$(printf '%s' "$RESP" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
[ -n "$ID" ] || { echo "SMOKE FAIL: no machine id in: $RESP" >&2; exit 1; }
SECRET=$(grep -o '"secret": *"[0-9a-f]\{64\}"' "$TMP/data/state.json" | head -n1 | sed 's/.*"\([0-9a-f]\{64\}\)"/\1/')
[ -n "$SECRET" ] || { echo "SMOKE FAIL: secret not found in state.json" >&2; exit 1; }

cat >"$TMP/power.sh" <<PEOF
#!/bin/sh
echo "\$@" >> "$TMP/power.log"
PEOF
chmod +x "$TMP/power.sh"
cat >"$TMP/agent.toml" <<AEOF
secret = "$SECRET"
port = $AGENT_PORT
listen = "127.0.0.1"
power_cmd = "$TMP/power.sh"
AEOF

"$AGENT" --config "$TMP/agent.toml" run >"$TMP/agent.log" 2>&1 &
PIDS="$PIDS $!"

paired=""
for _ in $(seq 100); do
  if curl -fs "$BASE/api/v1/machines/$ID/status" | grep -q '"agent":"paired"'; then paired=1; break; fi
  sleep 0.1
done
[ -n "$paired" ] || { echo "SMOKE FAIL: agent never paired" >&2; cat "$TMP/agent.log" >&2; exit 1; }

CODE=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/v1/machines/$ID/shutdown")
[ "$CODE" = 202 ] || { echo "SMOKE FAIL: shutdown returned $CODE" >&2; exit 1; }

sleep 3
grep -q shutdown "$TMP/power.log" 2>/dev/null || { echo "SMOKE FAIL: power_cmd did not receive shutdown" >&2; cat "$TMP/agent.log" >&2; exit 1; }

echo "SMOKE OK"
