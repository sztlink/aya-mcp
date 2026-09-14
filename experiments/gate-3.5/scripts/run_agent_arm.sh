#!/usr/bin/env bash
set -euo pipefail

ARM=${1:?usage: run_agent_arm.sh A|B}
[[ "$ARM" =~ ^[AB]$ ]] || { echo 'arm must be A or B' >&2; exit 1; }
REPO_ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
REMOTE_BASE=${GATE35_REMOTE_BASE:-/home/aya/tmp/aya-mcp-gate35-20260914}
LOCAL_BASE=${GATE35_AGENT_BASE:-/home/aya/tmp/aya-mcp-gate35-agent}
LOCAL_ROOT="$LOCAL_BASE/$ARM"
MODEL=${GATE35_MODEL:-openai-codex/gpt-5.6-terra}
THINKING=${GATE35_THINKING:-xhigh}

mkdir -p "$LOCAL_ROOT"
cp "$REPO_ROOT/experiments/gate-3.5/scripts/local_call.sh" "$LOCAL_ROOT/call"
cp "$REPO_ROOT/experiments/gate-3.5/scripts/local_sync_renders.sh" "$LOCAL_ROOT/sync-renders"
chmod +x "$LOCAL_ROOT/call" "$LOCAL_ROOT/sync-renders"

if [[ "$ARM" == A ]]; then
  PATH_DESCRIPTION='direct agent to Blender Agent Bridge'
else
  PATH_DESCRIPTION='agent to experimental AYA Worker to the same Blender Agent Bridge'
fi
{
  printf 'You are the autonomous execution agent for benchmark arm %s (%s). Complete the briefing without asking for help and without human intervention.\n\n' "$ARM" "$PATH_DESCRIPTION"
  cat "$REPO_ROOT/experiments/gate-3.5/fixtures/briefing.md"
  cat <<EOF

Arm protocol:
- Blender 5.1.2 is already running on the remote host 4090 with the candidate copy open and trusted scripts enabled.
- The immutable source is $REMOTE_BASE/runs/$ARM/input/source.blend.
- Save only to $REMOTE_BASE/runs/$ARM/work/candidate.blend.
- Initial renders must go to $REMOTE_BASE/runs/$ARM/renders/initial/.
- Corrected renders must go to $REMOTE_BASE/runs/$ARM/renders/final/.
- Invoke the installed bridge only through $LOCAL_ROOT/call GATEWAY_TOOL 'JSON_ARGUMENTS'.
- Start with blender_bridge_status, then use the five-tool gateway progressively: blender_tool_catalog or search_blender_tools, get_blender_tool_schema, invoke_blender_tool.
- Do not run bpy, Blender, or Python directly over SSH. All Blender mutation and rendering must pass through the bridge.
- After creating initial renders, run $LOCAL_ROOT/sync-renders and use the read tool on the actual local PNG files. You must visually inspect them before deciding the correction.
- After corrected renders, sync and inspect the actual final PNG files as well.
- Keep a count under the 25-call bridge budget. Prefer cohesive trusted draft_script operations, but use inspection and evidence tools where useful.
- Do not alter the immutable source. Do not touch the other benchmark arm.

Proceed autonomously now. Stop only after the candidate is saved and final renders have been visually inspected.
EOF
} > "$LOCAL_ROOT/prompt.txt"

export GATE35_ARM="$ARM"
export GATE35_LOCAL_ROOT="$LOCAL_ROOT"
printf '%s\n' "$(date -u +%FT%TZ)" > "$LOCAL_ROOT/started-at.txt"
START_MS=$(date +%s%3N)
set +e
timeout --signal=TERM --kill-after=10s 1200 \
  pi --print --mode json --no-session --model "$MODEL" --thinking "$THINKING" \
  --tools bash,read -- "$(cat "$LOCAL_ROOT/prompt.txt")" \
  > "$LOCAL_ROOT/agent.jsonl" 2> "$LOCAL_ROOT/agent.stderr"
EXIT_CODE=$?
set -e
END_MS=$(date +%s%3N)
printf '%s\n' "$EXIT_CODE" > "$LOCAL_ROOT/exit-code.txt"
printf '%s\n' "$(( END_MS - START_MS ))" > "$LOCAL_ROOT/wall-ms.txt"
printf '%s\n' "$(date -u +%FT%TZ)" > "$LOCAL_ROOT/finished-at.txt"
exit "$EXIT_CODE"
