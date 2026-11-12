#!/usr/bin/env bash
# Commit one step of a plan in the current worktree for a local model,
# then run the check command. Run it from the worktree:
#
#   bash local-step.sh PLAN BASE STEP MESSAGE FOLDER CHECK
#
# FOLDER/summary.txt gets "result: pass" or "result: fail: <why>",
# then "exit: <code>" and "done", so the caller reads what the script
# wrote, not the model's words. A rerun after a commit skips it.
set -u

if [ $# -ne 6 ]; then
  echo "usage: local-step.sh PLAN BASE STEP MESSAGE FOLDER CHECK" >&2
  exit 2
fi
plan=$1
base=$2
step=$3
message=$4
folder=$5
check=$6
mkdir -p "$folder" || exit 2

finish() {
  printf 'result: %s\nexit: %s\ndone\n' "$1" "$2" > "$folder/summary.txt"
  exit "$2"
}

. "$(dirname "$0")/../../roadmap-run/scripts/fns.sh" ||
  finish "fail: cannot load roadmap-run's fns.sh" 2
repo=$(git rev-parse --show-toplevel) || finish "fail: not a worktree" 2

planner check "$plan" --repo "$repo" --json-errors \
  > "$folder/planner.log" 2>&1 ||
  finish "fail: planner check; see planner.log" 1
[ -z "$(git status --porcelain)" ] ||
  finish "fail: worktree has changes; nothing committed" 1
tree=$(plan_tree "$plan" "$repo" "$base" "$step") ||
  finish "fail: planner export of step $step" 1
if [ "$(git rev-parse 'HEAD^{tree}')" != "$tree" ]; then
  commit=$(git commit-tree "$tree" -p HEAD -F "$message") ||
    finish "fail: commit of step $step" 1
  git merge -q --ff-only "$commit" ||
    finish "fail: branch did not move to $commit" 1
fi

sh -c "$check" > "$folder/check.log" 2>&1
code=$?
[ "$code" -eq 0 ] && finish pass 0
finish "fail: check; last line: $(tail -n 1 "$folder/check.log")" "$code"
