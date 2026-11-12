# Commands for a roadmap run

The snippets work in zsh and bash. Variables, arrays and functions
are lost between tool calls, so each PR keeps them in a run folder:
`env` holds the variables and loads `fns.sh`. Start every tool call
after the setup with `. <run folder>/env`, using the path the setup
printed. Each snippet stops with `exit 1` on the first failure; never
run the lines after a failed one by hand.

The setup copies this skill's scripts into the run folder: `fns.sh`,
`build.sh`, `ship.sh`, `plants.sh`, `suites.sh` and
`deploy-check.sh`. Run each as the
section below says. You need not read them: each prints a short
result and logs the rest in the run folder.

## Contents

- Send script-checked jobs to the local model
- Write a design digest
- Ask a digest a question
- Set up a PR's run folder
- Run the suites once per plan version
- Build the PR
- Apply a fix round
- Put back the bugs
- Ship
- Check a deploy
- Push a later round
- Print a PR's status line
- Base a child on an unbuilt parent
- Move a PR's base
- Carry a parent's commit to a child

## Send script-checked jobs to the local model

Writing a digest, running the suites, putting back the bugs and
checking a deploy are script jobs, as `model-routing`'s Brief a local
model describes. Send each one to the local model first, with the
command and summary file its section below names. Move it to the
cheap model only when the local job fails its check, as
`model-routing`'s When the check fails says.

## Write a design digest

Before a PR's first planner starts, write its design digest in the
run's shared folder, beside `rules.md`. A script writes it, so it
drops no passage that names a term. List every design note, the
design-edits note among them, and each term the PR adds or changes,
such as a file, type, function, command, flag or error code. Name
each section to copy whole, such as the Dictionary. Write the snippet
below, filled in, to `<shared folder>/digest-<PR name>.sh`. Hand the
job to the local model with the command
`bash <shared folder>/digest-<PR name>.sh` and the summary file
`<shared folder>/design-<PR name>.md.summary`:

```sh
G='<shared folder>/design-<PR name>.md'
rm -f "$G.summary" || exit 1
E='<design-edits note, or the one design note>'
set -- '<design note>' '<design note>'
printf '%s\n' '<term>' '<term>' > "$G.terms" || exit 1
printf '%s\n' 'Dictionary' > "$G.sections" || exit 1
rg -n -F -C 2 --heading -f "$G.terms" -- "$@" > "$G.passages" ||
  { echo "no passage matched, or a note is missing"; exit 1; }
# Copy each design-edits entry that a matched passage links to.
link='\[\[([^]|#]*)#([^]|\\]+)'
rg -o -N --no-filename -r '$1#$2' "$link" "$G.passages" |
  awk -F '#' -v e="$(basename "$E" .md)" '$1 == e { print $2 }' \
  >> "$G.sections"
# Print each listed section and its subsections, with line numbers.
awk '
NR == FNR { want[$0]; next }
FNR == 1 { lvl = 0 }
/^#+ / {
  n = index($0, " ") - 1
  if (lvl && n <= lvl) lvl = 0
  if (!lvl && (substr($0, n + 2) in want)) {
    lvl = n
    print "\n" FILENAME
  }
}
lvl { print FNR ":" $0 }
' "$G.sections" "$@" > "$G.whole" || exit 1
cat "$G.whole" "$G.passages" > "$G" || exit 1
printf '%s lines\ndone\n' "$(wc -l < "$G")" > "$G.summary" || exit 1
cat "$G.summary"
```

Each passage keeps its note and line number, so a reader can read
around it with `sed -n`. To add a term, run the whole snippet again.

## Ask a digest a question

Planners and reviewers ask the local model about a digest instead of
reading the whole digest. They read whole only the passages their
work changes. The rules file names the local and cheap models:

```sh
python3 ~/.agents/skills/roadmap-run/scripts/digest_reader.py \
  ask --model '<local model>' '<design digest>' '<question>'
```

It prints one JSON line with `answer`, `line` and `quote`. Before it
prints, it checks that the quote starts on that line of the digest,
so a printed quote is always the digest's own words. On a nonzero
exit, ask once more, then once with the cheap model, as
`model-routing`'s When the check fails says. When the cheap model
fails too, read the passage yourself with `rg -n` and `sed -n`. You
run on a stronger model than the cheap one. `model-routing`'s When the
check fails lets a stronger model take the digest job over, which is
why you read the passage instead of asking again.

## Set up a PR's run folder

