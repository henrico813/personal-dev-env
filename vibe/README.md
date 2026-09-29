# Vibe

Vibe is a safe execution harness that runs agent tasks in managed worktrees,
captures run artifacts, records snapshot commits, and sandboxes execution in
Docker.

See the [documentation index](docs/README.md) for focused guidance on shared
skills, trust boundaries, and runtime inputs.

## Prereqs

- run from a git checkout of the target repo
- Docker available locally
- provider auth via supported environment variables or `~/.pi/agent/auth.json`

## Build and test

```bash
make -C vibe build
make -C vibe check
make -C vibe test
```

The live integration tests are compiled but not run by `make check` or `make
test`. On Linux, `make -C vibe integration` runs the file-auth test
`vibe_forwards_model_selector_to_pi`; it requires Docker, GNU `timeout`, network
access, and valid `~/.pi/agent/auth.json` credentials. It uses an isolated Vibe
home but links the real Pi agent directory, so OAuth refreshes can persist.
`make -C vibe integration-goog` runs `vibe_discovers_goog_model`. That test uses
`GOOG_BASE_URL`, `GOOG_API_KEY`, and `GOOG_MODEL`; set the model variable to
`qwen3.8` when the endpoint advertises that ID. It does not mount host Pi state.

Full PDE installs create `~/.config/vibe/goog.env` when it does not already
exist. Fill in its endpoint and key; the file already exports `GOOG_MODEL`.
Run this complete live integration command (the explicit model assignment keeps
the example pinned to the advertised model):

```bash
set -a; source ~/.config/vibe/goog.env; set +a; GOOG_MODEL=qwen3.8 make -C vibe integration-goog
```

## Install

```bash
make -C vibe install
```

If you use PDE, `pde-installer install` installs the same `vibe` binary to
`~/.local/bin`. On first run, Vibe extracts bundled runtime assets under
`~/.local/share/vibe/<version>/` and builds its Docker image; the target
checkout does not need a local `vibe/` directory.

## Run

```bash
cat >/tmp/vibe-task.txt <<'EOF'
Summarize the files under the current worktree and update README.md with one short note.
EOF

vibe run \
  --key pdev-049-demo \
  --base origin/feature/demo \
  --prompt-file /tmp/vibe-task.txt \
  --model openai-codex/gpt-5.6-luna \
  --stderr-level info \
  --commit-message "docs: update README note"

vibe status --key pdev-049-demo
```

Choose a `provider/model` selector. Vibe passes it unchanged to Pi; bare model
names can be ambiguous. For `goog/<model>`, Vibe discovers models from
`GOOG_BASE_URL`; use `goog/qwen3.8` when the endpoint advertises `qwen3.8`. See
[Runtime inputs](docs/reference/runtime-inputs.md). Use `--base <revision>` to
seed a new managed
worktree. Reusing a key keeps its managed branch and must omit `--base`.
Concurrent keys that normalize to the same slug are rejected.

Use `--insecure-tls` only when necessary; it disables certificate verification
inside Docker. Provider credential selection and shared skills are described in
[Runtime inputs](docs/reference/runtime-inputs.md) and [Use shared skills](docs/how-to/use-shared-skills.md).

## Runtime overview

- the managed worktree remains the Git state
- Docker is the execution boundary and runs as the host UID/GID
- bundled assets are extracted under `~/.local/share/vibe/<version>/`
- Vibe mounts the worktree, shared Git metadata, and `/artifacts`
- user and repository skill directories are mounted read-only when present
- the host Pi agent directory is writable only for file-auth fallback
- the executor uses the combined prompt artifact

Artifacts are under
`~/.local/state/vibe/<repo>-<16-hex-git-common-dir-hash>/<slug>/runs/.../`.
The original-key claim is stored in that slug directory. State under the old
`~/.local/state/vibe/<basename>/` layout is intentionally orphaned and is not
migrated. `stdout`
returns one machine-readable Vibe JSON result; `events.jsonl` stores the raw Pi
JSONL stream. `stderr` and `agent.stderr.log` provide the structured caller
stream, while `extension-events.jsonl` stores progress events.

Inspect these artifacts when dogfooding:

- `prompt.txt`, `system-prompt.txt`, `combined-prompt.txt`, and `system-prompt-versions.txt`
- `commit-message.txt`, `events.jsonl`, `agent.stderr.log`, and `extension-events.jsonl`
- `run.json`, `summary.json`, `runs_index.jsonl`, `result.json`, `vibe.log`, and `snapshots.jsonl`
- the result commit and snapshot refs under `refs/vibe/snapshots/...`

## Recovery

- `run.json` is the authoritative per-run record.
- `summary.json` is the default derived status view; `result.json` is the saved command-result view.
- `runs_index.jsonl` is a best-effort lookup index, and `run-state.json` is no longer supported.
- Late persistence failures populate `persistence_error` without rewriting execution status.
- `vibe status --key ...` reads the latest readable state for the normalized key after checking the stored original key; run it inside the target checkout.
- `vibe status --long` shows the full saved run record.
