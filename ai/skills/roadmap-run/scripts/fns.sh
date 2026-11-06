# Functions for a roadmap run's scripts. The run folder's env file
# loads this copy, so every tool call and script has them.

# Set a variable here and save it for later tool calls; a later value
# wins.
keep() { eval "$1=\$2"; printf '%s=%q\n' "$1" "$2" >> "$D/env"; }

# Print the hash of the tree the plan makes through STEP. The tree is
# written through a scratch index, so no checkout changes.
plan_tree() {
  local plan=$1 repo=$2 base=$3 step=$4 tmp git_dir rc
  tmp=$(mktemp -d) || return 1
  if ! planner export "$plan" --repo "$repo" --base-commit "$base" \
    --out "$tmp/tree" --through "$step" >/dev/null; then
    rm -rf "$tmp"
    return 1
  fi
  git_dir=$(git -C "$repo" rev-parse --path-format=absolute --git-common-dir)
  (
    cd "$tmp/tree" &&
      GIT_INDEX_FILE="$tmp/index" git --git-dir "$git_dir" --work-tree . add -A -f . &&
      GIT_INDEX_FILE="$tmp/index" git --git-dir "$git_dir" --work-tree . write-tree
  )
  rc=$?
  rm -rf "$tmp"
  return $rc
}

# Put back one bug, run the test command, and restore the file.
# Returns 1 when the test command passes, since no test caught the
# bug, and 2 when FILE is gone or OLD does not appear exactly once.
plant() {
  local file=$1 old=$2 new=$3 backup rc
  shift 3
  backup=$(mktemp "$D/plant.XXXXXX") || return 1
  cp "$file" "$backup" || { rm "$backup"; return 2; }
  python3 - "$file" "$old" "$new" <<'EOF' || { rm "$backup"; return 2; }
import sys
path, old, new = sys.argv[1:]
text = open(path).read()
assert text.count(old) == 1, "old text must appear exactly once"
open(path, "w").write(text.replace(old, new))
EOF
  "$@"
  rc=$?
  cp "$backup" "$file" && rm -f "$backup"
  if [ "$rc" -eq 0 ]; then
    echo "plant: no test failed for $file"
    return 1
  fi
  echo "plant: tests failed as expected, exit $rc"
}

# Print the lines added plus deleted in REPO between FROM and TO.
changed() {
  local out
  out=$(git -C "$1" diff --numstat "$2" "$3") || return 1
  printf '%s\n' "$out" | awk '{n += $1 + $2} END {print n + 0}'
}

# Run function step_NAME of build.sh or ship.sh in a subshell, with
# its output added to LOG. The step's mark holds the plan's hash, so a
# rerun skips it until the plan changes. A step that exits 3 ran but
# leaves no mark. Any other failure prints one STOPPED line and ends
# the script.
run_step() {
  local log=$1 name=$2 h rc
  h=$(git hash-object "$P") ||
    { echo "STOPPED at $name: cannot read $P"; exit 1; }
  [ "$(cat "$D/done.$name" 2>/dev/null)" = "$h" ] && return 0
  echo "== $name $(date -u +%FT%TZ)" >> "$log"
  ( . "$D/env" && "step_$name" ) >> "$log" 2>&1
  rc=$?
  case $rc in
    0) echo "$h" > "$D/done.$name" ;;
    3) ;;
    *) echo "STOPPED at $name: $(tail -n 1 "$log")"; exit 1 ;;
  esac
}

# Replace the first line of status.txt and print it.
st() {
  touch "$D/status.txt" &&
    { printf '%s\n' "$1"; sed 1d "$D/status.txt"; } > "$D/status.new" &&
    mv "$D/status.new" "$D/status.txt" && printf '%s\n' "$1"
}

# Print the roadmap's status line for this PR, ending with NEXT, and
# write it as the first line of status.txt.
status_line() (
  { [ -n "$W" ] && [ "$W" != null ]; } ||
    { st "$BR: not built; next: $NEXT"; exit $?; }
  cd "$W" || exit 1
  git fetch -q "$REMOTE" || exit 1
  head=$(git rev-parse HEAD) || exit 1
  out=$(git ls-remote "$REMOTE" "refs/heads/$BR") || exit 1
  pushed=$(printf '%s' "$out" | cut -f 1)
  same=no
  [ "$head" = "$pushed" ] && same=yes
  mb=$(git merge-base "$REMOTE/$PARENT" HEAD) || exit 1
  n=$(changed . "$mb" HEAD) || exit 1
  req=$(cat "$D/request.result" 2>/dev/null) || req="not filed"
  line="$BR: pushed ${pushed:-none}; local equals pushed: $same;"
  line="$line $n changed lines against $PARENT; request: $req"
  st "$line; next: $NEXT"
)