The folder is under `~/.local/state`, so it survives a reboot.
`PARENT` is the branch this PR stacks on: the default branch, or the
parent PR's branch. `P` is the plan file. The setup runs
`planner key` on it before making the folder and stops when the file
has no key. `K` holds that key, such as `abc-12`, and goes into the
run folder's name and `BR`, so one search for the key finds the
branch, its worktree and the run folder. `BR` is this PR's branch,
`<type>/$K-<words>`: a `git-messages` commit type, the key, then
lowercase words joined by hyphens, at most 48 characters after the
type. Words after the key are required: `feat/abc-12-approvals-file`,
never `feat/abc-12`. When the repository sets `pde.branchPattern`,
set `BR` to a branch that starts with the ticket key the plan names,
in the form the pattern asks for, with words after it, and ask the
user when the plan names none; `K` and the run folder still come
from `planner key`. `CHECK` is the plan's test and
lint command, with any race detector or type checker the language
skills ask for. Keep each value in single quotes, since vault paths
have spaces; use double quotes for a value that holds a single quote
or `$K`.

```sh
S=${XDG_STATE_HOME:-$HOME/.local/state}
P='<plan file>'
K=$(planner key "$P") || exit 1
mkdir -p "$S" && D=$(mktemp -d "$S/roadmap-run.$K.XXXXXX") || exit 1
printf 'D=%q\n. "$D/fns.sh"\n' "$D" > "$D/env"
cp "$HOME/.agents/skills/roadmap-run/scripts/"*.sh "$D/" || exit 1
. "$D/env"
keep R '<repository root>'
keep P "$P"
keep B '<full commit from the plan>'
keep REMOTE '<remote, such as origin>'
keep PARENT '<parent branch>'
keep BR "<type>/$K-<words>"
keep CHECK '<test and lint command>'
keep M "$HOME/.agents/skills/git-messages/scripts/msg"
echo "run folder: $D"
```

Write the run folder's path on the roadmap page next to the PR.

## Run the suites once per plan version

Each plan version a reviewer reads gets one run of the test and lint
suites, after `planner check` passes on it. Planners and reviewers
run no test suites or linters, since one script runs them once per
plan version. Reviewers read its summary and run at most the one test
a finding needs. Hand the suites run as a local job to the local
model, as `model-routing` picks. Give it the command
`bash <run folder>/suites.sh` and the summary file
`<run folder>/suites.txt`. Then name that file in each reviewer's
brief.

suites.sh exports the plan's tree through its last step, commits it
on `B` without touching any checkout, and runs `CHECK` on it in a
scratch clone with the pull-request skill's `merge-check.sh`. The
first line of `suites.txt` is `plan: <hash>`, the plan file's
`git hash-object`; the summary holds only for that version of the
plan, and only when its last line is `done`. Each `exit` line gives
the exit code and the last output line; the full output is in
`<run folder>/suites/<key>/test1.log`, where the key hashes the plan
and `CHECK`. A rerun for the same plan version and `CHECK` prints the
saved summary and runs nothing.

## Build the PR

Write `$D/MSG<step>` for each `### <step>.` heading in the plan,
following `git-messages`. Run Move a PR's base right before, since the
parent branch may have moved since the plan was written. Then run:

```sh
bash <run folder>/build.sh
```

It prints `built <branch> in <worktree>: <n> changed lines`, or one
line `STOPPED at <step>: <reason>`. The full output is in `build.log`
in the run folder. To resume, fix the cause and run
`bash <run folder>/build.sh`; it skips each step it finished for this
version of the plan.

- `check`: start a new planner with planner's errors from
  `build.log`, then rerun.
- `lint`: fix the message, then rerun.
- Any other step means a stale plan or a broken tool. Do not retry or
  edit files: stop this PR and its children, put the error on the
  roadmap page as a blocking question, and keep building other
  stacks.

A revised plan makes build.sh repeat every step. Once the branch holds
a build, build.sh stops at `worktree`; bring a revision in with Apply
a fix round instead.

## Apply a fix round

After the planner revises the plan, commit its whole tree as one fix
commit. Read the step list again, since the revision may add steps.
Write `$D/MSGfix` first:

```sh
. <run folder>/env
{ [ -n "$W" ] && [ "$W" != null ] && cd "$W"; } ||
  { echo "no worktree: '$W'"; exit 1; }
python3 "$M" fmt "$D/MSGfix" >/dev/null || exit 1
python3 "$M" lint "$D/MSGfix" || exit 1
planner check "$P" --repo "$R" --json-errors >/dev/null || exit 1
rg -o '^### ([0-9]+)\.' -r '$1' "$P" > "$D/steps" || exit 1
t=$(plan_tree "$P" "$R" "$B" "$(tail -n 1 "$D/steps")") || exit 1
c=$(git -C "$W" commit-tree "$t" -p HEAD -F "$D/MSGfix") || exit 1
git -C "$W" merge -q --ff-only "$c" || exit 1
n=$(changed "$W" "$B" HEAD) || exit 1
echo "changed lines: $n"
```

