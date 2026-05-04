#!/usr/bin/env bash
set -euo pipefail

mkdir -p "$HOME"
if [[ -n "${VIBE_AUTH_FILE:-}" && -f "${VIBE_AUTH_FILE}" ]]; then
  mkdir -p "$HOME/.pi/agent"
  cp "${VIBE_AUTH_FILE}" "$HOME/.pi/agent/auth.json"
fi

git config --global --add safe.directory "$(pwd)"
git config --global --add safe.directory "${VIBE_REPO_ROOT}"
if [[ -n "${VIBE_GIT_USER_NAME:-}" ]]; then
  git config --global user.name "${VIBE_GIT_USER_NAME}"
fi
if [[ -n "${VIBE_GIT_USER_EMAIL:-}" ]]; then
  git config --global user.email "${VIBE_GIT_USER_EMAIL}"
fi

PROMPT="$(cat "${VIBE_PROMPT_FILE}")"

PI_ARGS=(
  --mode json
  --no-session
  --no-extensions
  -e /opt/vibe/extensions/jsonl-observer.mjs
  -e /opt/vibe/extensions/git-snapshot.mjs
)

if [[ "${VIBE_MODEL}" == */* ]]; then
  PI_ARGS+=(--provider "${VIBE_MODEL%%/*}" --model "${VIBE_MODEL#*/}")
else
  PI_ARGS+=(--model "${VIBE_MODEL}")
fi

exec pi "${PI_ARGS[@]}" "${PROMPT}"
