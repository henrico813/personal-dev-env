#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OPENCODE="$REPO_ROOT/pde-installer/node_modules/.bin/opencode"

[[ -x "$OPENCODE" ]]
node --version >/dev/null

home="$(mktemp -d)"
trap 'rm -rf "$home"' EXIT
mkdir -p "$home/.config/opencode"
cat >"$home/.config/opencode/opencode.jsonc" <<'JSON'
{
  "plugin": [
    "@openchamber/opencode-claude@0.14.0",
  ],
}
JSON

models="$(HOME="$home" XDG_CONFIG_HOME="$home/.config" "$OPENCODE" models)"
[[ "$models" == *"claude-code/"* ]]
printf 'PASS: pinned Claude Code provider is registered\n'
