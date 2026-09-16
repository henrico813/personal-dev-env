#!/bin/sh
set -eu

repo=$(mktemp -d "${TMPDIR:-/tmp}/planner-workflow-repo.XXXXXX")
state=$(mktemp -d "${TMPDIR:-/tmp}/planner-workflow-state.XXXXXX")
cleanup() { rm -rf "$repo" "$state"; }
trap cleanup EXIT INT TERM

git -C "$repo" init --quiet
printf '# Fixture\n' >"$repo/README.md"
git -C "$repo" add README.md
git -C "$repo" -c user.name=Workflow -c user.email=workflow.invalid commit --quiet -m fixture
export XDG_STATE_HOME="$state"

workflow_id() { planner workflow id | jq -r .workflow_id; }
revision() { jq -r .revision; }

populate_tasks() {
	root=$1
	for task in architecture interfaces-data-state tests-verification; do
		cat >"$root/$task/task.json" <<EOF
{
  "summary": "workflow fixture",
  "explicit_files": ["README.md"],
  "search_areas": ["."],
  "query": ["How does the fixture behave?"],
  "terms": ["Fixture"]
}
EOF
	done
}

create_root() {
	root=$(surveil new task --task architecture)
	surveil new task --root "$root" --task interfaces-data-state >/dev/null
	surveil new task --root "$root" --task tests-verification >/dev/null
	populate_tasks "$root"
	printf '%s\n' "$root"
}

id=$(workflow_id)
output="$state/plan.md"
start=$(planner workflow start --id "$id" --repo "$repo" --output "$output" --operation new)
rev=$(printf '%s' "$start" | revision)
root=$(create_root)
update=$(planner workflow research bind "$id" --expected-revision "$rev" --managed-root "$root")
rev=$(printf '%s' "$update" | revision)
surveil index --repo "$repo"
receipt=$(surveil session run --repo "$repo" --root "$root")
update=$(planner workflow research complete "$id" --expected-revision "$rev" --receipt "$receipt")
rev=$(printf '%s' "$update" | revision)
planner new "$output" >/dev/null
printf 'No findings.\n' >"$root/manual-review.md"
printf 'No material findings.\n' >"$root/evidence-disposition.md"
update=$(planner workflow evidence complete "$id" --expected-revision "$rev" --review "$root/manual-review.md" --disposition "$root/evidence-disposition.md")
rev=$(printf '%s' "$update" | revision)
printf 'No findings.\n' >"$root/loaded-skill-review.md"
update=$(planner workflow review complete "$id" --expected-revision "$rev" --path "$root/loaded-skill-review.md" --outcome pass)
rev=$(printf '%s' "$update" | revision)
evidence="$root/.surveil-session/evidence.json"
cp "$evidence" "$evidence.saved"
printf 'tamper\n' >>"$evidence"
if planner workflow finish "$id" --expected-revision "$rev" >/dev/null 2>&1; then
	printf 'finish accepted tampered evidence\n' >&2
	exit 1
fi
mv "$evidence.saved" "$evidence"
planner workflow finish "$id" --expected-revision "$rev" | jq -e '.completion.sha256 | length == 64' >/dev/null

start_id=$(workflow_id)
start_output="$state/start-stdout.md"
if planner workflow start --id "$start_id" --repo "$repo" --output "$start_output" --operation new >/dev/full 2>/dev/null; then
	printf 'start stdout failure unexpectedly succeeded\n' >&2
	exit 1
fi
planner workflow show "$start_id" | jq -e '.revision == 1 and .stage == "research-pending"' >/dev/null

stdout_id=$(workflow_id)
stdout_output="$state/session-stdout.md"
stdout_start=$(planner workflow start --id "$stdout_id" --repo "$repo" --output "$stdout_output" --operation new)
stdout_rev=$(printf '%s' "$stdout_start" | revision)
stdout_root=$(create_root)
stdout_update=$(planner workflow research bind "$stdout_id" --expected-revision "$stdout_rev" --managed-root "$stdout_root")
stdout_rev=$(printf '%s' "$stdout_update" | revision)
if surveil session run --repo "$repo" --root "$stdout_root" >/dev/full 2>/dev/null; then
	printf 'session stdout failure unexpectedly succeeded\n' >&2
	exit 1
fi
stdout_receipt="$stdout_root/.surveil-session/receipt.json"
test -f "$stdout_receipt"
planner workflow research complete "$stdout_id" --expected-revision "$stdout_rev" --receipt "$stdout_receipt" >/dev/null

failed_id=$(workflow_id)
failed_output="$state/failed.md"
failed_start=$(planner workflow start --id "$failed_id" --repo "$repo" --output "$failed_output" --operation new)
failed_rev=$(printf '%s' "$failed_start" | revision)
failed_root=$(surveil new task --task architecture)
failed_update=$(planner workflow research bind "$failed_id" --expected-revision "$failed_rev" --managed-root "$failed_root")
failed_rev=$(printf '%s' "$failed_update" | revision)
if surveil session run --repo "$repo" --root "$failed_root" >/dev/null 2>&1; then
	printf 'invalid session unexpectedly succeeded\n' >&2
	exit 1
fi
test ! -e "$failed_root/.surveil-session/receipt.json"
failed_update=$(planner workflow fail "$failed_id" --expected-revision "$failed_rev" --stage research --reason "surveil session exited without a receipt")
failed_rev=$(printf '%s' "$failed_update" | revision)
if planner workflow finish "$failed_id" --expected-revision "$failed_rev" >/dev/null 2>&1; then
	printf 'finish accepted failed research\n' >&2
	exit 1
fi

source="$state/source.md"
destination="$state/destination.md"
planner new "$source" >/dev/null
partial_id=$(workflow_id)
planner workflow start --id "$partial_id" --repo "$repo" --input "$source" --output "$destination" --operation partial-update \
	| jq -e '.stage == "research-pending"' >/dev/null
planner workflow show "$partial_id" \
	| jq -e --arg source "$source" --arg destination "$destination" '.input.path == $source and .initial_output.path == $destination and .initial_output.absent' >/dev/null