After the first push, run Push a later round right away. A fix that
stays local is not done. The revised plan no longer matches the marks
that build.sh and ship.sh left, so a later ship.sh run repeats every
step.

## Put back the bugs

This runs every `plant` line in the plan and in the run folder's
reports, one at a time, and writes one `pass`, `FAIL`, `stale` or
`broken` line per `plant` line to `$D/plants.txt`, ending with `done`. Their
output goes to `$D/plants.log`. A passed line is skipped only while
the plan is unchanged. A plan change runs every line again:

```sh
bash <run folder>/plants.sh
```

A `FAIL` line means no test caught that bug. A `stale` line means
its file is gone or its old text no longer appears once, because a
fix round changed it. When a `stale` line's bug still matters, write
a new `plant` line for it in a `.md` file in the run folder. A
`broken` line's test command could not run, such as a misspelled
command; fix the line the same way. Send only `FAIL` lines to a
planner.

## Ship

Run this after the final pass. Write the PR text to `$D/FILE` first,
so it can give the final pass's real numbers. ship.sh squashes only
before the branch's first push:

```sh
bash <run folder>/ship.sh
```

It prints the PR's status line, or one line
`STOPPED at <step>: <reason>`. The full output is in `ship.log` in
the run folder. To resume, fix the cause and run
`bash <run folder>/ship.sh`. It checks the tree and lints the PR
text on every run, and skips each other step it finished for this
version of the plan.

- `tree`: the branch lacks the plan's last revision. Run Apply a fix
  round, then rerun.
- `text`: fix `$D/FILE`, then rerun.
- `push`: follow the reason. A failed check is never minor. When the
  parent moved, run Carry a parent's commit to a child, then rerun.
- The stop after `push`: deploy and try the pushed commit with Check
  a deploy, add the
  one-line result to Testing in `$D/FILE`, write the same line,
  starting with the full hash from `git -C "$W" rev-parse HEAD`,
  to `$D/tested`, then rerun.
- `request`: the reason gives the PR request's result, below.

Put `bash <run folder>/request.sh` on the roadmap page. Rerunning it
reads the result:

- `0`, or `already ran <id>, exit 0`: the PR is open.
- `3`: it waits for the user; say so once and move on. ship.sh still
  prints the status line.
- `4`, `declined <id>`: park this PR and its children.
- `6`, `interrupted <id>`: it is never retried; ask the user.
- `2`: the command was refused; fix it, then rerun ship.sh, which
  writes `request.sh` again.
- Anything else is from `gh`; put its output on the roadmap page.

A request expires four hours after it is filed. Rerunning the script
after that files a fresh request. ship.sh keeps the request's first
text, reruns the saved script while the request waits, and skips it
once the PR is open, so a later round never files a second PR
request.

## Check a deploy

Write each try a script can check as a `try` line in
`<run folder>/deploy.checks`, and delete any old
`<run folder>/deploy.txt`. Hand the job to the local model with
the command `bash <run folder>/deploy-check.sh` and the summary file
`<run folder>/deploy.txt`:

```sh
try '<name>' '<command>' '<text the output must hold>'
```

Each command runs with `sh` in one scratch folder that all the lines
share, so a later line can use what an earlier one made. Name every
other folder in full, as in `git -C <worktree>`. Leave out the text
when the exit code is enough. Put longer setup, such as a fake
program, in a file in the run folder, and run it from a `try` line.
A try that needs judgment, such as an agent following a changed
skill, gets its model from `model-routing` instead.

`deploy.txt` starts with `folder: <scratch folder>`, then holds one
`pass` or `FAIL` line per `try` line. The script stops at the first
`FAIL`. It writes the last line, `done`, only after every line
passed, so a failed try never ends in `done`. Each command's output
is in `<run folder>/deploy.log`.

- `done` last: write the one-line result from the `pass` lines.
- A `FAIL` line last: the PR failed its try. That is the PR's result,
  not a failed job, so do not move the job to the cheap model. Fix the
  PR through the loop.
- Anything else, such as no `deploy.txt`: the local job failed its
  check.

## Push a later round

After the first push, each change adds a commit, and ship.sh stops at
`push`. Update `$D/FILE`, then push the commit with this:

