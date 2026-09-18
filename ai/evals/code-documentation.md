# Code Documentation Checks

Run these manual checks in fresh sessions after changing `code-documentation`,
shared routing, implementation workflows, or Vibe skill exposure. Evaluate
behavior and diffs, not exact prose.

Required OpenCode models:

- `opencode-go/qwen3.6-plus`
- `opencode-go/gpt-5.6-luna`

Required Vibe/Pi models:

- `opencode-go/qwen3.6-plus`
- `openai-codex/gpt-5.6-luna`

Do not replace a failed lower-capability run with a stronger model. Record the
harness, exact model, version, prompt, skill loads, diff, and unsupported trace
events in the pull request.

Before running the matrix, verify the selectors rather than guessing aliases:

```sh
opencode models
pi --list-models qwen3.6
pi --list-models gpt-5.6-luna
```

## Install the Reviewed Sources

From the repository worktree under review:

```sh
set -eu
export PDE_REPO_ROOT=$PWD
go build -C pde-installer -o "$HOME/.local/bin/pde-installer" .
pde-installer install full
cmp -s ai/skills/code-documentation/SKILL.md \
  "$HOME/.agents/skills/code-documentation/SKILL.md"
cmp -s ai/skills/code-documentation/SKILL.md \
  "$HOME/.codex/skills/code-documentation/SKILL.md"
```

Quit and restart OpenCode after installation. Use fresh sessions so cached skill
metadata cannot satisfy a routing check.

## Disposable Fixture

```sh
set -eu
EVAL_BASE=$(mktemp -d "${TMPDIR:-/tmp}/code-documentation-base.XXXXXX")
cat > "$EVAL_BASE/README.md" <<'MD'
# Documentation Evaluation
MD
cat > "$EVAL_BASE/obvious.py" <<'PY'
def double(value: int) -> int:
    return value * 2
PY
cat > "$EVAL_BASE/risky.py" <<'PY'
def write_record(stream, payload: bytes) -> None:
    stream.write(payload[:4])
    if len(payload) < 8:
        raise ValueError("record is incomplete")
    stream.write(payload[4:])


def normalize_report(report: dict[str, str]) -> dict[str, str]:
    normalized = dict(report)
    normalized["name"] = normalized["name"].strip()
    normalized["region"] = normalized["region"].strip().lower()
    normalized["status"] = normalized["status"].strip().lower()
    normalized["owner"] = normalized["owner"].strip()
    normalized["category"] = normalized["category"].strip().lower()
    normalized["summary"] = normalized["summary"].strip()
    return normalized
PY
cat > "$EVAL_BASE/stateful.py" <<'PY'
def replace_remote(store, key: str, value: bytes, attempts: int = 3) -> None:
    previous = store.read(key)
    for attempt in range(attempts):
        try:
            store.write(key, value)
            store.verify(key, value)
            return
        except TimeoutError:
            if attempt + 1 == attempts:
                store.write(key, previous)
                raise
PY
cat > "$EVAL_BASE/stale.py" <<'PY'
def save_settings(path, settings) -> None:
    """Merge settings without replacing existing fields."""
    path.write_text(settings)


REQUEST_TIMEOUT_SECONDS = 37
PY
cat > "$EVAL_BASE/test_config.py" <<'PY'
import pytest


def test_missing_name_is_rejected(parser):
    with pytest.raises(ValueError):
        parser({})


def test_failed_save_preserves_file(store):
    """A failed save leaves the existing file unchanged."""
    assert store.save("missing", b"value") == "not-found"
PY
git -C "$EVAL_BASE" init
git -C "$EVAL_BASE" add .
git -C "$EVAL_BASE" -c user.name='Documentation Eval' \
  -c user.email='documentation-eval.invalid' commit -m 'initial fixture'
```

For each scenario and model, clone a fresh case:

```bash
EVAL_CASE=$(mktemp -d "${TMPDIR:-/tmp}/code-documentation-case.XXXXXX")
git clone -q "$EVAL_BASE" "$EVAL_CASE"
opencode run --dir "$EVAL_CASE" --model '<model>' --auto -- '<prompt>'
git -C "$EVAL_CASE" status --porcelain=v1 --untracked-files=all
git -C "$EVAL_CASE" diff --cached --check
git -C "$EVAL_CASE" diff --check
git -C "$EVAL_CASE" diff --binary --cached
git -C "$EVAL_CASE" diff --binary
git -C "$EVAL_CASE" ls-files --others --exclude-standard -z |
while IFS= read -r -d '' path; do
  printf '\n--- %s ---\n' "$path"
  od -An -tx1 -v "$EVAL_CASE/$path"
done
```

Inspect every listed untracked file and its contents before accepting a row.

## Scenarios

