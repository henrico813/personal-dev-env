#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob

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

# Returns 0 when $1 is a PDE workflow-orchestration skill, i.e. its
# SKILL.md frontmatter marks pde-workflow: "true". Vibe workers execute a
# single task, so workflow skills are never offered to Pi.
is_workflow_skill() {
  local skill_md="$1/SKILL.md"
  local line marker
  [[ -f "${skill_md}" ]] || return 1
  [[ "$(head -n 1 "${skill_md}")" == "---" ]] || return 1
  marker='^[[:space:]]*pde-workflow[[:space:]]*:[[:space:]]*\"?([Tt][Rr][Uu][Ee])\"?[[:space:]]*$'
  while IFS= read -r line || [[ -n "${line}" ]]; do
    [[ "${line}" == "---" ]] && break
    if [[ "${line}" =~ ${marker} ]]; then
      return 0
    fi
  done < <(tail -n +2 "${skill_md}")
  return 1
}

# Pi loads every skill under a --skill directory, so each skill directory is
# selected individually and workflow-orchestration skills are left out.
select_skill_root() {
  local root="$1" entry
  if [[ ! -d "${root}" ]]; then
    return 0
  fi
  if [[ -f "${root}/SKILL.md" ]]; then
    # The directory is one skill; select it unless it is a workflow skill.
    if ! is_workflow_skill "${root}"; then
      PI_ARGS+=(--skill "${root}")
    fi
    return 0
  fi
  for entry in "${root}"/*/; do
    entry="${entry%/}"
    if ! is_workflow_skill "${entry}"; then
      PI_ARGS+=(--skill "${entry}")
    fi
  done
}

shared_skills_dir="$HOME/.agents/skills"
select_skill_root "${shared_skills_dir}"
repository_skills_dir="${VIBE_REPO_SKILLS_DIR:-}"
if [[ -n "${repository_skills_dir}" ]]; then
  select_skill_root "${repository_skills_dir}"
fi

if [[ "${VIBE_MODEL}" == goog/* ]]; then
  mkdir -p "$HOME/.pi/agent"
  node -e '
    const fs = require("fs");
    const path = process.argv[1];
    const config = {
      providers: {
        goog: {
          baseUrl: process.env.GOOG_BASE_URL,
          api: "openai-completions",
          apiKey: "$GOOG_API_KEY",
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
