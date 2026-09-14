#!/usr/bin/env bash
set -euo pipefail
ARM=${GATE35_ARM:?GATE35_ARM must be A or B}
TOOL=${1:?gateway tool required}
ARGS=${2:-'{}'}
REMOTE_BASE=/home/aya/tmp/aya-mcp-gate35-20260914
if [[ "$ARM" == A ]]; then REMOTE_SCRIPT="$REMOTE_BASE/scripts/direct_call.sh"; else REMOTE_SCRIPT="$REMOTE_BASE/scripts/aya_call.sh"; fi
printf -v Q_TOOL '%q' "$TOOL"
printf -v Q_ARGS '%q' "$ARGS"
ssh 4090 "$REMOTE_SCRIPT $Q_TOOL $Q_ARGS"
