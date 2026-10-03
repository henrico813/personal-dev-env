#!/usr/bin/env zsh
# Checks the planner commands written in the AI planning prompts, without a model.
#
# Agents do not invent planner commands. They copy them from
# ai/opencode/commands/*.md and ai/codex/skills/*/SKILL.md and run them in the
# user's shell. Planner's Go tests prove the binary works, but nothing checked
# that the prompt text still matches it. When the two drift, every plan run
# fails or burns turns recovering, and the only check was a manual eval that
# spends model tokens, so in practice nobody ran it.
#
# Real failures this catches:
# - Prompts wrote `--target implementation[1].file_changes[1]` unquoted. zsh
#   reads the brackets as a glob, stops with "no matches found", and planner
#   never runs. The first eval of the guarded workflow lost a turn to this.
# - The guarded patch rewrite deleted `planner dod`, `planner implementation`,
#   and `planner verification`. A prompt still naming them sends every agent to
#   a command that no longer exists.
set -euo pipefail
setopt typesetsilent

repo_root=${0:A:h:h:h}
tmp_root=$(mktemp -d "${TMPDIR:-/tmp}/prompt-commands.XXXXXX")
trap 'rm -rf -- "$tmp_root"' EXIT

go build -C "$repo_root/planner" -o "$tmp_root/planner" ./main
export PATH="$tmp_root:$PATH"

# Extract backtick-quoted planner invocations and flatten line wrapping. A
# prompt wraps one command across lines, so matching whole backtick spans keeps
# the invocation intact.
extract_commands() {
  perl -0777 -ne 'while (/`(planner\b[^`]*)`/gs) {
    my $c = $1; $c =~ s/\s+/ /g; print "$c\n";
  }' "$1"
}

prompt_files=("$repo_root"/ai/opencode/commands/*.md "$repo_root"/ai/codex/skills/*/SKILL.md)

# Planner has only these subcommands. When one is removed, this fails until
# every prompt stops naming it.
allowed=(help new check inspect patch)

for file in "${prompt_files[@]}"; do
  while IFS= read -r command; do
    sub=${${(z)command}[2]}
    if (( ${allowed[(Ie)$sub]} == 0 )); then
      print -u2 "prompt $file uses unknown planner subcommand '$sub': $command"
      exit 1
    fi
  done < <(extract_commands "$file")
done

# Each planning prompt must still tell the agent to write and check the plan.
# If an edit drops `planner check`, agents stop validating plans and nothing
# notices, because the unchecked plan still looks finished.
require_subcommands() {
  local file=$1
  shift
  local commands=("${(@f)$(extract_commands "$file")}")
  local want command
  for want in "$@"; do
    local found=0
    for command in "${commands[@]}"; do
      [[ ${${(z)command}[2]} == "$want" ]] && found=1
    done
    if (( !found )); then
      print -u2 "prompt $file no longer invokes planner $want"
      exit 1
    fi
  done
}

for file in "${prompt_files[@]}"; do
  case $file in
    */create_plan.md|*/create-plan/SKILL.md) require_subcommands "$file" new inspect patch check ;;
    */implement_plan.md|*/implement-plan/SKILL.md) require_subcommands "$file" inspect patch check ;;
  esac
done

# Build a disposable repository with one committed source file.
new_fixture() {
  local dir=$1
  mkdir -p "$dir"
  git -C "$dir" init -q
  printf 'module example.com/prompt-eval\n\ngo 1.21\n' > "$dir/go.mod"
  printf 'package main\n\nfunc main() {}\n' > "$dir/main.go"
  git -C "$dir" add -A
  git -C "$dir" -c user.name=eval -c user.email=eval@example.com commit -qm init
}

# implement_plan revises an existing plan, so give it one real change to edit.
seed_plan() {
  local repo=$1 plan=$2
  local base
  base=$(git -C "$repo" rev-parse HEAD)
  planner new "$plan" >/dev/null
  perl -0pi -e 's/^`path\/to\/file`$/`main.go`/m' "$plan"
  local scratch_dir
  scratch_dir=$(mktemp -d "$tmp_root/seed.XXXXXX")
  local scratch="$scratch_dir/source"
  local token
  token=$(planner inspect "$plan" --target 'implementation[1].file_changes[1]' \
    --repo "$repo" --base "$base" --before --code-out "$scratch" | jq -r .edit_expect)
  printf '\nvar seeded = true\n' >> "$scratch"
  planner patch "$plan" --target 'implementation[1].file_changes[1]' \
    --expect "$token" --repo "$repo" --base "$base" --after-file "$scratch" >/dev/null
  print -r -- "$base"
}

# Run the prompt's own inspect, patch, and check commands in document order.
# Matching text alone cannot catch shell problems, so the commands really run.
# Placeholders are replaced at the text level and eval keeps the prompt's
# quoting, so an unquoted bracket selector fails here exactly as it would in zsh.
run_prompt_commands() {
  local file=$1 repo=$2 plan=$3 base=$4
  local selector='implementation[1].file_changes[1]'
  local token='' scratch='' command output
  local commands=("${(@f)$(extract_commands "$file")}")
  for command in "${commands[@]}"; do
    [[ $command == 'planner '(inspect|patch|check)' '* &&
      $command == *'--repo <repo>'* && $command == *'--base <commit>'* ]] || continue
    if [[ $command == 'planner inspect '* ]]; then
      scratch="$(mktemp -d "$tmp_root/run.XXXXXX")/source"
    fi
    command=${command//'<plan.md>'/$plan}
    command=${command//'<output.md>'/$plan}
    command=${command//'<selector>'/$selector}
    command=${command//'<repo>'/$repo}
    command=${command//'<commit>'/$base}
    command=${command//'<new-scratch-file>'/$scratch}
    command=${command//'<scratch-file>'/$scratch}
    command=${command//'<edit_expect>'/$token}
    output=$(eval "$command") || { print -u2 "prompt $file failed: $command"; exit 1; }
    if [[ $command == 'planner inspect '* ]]; then
      token=$(jq -r .edit_expect <<<"$output")
      # Change the scratch source so the next patch has something to write.
      printf '\n// prompt-check edit\n' >> "$scratch"
    fi
  done
}

for file in "${prompt_files[@]}"; do
  case $file in
    */create_plan.md|*/create-plan/SKILL.md|*/implement_plan.md|*/implement-plan/SKILL.md) ;;
    *) continue ;;
  esac
  fixture="$tmp_root/fixture"
  rm -rf -- "$fixture"
  mkdir -p "$fixture/plans"
  new_fixture "$fixture"
  plan="$fixture/plans/prompt.md"
  base=$(seed_plan "$fixture" "$plan")
  run_prompt_commands "$file" "$fixture" "$plan" "$base"
done

print 'prompt command checks passed'
