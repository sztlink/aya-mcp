#!/usr/bin/env bash
set -euo pipefail

BASE=${GATE35_BASE:-"$HOME/tmp/aya-mcp-gate35-20260914"}
DOWNLOADS="$BASE/install/downloads"
BLENDER_ARCHIVE="$DOWNLOADS/blender-5.1.2-linux-x64.tar.xz"
BRIDGE_ARCHIVE="$DOWNLOADS/claude_blender-0.5.6.zip"
BLENDER_SHA=aaccb355f50183979b698bcce7467103a76261b5fa59f4972295842662a285fb
BRIDGE_SHA=b16756274c26f64c34c4e78f4d68e09753068445817566d10414411d7b48efb7
BLENDER_URL=https://download.blender.org/release/Blender5.1/blender-5.1.2-linux-x64.tar.xz
BRIDGE_URL=https://github.com/CallMeJones/blender-agent-bridge/releases/download/v0.5.6/claude_blender-0.5.6.zip

mkdir -p "$DOWNLOADS" "$BASE/profile"/{config,scripts,cache,extensions,datafiles,resources}
[[ -f "$BLENDER_ARCHIVE" ]] || curl -fL --retry 3 -o "$BLENDER_ARCHIVE" "$BLENDER_URL"
[[ -f "$BRIDGE_ARCHIVE" ]] || curl -fL --retry 3 -o "$BRIDGE_ARCHIVE" "$BRIDGE_URL"
printf '%s  %s\n%s  %s\n' \
  "$BLENDER_SHA" "$(basename "$BLENDER_ARCHIVE")" \
  "$BRIDGE_SHA" "$(basename "$BRIDGE_ARCHIVE")" \
  > "$DOWNLOADS/install-sha256.txt"
(cd "$DOWNLOADS" && sha256sum -c install-sha256.txt)

if [[ ! -x "$BASE/install/blender-5.1.2-linux-x64/blender" ]]; then
  tar -xJf "$BLENDER_ARCHIVE" -C "$BASE/install"
fi
ln -sfn "$BASE/install/blender-5.1.2-linux-x64" "$BASE/install/blender-current"
BLENDER="$BASE/install/blender-current/blender"
"$BLENDER" --version

rm -rf "$BASE/profile/extensions/claude_blender"
mkdir -p "$BASE/profile/extensions/claude_blender"
unzip -q "$BRIDGE_ARCHIVE" -d "$BASE/profile/extensions/claude_blender"

export BLENDER_USER_CONFIG="$BASE/profile/config"
export BLENDER_USER_SCRIPTS="$BASE/profile/scripts"
export BLENDER_USER_CACHE="$BASE/profile/cache"
export BLENDER_USER_EXTENSIONS="$BASE/profile/extensions"
export BLENDER_USER_DATAFILES="$BASE/profile/datafiles"
export BLENDER_USER_RESOURCES="$BASE/profile/resources"
"$BLENDER" --background --factory-startup --command extension repo-add gate35 \
  --name 'Gate 3.5 disposable extensions' \
  --directory "$BASE/profile/extensions/.user/gate35" \
  --clear-all
"$BLENDER" --background --factory-startup --command extension install-file \
  -r gate35 -e "$BRIDGE_ARCHIVE"
printf 'installed=%s\n' "$BASE"
