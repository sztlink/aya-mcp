#!/usr/bin/env bash
set -euo pipefail

BASE=${GATE35_BASE:-"$HOME/tmp/aya-mcp-gate35-20260914"}
SOURCE="$BASE/base/source.blend"
SOURCE_SHA=$(sha256sum "$SOURCE" | awk '{print $1}')
for ARM in A B; do
  ROOT="$BASE/runs/$ARM"
  test ! -e "$ROOT" || { echo "arm root exists: $ROOT" >&2; exit 2; }
  mkdir -p "$ROOT"/{input,work,renders/initial,renders/final,evidence,logs,control}
  cp --reflink=auto "$SOURCE" "$ROOT/input/source.blend"
  chmod 0444 "$ROOT/input/source.blend"
  cp --reflink=auto "$SOURCE" "$ROOT/work/candidate.blend"
  cat > "$ROOT/arm.json" <<EOF
{"arm":"$ARM","sourcePath":"input/source.blend","sourceSha256":"$SOURCE_SHA","candidatePath":"work/candidate.blend","bridge":"CallMeJones/blender-agent-bridge@v0.5.6","blender":"5.1.2"}
EOF
  if [[ "$ARM" == B ]]; then cp "$ROOT/arm.json" "$ROOT/workcell.json"; fi
  sha256sum "$ROOT/input/source.blend" "$ROOT/work/candidate.blend" > "$ROOT/logs/initial-sha256.txt"
done
cmp "$BASE/runs/A/input/source.blend" "$BASE/runs/B/input/source.blend"
cmp "$BASE/runs/A/work/candidate.blend" "$BASE/runs/B/work/candidate.blend"
printf 'source_sha256=%s\n' "$SOURCE_SHA"