```sh
. <run folder>/env
python3 "$M" fmt "$D/FILE" >/dev/null || exit 1
python3 "$M" lint --pr "$D/FILE" || exit 1
tail -n +3 "$D/FILE" > "$D/BODY" || exit 1
{ [ -n "$W" ] && [ "$W" != null ] && cd "$W"; } ||
  { echo "no worktree: '$W'"; exit 1; }
[ "$(git symbolic-ref -q --short HEAD)" = "$BR" ] ||
  { echo "not on $BR"; exit 1; }
[ -z "$(git status --porcelain)" ] ||
  { echo "worktree not clean"; exit 1; }
sh -c "$CHECK" || { echo "checks failed; not pushing"; exit 1; }
git fetch -q "$REMOTE" || exit 1
git merge-base --is-ancestor "$REMOTE/$PARENT" HEAD ||
  { echo "$PARENT moved; carry its commit to this PR first"; exit 1; }
git push -u "$REMOTE" "HEAD:refs/heads/$BR" || exit 1
```

Then update the PR text with
`pde-gh-write pr edit NUMBER --body-file "$D/BODY"`, and print the
status line. While the PR request waits, there is no PR to edit yet;
update its text once the PR opens. ship.sh's status line then names
`pr edit` as the next step.

## Print a PR's status line

Write the roadmap's status line for a PR from this output, never by
hand. Set NEXT to the PR's next step, such as `skill reviews` or
`rerun request.sh`, so a new session can resume from the roadmap
page. `status_line` also writes the line as the first line of
`$D/status.txt` and keeps the lines below it, so the status line that
ship.sh prints reaches the file too:

```sh
. <run folder>/env
keep NEXT '<next step>'
status_line
```

After the PR's first push, `local equals pushed: no` means a new
commit is not pushed: push it before anything else, and never deploy
it. Before that first push, ship.sh pushes the built PR.

## Base a child on an unbuilt parent

A child planned before its parent is built needs a base commit with
the parent's planned tree. In the parent's run folder:

```sh
. <parent's run folder>/env
pt=$(plan_tree "$P" "$R" "$B" "$(tail -n 1 "$D/steps")") || exit 1
git -C "$R" commit-tree "$pt" -p "$B" -m "Planned tree of $(basename "$P")"
```

Give the printed commit to the child's planner as its base. No branch
points at it, so `git gc` may delete it after two weeks.

## Move a PR's base

Run this in the PR's run folder right before build.sh, for every PR.
The parent branch may have moved since the plan was written, and a
child's parent has just shipped. It fetches the parent branch, sets
the plan's `Base commit:` line to the parent's tip, saves that
commit, and runs `planner check`:

```sh
. <run folder>/env
git -C "$R" fetch -q "$REMOTE" || exit 1
nb=$(git -C "$R" rev-parse --verify "$REMOTE/$PARENT^{commit}") ||
  exit 1
sed -i "0,/^Base commit: .*/s//Base commit: $nb/" "$P" || exit 1
grep -qx "Base commit: $nb" "$P" ||
  { echo "no Base commit: line in $P"; exit 1; }
keep B "$nb"
planner check "$P" --repo "$R" --json-errors >/dev/null ||
  { echo "plan does not apply on $nb"; exit 1; }
```

If the check passes, the plan's diffs apply unchanged. If it fails,
the parent changed after the plan was written: start a new planner
with the errors.

## Carry a parent's commit to a child

After a parent pushes a new commit, run this in each child's run
folder, in stack order, pushing each child before the next. It makes
a merge commit whose tree `planner export` writes from the child's
plan on the pushed parent:

```sh
. <child's run folder>/env
{ [ -n "$W" ] && [ "$W" != null ] && cd "$W"; } ||
  { echo "no worktree: '$W'"; exit 1; }
[ "$(git symbolic-ref -q --short HEAD)" = "$BR" ] ||
  { echo "not on $BR"; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "worktree not clean"; exit 1; }
git fetch -q "$REMOTE" || exit 1
nb=$(git rev-parse "$REMOTE/$PARENT") || exit 1
planner check "$P" --repo "$R" --base-commit "$nb" --json-errors >/dev/null ||
  { echo "plan does not apply on $PARENT; park this PR"; exit 1; }
t=$(plan_tree "$P" "$R" "$nb" "$(tail -n 1 "$D/steps")") || exit 1
c=$(git commit-tree "$t" -p HEAD -p "$nb" -m "chore: merge the parent branch") ||
  exit 1
git merge -q --ff-only "$c" || exit 1
sed -i "0,/^Base commit: .*/s//Base commit: $nb/" "$P" || exit 1
grep -qx "Base commit: $nb" "$P" ||
  { echo "no Base commit: line in $P"; exit 1; }
keep B "$nb"
```

Then rerun ship.sh before the branch's first push, or run Push a
later round after it; both run the plan's checks. A failed
`planner check` is a conflict: never resolve it by hand.
