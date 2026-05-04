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

TASK_PROMPT="$(cat "${VIBE_PROMPT_FILE}")"
COMMIT_MESSAGE_INSTRUCTIONS=$'\n\nWrite exactly one conventional commit subject to the absolute path /artifacts/commit-message.txt. Choose the type prefix yourself based on the current step or task title. Write one line only. Do not create commit-message.txt in the repository; only write /artifacts/commit-message.txt.'
PROMPT="${TASK_PROMPT}${COMMIT_MESSAGE_INSTRUCTIONS}"

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

exec pi "${PI_ARGS[@]}" "${PROMPT}" > >(tee /artifacts/events.jsonl >&2)
