#!/usr/bin/env bash
set -euo pipefail

ARM=${1:?usage: stop_arm.sh A|B}
[[ "$ARM" =~ ^[AB]$ ]] || { echo 'arm must be A or B' >&2; exit 1; }
BASE=${GATE35_BASE:-"$HOME/tmp/aya-mcp-gate35-20260914"}
ROOT="$BASE/runs/$ARM"
touch "$ROOT/control/stop"
PID=$(cat "$ROOT/control/blender.pid")
for _ in $(seq 1 300); do
  if ! kill -0 "$PID" 2>/dev/null; then
    wait "$PID" 2>/dev/null || true
    printf 'arm=%s pid=%s stopped=true\n' "$ARM" "$PID"
    exit 0
  fi
  sleep 0.1
done
echo "Blender did not stop normally: $PID" >&2
exit 2
