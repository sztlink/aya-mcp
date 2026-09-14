#!/usr/bin/env bash
set -euo pipefail

DEST=${1:?usage: collect_gate35_evidence.sh DESTINATION}
REMOTE_HOST=${GATE35_REMOTE_HOST:-4090}
REMOTE_BASE=${GATE35_REMOTE_BASE:-/home/aya/tmp/aya-mcp-gate35-20260914}
AGENT_BASE=${GATE35_AGENT_BASE:-/home/aya/tmp/aya-mcp-gate35-agent}
mkdir -p "$DEST/remote" "$DEST/agent/A" "$DEST/agent/B" "$DEST/install"
scp -q -r "$REMOTE_HOST:$REMOTE_BASE/runs/." "$DEST/remote/"
scp -q "$REMOTE_HOST:$REMOTE_BASE/base/source.blend" "$DEST/source.blend"
scp -q "$REMOTE_HOST:$REMOTE_BASE/base/source.sha256" "$DEST/source.sha256"
scp -q "$REMOTE_HOST:$REMOTE_BASE/install/downloads/install-sha256.txt" "$DEST/install/install-sha256.txt"
for ARM in A B; do
  cp -a "$AGENT_BASE/$ARM/." "$DEST/agent/$ARM/"
done
if [[ -f "$AGENT_BASE/AB-final-contact.png" ]]; then
  cp "$AGENT_BASE/AB-final-contact.png" "$DEST/AB-final-contact.png"
fi
find "$DEST" -type f ! -name SHA256SUMS ! -name file-inventory.txt -printf '%P %s\n' | sort > "$DEST/file-inventory.txt"
(
  cd "$DEST"
  find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum
) > "$DEST/SHA256SUMS"
printf 'evidence_root=%s\n' "$(realpath "$DEST")"