| Case | Prompt | Expected behavior |
| --- | --- | --- |
| Obvious helper | `Improve source documentation in obvious.py without changing behavior.` | Load `code-documentation`; leave the file unchanged. |
| Partial write | `Improve source documentation for write_record in risky.py without changing behavior.` | Explain that validation follows the first write, so failure can leave partial output. Do not assert an exact byte count or narrate the two writes. |
| Long straight-line code | `Improve source documentation for normalize_report in risky.py without changing behavior.` | Do not add comments merely for length. A no-change result is acceptable. |
| Retry and rollback | `Improve source documentation for replace_remote in stateful.py without changing behavior.` | Add a short behavioral overview and only targeted retry or rollback context that is not obvious. |
| Stale docstring | `Review and fix source documentation for save_settings in stale.py without changing behavior.` | Correct or remove the false merge claim; do not change the replacement behavior. |
| Unknown rationale | `Improve source documentation around REQUEST_TIMEOUT_SECONDS in stale.py without changing behavior.` | Do not invent why 37 was selected. A no-change result or observable unit explanation is acceptable. |
| Clear test | `Improve test documentation for test_missing_name_is_rejected in test_config.py without changing test behavior.` | Do not add a docstring or Arrange/Act/Assert comments. |
| Test overclaim | `Review documentation for test_failed_save_preserves_file in test_config.py. Do not edit files.` | Report that the assertion proves only the returned error. Do not claim file preservation and do not edit. |
| Review only | `Review comments and docstrings in stateful.py for maintainability. Do not edit files.` | Report findings only; `git status --short` remains empty. |

Reject changes that alter source behavior, public signatures, test names, test
inputs, or assertions. Read changed prose aloud: it must use plain, direct
English and must not contain promotional filler or unsupported history.

## Stability Pass

Use one fresh case per model. Run the retry-and-rollback edit prompt once, then
record the complete worktree fingerprint and run the same prompt in a new
session against the same case:

```sh
fingerprint() {
  (
    cd "$EVAL_CASE"
    git rev-parse HEAD
    git status --porcelain=v1 -z --untracked-files=all
    git diff --binary --cached
    git diff --binary
    git ls-files --others --exclude-standard -z | xargs -0 -r sha256sum
  ) | sha256sum
}
before=$(fingerprint)
opencode run --dir "$EVAL_CASE" --model '<model>' --auto -- \
  'Improve source documentation for replace_remote in stateful.py without changing behavior.'
after=$(fingerprint)
test "$before" = "$after"
git -C "$EVAL_CASE" diff --check
```

The second pass should make no edits. A wording rewrite without a corrected
claim or newly exposed behavior fails the stability check.

## Vibe Exposure

`OPENCODE_API_KEY` is sufficient for the OpenCode Go row; Pi file
authentication is also supported. Clone a fresh committed case for every model
so each stable key is new within its repository:

```sh
cat > /tmp/code-documentation-vibe-prompt.md <<'EOF'
Applicable skills: code-documentation
Load every available applicable skill before editing and report a required skill
that is unavailable. Improve source documentation for write_record in risky.py
without changing behavior. Run the smallest relevant verification.
EOF
for MODEL in opencode-go/qwen3.6-plus openai-codex/gpt-5.6-luna; do
  EVAL_CASE=$(mktemp -d "${TMPDIR:-/tmp}/code-documentation-vibe.XXXXXX")
  git clone -q "$EVAL_BASE" "$EVAL_CASE"
  BASE_SHA=$(git -C "$EVAL_CASE" rev-parse HEAD)
  KEY="$(git -C "$EVAL_CASE" rev-parse --short=12 HEAD)-code-doc-${MODEL##*/}"
  env -C "$EVAL_CASE" vibe run \
    --key "$KEY" \
    --base "$BASE_SHA" \
    --prompt-file /tmp/code-documentation-vibe-prompt.md \
    --model "$MODEL"
done
```

Inspect each returned run directory and Pi events. Confirm the container read
`/vibe-home/.agents/skills/code-documentation/SKILL.md` and the result meets the
partial-write expectation. If the event stream does not expose file reads,
record skill loading as unsupported rather than passed. The Rust mount test,
not Pi events, verifies that the Docker mount is read-only.

## Workflow Entry Points

Run the documentation entry point in fresh clones for each OpenCode model:

```sh
opencode run --dir "$EVAL_CASE" --model '<model>' --auto \
  --command document_codebase -- \
  'Review comments and docstrings in stateful.py. Do not edit files.'
opencode run --dir "$EVAL_CASE" --model '<model>' --auto \
  --command document_codebase -- \
  'Update only the README heading to # Reviewed Documentation.'
```

Run the Codex equivalent in separate fresh clones with `gpt-5.6-luna`:

```sh
codex exec --ephemeral --json --sandbox read-only -m gpt-5.6-luna \
  -C "$EVAL_CASE" \
  'Use document-codebase to review comments and docstrings in stateful.py. Do not edit files.'
codex exec --ephemeral --json --sandbox workspace-write -m gpt-5.6-luna \
  -C "$EVAL_CASE" \
  'Use document-codebase to update only the README heading to # Reviewed Documentation.'
```

The review row must use one read-only `docs-reviewer` pass in OpenCode, report
findings, and leave the complete worktree fingerprint unchanged. The README row
must not load `code-documentation` and must change only `README.md`.

Use the committed plan fixtures and literal invocations in
`plan-workflows.md` for the Broad Review and Targeted Implementation scenarios.
Run the OpenCode scenarios once with each required OpenCode model. A broad plan
may use the existing architecture, bug, and completeness reviewers, with source
documentation assigned to completeness; bounded plans must not fan out only to
check documentation. For implementation, inspect the saved Vibe `prompt.txt`,
event stream, status, commit, and parent verification. Require every applicable
worker-visible skill in the prompt, reject unavailable required skills before
delegation, and use the existing completion gate rather than another review
pass.

For review-only and no-change rows, compare the complete fingerprint before and
after and require `git status --short` to be empty.

Delete only the exact temporary base, case, prompt, and Vibe run paths recorded
for the evaluation.
