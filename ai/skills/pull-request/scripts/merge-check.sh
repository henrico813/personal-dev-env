#!/usr/bin/env bash
# Run test commands on the merge of HEAD into BASE, in a scratch clone,
# so no checkout changes. Run it from the repository:
#
#   bash merge-check.sh BASE HEAD OUT_DIR COMMAND...
#
# Each COMMAND runs with sh -c from the clone's root, with its output in
# OUT_DIR/test<N>.log. OUT_DIR/summary.txt gets one line per command
# and ends with "done". Exits 1 on a merge conflict or a failed command.
set -u

if [ $# -lt 4 ]; then
  echo "usage: merge-check.sh BASE HEAD OUT_DIR COMMAND..." >&2
  exit 2
fi
out=$3
mkdir -p "$out" || exit 1
sum=$out/summary.txt
base=$(git rev-parse --verify "$1^{commit}") || exit 1
head=$(git rev-parse --verify "$2^{commit}") || exit 1
shift 3
printf 'base: %s\nhead: %s\n' "$base" "$head" > "$sum" || exit 1

if ! tree=$(git merge-tree --write-tree --name-only "$base" "$head" \
  2> "$out/merge.log"); then
  printf '%s\n' "$tree" | sed 1d >> "$out/merge.log"
  echo "no clean merge; see $out/merge.log" >> "$sum"
  echo "done" >> "$sum"
  cat "$sum"
  exit 1
fi
commit=$(git commit-tree "$tree" -p "$base" -p "$head" -m merge-check) ||
  exit 1

# A clone has a real index, unlike a git archive copy, so tests that
# call git ls-files see the files. Tests get their own state folder.
work=$(mktemp -d "$out/work.XXXXXX") || exit 1
trap 'rm -rf "$work"' EXIT
src=$work/src
state=$work/state
mkdir "$state" || exit 1
git clone -q --shared --no-checkout \
  "$(git rev-parse --path-format=absolute --git-common-dir)" "$src" &&
  git -C "$src" checkout -q "$commit" || exit 1

failed=0
n=0
for cmd in "$@"; do
  n=$((n + 1))
  (cd "$src" && XDG_STATE_HOME=$state sh -c "$cmd") \
    > "$out/test$n.log" 2>&1
  rc=$?
  [ "$rc" -eq 0 ] || failed=1
  last=$(tail -n 1 "$out/test$n.log" | cut -c 1-160)
  echo "exit $rc: $cmd; last line: $last" >> "$sum"
done
echo "done" >> "$sum"
cat "$sum"
exit "$failed"
