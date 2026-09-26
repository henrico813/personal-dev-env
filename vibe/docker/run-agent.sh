#!/usr/bin/env bash
set -euo pipefail

mkdir -p "$HOME"
mkdir -p "${NPM_CONFIG_PREFIX:-$HOME/.npm-global}"

git config --global --add safe.directory "$(pwd)"
git config --global --add safe.directory "${VIBE_REPO_ROOT}"
if [[ -n "${VIBE_GIT_USER_NAME:-}" ]]; then
  git config --global user.name "${VIBE_GIT_USER_NAME}"
fi
if [[ -n "${VIBE_GIT_USER_EMAIL:-}" ]]; then
  git config --global user.email "${VIBE_GIT_USER_EMAIL}"
fi

combined_prompt_file="${VIBE_COMBINED_PROMPT_FILE:-}"
if [[ -z "${combined_prompt_file}" || ! -r "${combined_prompt_file}" ]]; then
  missing="${combined_prompt_file:-<unset>}"
  echo "missing combined prompt artifact: ${missing}" >&2
  exit 97
fi
mapfile -d '' -t PROMPT_PARTS < "${combined_prompt_file}"
if ((${#PROMPT_PARTS[@]})); then
  PROMPT="${PROMPT_PARTS[0]}"
else
  PROMPT=""
fi

PI_ARGS=(
  --mode json
  --no-session
  --no-extensions
  --no-skills
  -e /opt/vibe/extensions/jsonl-observer.mjs
  -e /opt/vibe/extensions/git-snapshot.mjs
)

shared_skills_dir="$HOME/.agents/skills"
if [[ -d "${shared_skills_dir}" ]]; then
  PI_ARGS+=(--skill "${shared_skills_dir}")
fi

if [[ "${VIBE_MODEL}" == openai-compatible/* ]]; then
  mkdir -p "$HOME/.pi/agent"
  node -e '
    const fs = require("fs");
    const path = process.argv[1];
    const config = {
      providers: {
        "openai-compatible": {
          baseUrl: process.env.OPENAI_COMPATIBLE_BASE_URL,
          api: "openai-completions",
          apiKey: "$OPENAI_COMPATIBLE_API_KEY",
          discoverModels: true,
        },
      },
    };
    fs.writeFileSync(path, JSON.stringify(config));
  ' "$HOME/.pi/agent/models.json"
  PI_ARGS+=(-e /opt/vibe/.pi/agent/npm/node_modules/pi-models-discovery/index.ts)
fi

PI_ARGS+=(--model "${VIBE_MODEL}")

export VIBE_EVENTS_LOG=/artifacts/events.jsonl
export VIBE_STDERR_LEVEL="${VIBE_STDERR_LEVEL:-info}"

pi "${PI_ARGS[@]}" "${PROMPT}" | node /opt/vibe/extensions/stderr-progress.mjs
