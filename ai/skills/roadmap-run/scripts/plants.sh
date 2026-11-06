# Put back each bug from the plan's and the run folder's `plant` lines,
# one at a time, and write one pass, FAIL or stale line per bug to
# plants.txt, ending with done. A passed line is skipped only while the
# plan is unchanged. A plan change runs every line again.
. "$(dirname "$0")/env" || exit 1
{ [ -n "$W" ] && [ "$W" != null ] && cd "$W"; } ||
  { echo "no worktree: '$W'"; exit 1; }
rg -N --no-filename --sort path --max-depth 1 -g '*.md' '^plant ' \
  "$P" "$D" > "$D/plants" || { echo "no plant lines found"; exit 1; }
plan_hash=$(git hash-object "$P") || exit 1
touch "$D/plants.passed" && : > "$D/plants.txt" || exit 1
n=0
while IFS= read -r line <&3; do
  n=$((n + 1))
  if grep -qxF -e "$plan_hash $line" "$D/plants.passed"; then
    echo "skip $n: passed before" >> "$D/plants.txt"
    continue
  fi
  echo "## $n: $line" >> "$D/plants.log"
  eval "$line" < /dev/null >> "$D/plants.log" 2>&1
  case $? in
    0) r=pass ;;
    2) r=stale ;;
    *) r=FAIL ;;
  esac
  [ -z "$(git status --porcelain)" ] || {
    echo "worktree not clean after line $n" >> "$D/plants.txt"
    exit 1
  }
  echo "$r $n: $line" >> "$D/plants.txt"
  [ "$r" != pass ] ||
    printf '%s %s\n' "$plan_hash" "$line" >> "$D/plants.passed"
done 3< "$D/plants"
echo done >> "$D/plants.txt"
cat "$D/plants.txt"
