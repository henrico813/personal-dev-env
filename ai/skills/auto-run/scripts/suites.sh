# Run CHECK once on the tree of the plan's last step, with pde-check in
# a scratch clone, and write its summary to suites.txt. Agents read that
# file instead of running the suites. Its first line holds the plan's
# hash, and a rerun for the same plan and CHECK runs nothing. Logs stay
# in suites/<key>, a hash of both.
. "$(dirname "$0")/env" || exit 1
h=$(git hash-object "$P") || exit 1
k=$(printf '%s\n%s\n' "$h" "$CHECK" | git hash-object --stdin) ||
  exit 1
out=$D/suites/$k
if [ "$(tail -n 1 "$out/summary.txt" 2>/dev/null)" != "done" ]; then
  last=$(rg -o '^### ([0-9]+)\.' -r '$1' "$P" | tail -n 1)
  t=$(plan_tree "$P" "$R" "$B" "$last") ||
    { echo "export failed: step $last"; exit 1; }
  c=$(git -C "$R" commit-tree "$t" -p "$B" -m "suites: $h") || exit 1
  # Some tests call git ls-files, so CHECK runs in a checked-out clone.
  work=$(mktemp -d "$D/work.XXXXXX") || exit 1
  trap 'rm -rf "$work"' EXIT
  git clone -q --shared --no-checkout \
    "$(git -C "$R" rev-parse --path-format=absolute --git-common-dir)" \
    "$work/src" &&
    git -C "$work/src" checkout -q "$c" &&
    mkdir "$work/state" || exit 1
  # Tests get their own state folder, so they leave the user's alone.
  (cd "$work/src" &&
    XDG_STATE_HOME=$work/state pde-check -o "$out" "$CHECK") > /dev/null
fi
{ echo "plan: $h"; cat "$out/summary.txt"; } > "$D/suites.txt" || exit 1
cat "$D/suites.txt"
