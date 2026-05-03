# Vibe

Vibe is a safe execution harness that forces agent tasks to run in managed
worktrees, captures observable run artifacts, records snapshot commits for
rollback, and sandboxes the agent in Docker.

## Prereqs

- run from a git checkout of the target repo
- docker available locally
- provider auth available via env vars or `~/.pi/agent/auth.json`

## Build

```bash
make -C vibe build
```

## Test

```bash
make -C vibe check
```

```bash
make -C vibe test
```

## Install

```bash
make -C vibe install
```

This installs `vibe` to `~/.local/bin`.
It does not require the target repo checkout to contain a local `vibe/`
directory. On first run, `vibe` extracts its bundled runtime assets to
`~/.local/share/vibe/<version>/` and builds the Docker image from there.

## Run

```bash
cat >/tmp/vibe-task.txt <<'EOF'
Summarize the files under the current worktree and update README.md with one short note.
EOF

vibe run \
  --key pdev-049-demo \
  --prompt-file /tmp/vibe-task.txt \
  --model openai-codex/gpt-5.4 \
  --commit-message "docs: update README note"
```

## Runtime model

- the managed worktree stays the canonical git state
- Docker is only the execution boundary
- bundled Docker, hook, and extension assets are extracted under
  `~/.local/share/vibe/<version>/`
- Vibe mounts the target worktree, shared git metadata, and `/artifacts`
- the container runs as the host UID/GID and sets git `safe.directory`

Artifacts land under `~/.local/state/vibe/<repo>/<slug>/runs/.../`, where
`<slug>` is the normalized `--key` value.
`stdout` returns one machine-readable JSON result. Progress logs stay on
`stderr`, `events.jsonl`, `extension-events.jsonl`, and
`agent.stderr.log`.

Auth is copied into an ephemeral container home for the run and is not
persisted in the artifact directory.

Dogfood by inspecting:

- `prompt.txt`
- `events.jsonl`
- `agent.stderr.log`
- `extension-events.jsonl`
- `snapshots.jsonl`
- the result commit on the worktree branch
- snapshot refs and commits under `refs/vibe/snapshots/...`
