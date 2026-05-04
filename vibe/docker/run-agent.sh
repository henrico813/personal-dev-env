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
COMMIT_MESSAGE_INSTRUCTIONS=$'Vibe runtime protocol:\nKeep exactly one unscoped conventional snapshot subject in /artifacts/commit-message.txt.\nIf the task is clear, write an initial subject before editing repository files.\nBefore finishing, update that one-line subject based on the actual changes.\nUse a subject like "feat: add setting", not "feat(vibe): add setting", unless the user explicitly asks for a scope.\nDo not create commit-message.txt in the repository.\nDo not run git commit.'
PROMPT="${COMMIT_MESSAGE_INSTRUCTIONS}"$'\n\nTask:\n'"${TASK_PROMPT}"

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

export VIBE_EVENTS_LOG=/artifacts/events.jsonl
export VIBE_STDERR_LEVEL="${VIBE_STDERR_LEVEL:-info}"

case "${VIBE_STDERR_LEVEL}" in
  trace)
    pi "${PI_ARGS[@]}" "${PROMPT}" | tee /artifacts/events.jsonl >&2
    ;;
  *)
    pi "${PI_ARGS[@]}" "${PROMPT}" | node /opt/vibe/extensions/stderr-progress.mjs
    ;;
esac
