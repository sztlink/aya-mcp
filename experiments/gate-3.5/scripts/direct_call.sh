#!/usr/bin/env bash
set -euo pipefail
TOOL=${1:?tool required}
ARGS=${2:-'{}'}
BASE=${GATE35_BASE:-"$HOME/tmp/aya-mcp-gate35-20260914"}
ROOT="$BASE/runs/A"
STARTED=$(cat "$ROOT/control/benchmark-start-epoch")
ELAPSED=$(( $(date +%s) - STARTED ))
REMAINING=$(( 1200 - ELAPSED ))
CALLS=0
[[ ! -f "$ROOT/logs/bridge-mcp.jsonl" ]] || CALLS=$(wc -l < "$ROOT/logs/bridge-mcp.jsonl")
(( REMAINING > 0 )) || { echo '20 minute arm budget expired' >&2; exit 3; }
(( CALLS < 25 )) || { echo '25 call arm budget exhausted' >&2; exit 4; }
python3 "$BASE/scripts/mcp_call.py" \
  --server "$BASE/profile/extensions/claude_blender/mcp_server.py" \
  --tool "$TOOL" --arguments "$ARGS" \
  --log "$ROOT/logs/bridge-mcp.jsonl" \
  --timeout-seconds "$REMAINING"
