#!/usr/bin/env bash
set -euo pipefail

ARM=${1:?usage: start_arm.sh A|B}
[[ "$ARM" =~ ^[AB]$ ]] || { echo 'arm must be A or B' >&2; exit 1; }
BASE=${GATE35_BASE:-"$HOME/tmp/aya-mcp-gate35-20260914"}
ROOT="$BASE/runs/$ARM"
B="$BASE/install/blender-current/blender"
if curl -fsS --max-time 1 http://127.0.0.1:8765/health >/dev/null 2>&1; then
  echo 'bridge port 8765 is already active' >&2
  exit 2
fi
rm -f "$ROOT/logs/bridge-ready.json" "$ROOT/control/stop"
export BLENDER_USER_CONFIG="$BASE/profile/config"
export BLENDER_USER_SCRIPTS="$BASE/profile/scripts"
export BLENDER_USER_CACHE="$BASE/profile/cache"
export BLENDER_USER_EXTENSIONS="$BASE/profile/extensions"
export GATE35_ARM_ROOT="$ROOT"
nohup "$B" "$ROOT/work/candidate.blend" --background --python "$BASE/scripts/bridge_host.py" \
  > "$ROOT/logs/blender-bridge.log" 2>&1 &
PID=$!
printf '%s\n' "$PID" > "$ROOT/control/blender.pid"
for _ in $(seq 1 200); do
  if [[ -f "$ROOT/logs/bridge-ready.json" ]] && curl -fsS --max-time 2 http://127.0.0.1:8765/health > "$ROOT/logs/bridge-health.json"; then
    date +%s > "$ROOT/control/benchmark-start-epoch"
    printf 'arm=%s pid=%s ready=true\n' "$ARM" "$PID"
    exit 0
  fi
  if ! kill -0 "$PID" 2>/dev/null; then
    tail -100 "$ROOT/logs/blender-bridge.log" >&2
    exit 3
  fi
  sleep 0.1
done
echo 'bridge readiness timeout' >&2
exit 4
