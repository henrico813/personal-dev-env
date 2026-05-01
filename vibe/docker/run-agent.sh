#!/usr/bin/env bash
set -euo pipefail

PROMPT="$(cat "${VIBE_PROMPT_FILE}")"

exec pi \
  --mode json \
  --no-session \
  --no-extensions \
  -e /opt/vibe/extensions/jsonl-observer.mjs \
  -e /opt/vibe/extensions/git-snapshot.mjs \
  --model "${VIBE_MODEL}" \
  "${PROMPT}"
