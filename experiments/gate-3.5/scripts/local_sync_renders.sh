#!/usr/bin/env bash
set -euo pipefail
ARM=${GATE35_ARM:?GATE35_ARM must be A or B}
LOCAL_ROOT=${GATE35_LOCAL_ROOT:?GATE35_LOCAL_ROOT required}
REMOTE_ROOT=/home/aya/tmp/aya-mcp-gate35-20260914/runs/$ARM
mkdir -p "$LOCAL_ROOT/renders"
scp -q -r "4090:$REMOTE_ROOT/renders/." "$LOCAL_ROOT/renders/"
find "$LOCAL_ROOT/renders" -type f -name '*.png' -printf '%P\n' | sort
