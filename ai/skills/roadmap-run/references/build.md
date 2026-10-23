# Commands for a roadmap run

The snippets work in zsh and bash. Set these first:

```sh
R=<repository root>
P=<plan file>
B=<the plan's Base commit, in full>
KEY=<Vibe key, such as agent-commands>
MODEL=<provider/model for Vibe workers>
M=~/.agents/skills/git-messages/scripts/msg
D=$(mktemp -d)
```

## Contents

- Turn a plan step into a tree
- Make each step's patch
- Write and lint the commit messages
- Build with Vibe
- Apply a fix round
- Plant a bug
- Ship
- Move a child's base

## Turn a plan step into a tree

`planner export` writes the whole tree, so files the plan deletes are
simply missing. This writes the tree into the repository's object
store through a scratch index, without touching any checkout, and
prints its hash:

```sh
plan_tree() {
  local plan=$1 repo=$2 base=$3 step=$4 tmp git_dir rc
  tmp=$(mktemp -d)
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
```

## Make each step's patch

```sh
planner check "$P" --repo "$R" --json-errors
steps=($(rg -o '^### ([0-9]+)\.' -r '$1' "$P"))
prev=$(git -C "$R" rev-parse "$B^{tree}")
for s in "${steps[@]}"; do
  t=$(plan_tree "$P" "$R" "$B" "$s") || break
  git -C "$R" diff "$prev" "$t" > "$D/step$s.patch"
  echo "$t" > "$D/tree$s"
  prev=$t
done
git -C "$R" diff --shortstat "$B" "$prev"
```

The last line is the PR's size. Over 700 changed lines, stop and
split the plan.

## Write and lint the commit messages

Write `$D/MSG<step>` for each step, following `git-messages`, and
the step prompts `$D/prompt<step>.md` from
[prompts.md](prompts.md). Lint before any build, and stop on failure:

```sh
for s in "${steps[@]}"; do
  python3 "$M" fmt "$D/MSG$s" >/dev/null
  python3 "$M" lint "$D/MSG$s" || { echo "lint failed: step $s"; exit 1; }
done
```

## Build with Vibe

Check the key first, as `implement-plan` says. Only the first run of a
new key takes `--base`:

```sh
first=1
for s in "${steps[@]}"; do
  base=()
  if [ "$first" = 1 ]; then base=(--base "$B"); first=0; fi
  out=$(vibe run --key "$KEY" "${base[@]}" \
    --prompt-file "$D/prompt$s.md" --input "$D/step$s.patch" \
    --model "$MODEL" --commit-message "$(cat "$D/MSG$s")" \
    --stderr-level warn)
  st=$(printf '%s' "$out" | jq -r '.status + " " + (.persistence_error // "none")')
  W=$(printf '%s' "$out" | jq -r .worktree)
  if [ "$st" != "completed none" ]; then
    printf '%s\n' "$out"
    break
  fi
  if [ "${#base[@]}" -gt 0 ] &&
    [ "$(printf '%s' "$out" | jq -r .pre_run_commit)" != "$B" ]; then
    echo "step $s: Vibe did not start from $B"
    break
  fi
  if [ "$(git -C "$W" rev-parse 'HEAD^{tree}')" != "$(cat "$D/tree$s")" ]; then
    echo "step $s: branch tree differs from the plan"
    break
  fi
done
```

Then, in the worktree `$W`, copy the parent's approval record in if
the comment checker needs one, and run the plan's checks.

## Apply a fix round

After the planner revises the plan, apply the whole difference as one
commit. Omit `--base`, since the key exists:

```sh
t=$(plan_tree "$P" "$R" "$B" "${steps[-1]}")
git -C "$W" diff HEAD "$t" > "$D/fix.patch"
```

Write `$D/MSGfix` and a prompt naming `fix.patch`, lint, run
`vibe run` as above without `--base`, and check that the branch tree
equals `$t`.

## Plant a bug

Put back one bug a reviewer found, run the tests, and restore the file.
A test must fail:

```sh
plant() {
  local file=$1 old=$2 new=$3 backup
  shift 3
  backup=$(mktemp)
  cp "$file" "$backup"
  python3 - "$file" "$old" "$new" <<'EOF' || { rm "$backup"; return 1; }
import sys
path, old, new = sys.argv[1:]
text = open(path).read()
assert text.count(old) == 1, "old text must appear exactly once"
open(path, "w").write(text.replace(old, new))
EOF
  "$@"
  cp "$backup" "$file"
  rm "$backup"
}
plant internal/x.go 'if a {' 'if false && a {' go test ./internal/...
git -C "$W" status --short  # must print nothing
```

## Ship

Squash only before the branch's first push. After it, every round is
a new commit.

```sh
cd "$W"
git fetch -q <remote>
tree=$(git rev-parse 'HEAD^{tree}')
git reset -q --soft "<remote>/<parent branch>"
git commit -q -F "$D/FILE"
[ "$(git rev-parse 'HEAD^{tree}')" = "$tree" ] || echo "tree changed"
git push --force-with-lease -u <remote> HEAD
tail -n +3 "$D/FILE" > "$D/BODY"
pde-gh-write pr create --base "<parent branch>" \
  --title "$(head -n 1 "$D/FILE")" --body-file "$D/BODY"
```

`$D/FILE` is the PR text; lint it with `msg lint --pr` before the
reset. Exit 3 from `pde-gh-write` means the request waits for the
user; tell them once and move on.

## Move a child's base

After the parent ships, set the child plan's `Base commit:` line to
the parent's final commit and run `planner check`. If it passes, the
child's diffs apply unchanged. If it fails, the parent changed after
the child was written: send the errors to the child's planner.
