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

If you already use the PDE Go CLI, `pde install ai-tools` installs the
same `vibe` binary to `~/.local/bin`.

This installs `vibe` to `~/.local/bin`.
It does not require the target repo checkout to contain a local `vibe/`
directory. On first run, `vibe` extracts its bundled runtime assets to
`~/.local/share/vibe/<version>/` and builds the Docker image from there.

Before `vibe run` starts Docker, it requires provider auth via supported
env vars or a readable `~/.pi/agent/auth.json`; missing auth fails early
as `setup_error`.
Supported env vars are `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`,
`GEMINI_API_KEY`, `DEEPSEEK_API_KEY`, or the Azure pair
`AZURE_OPENAI_API_KEY` + `AZURE_OPENAI_BASE_URL`.

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
`stdout` returns one machine-readable Vibe JSON result. Pi JSONL is
written to `events.jsonl` and mirrored to `stderr` for Docker log
watching. `agent.stderr.log` stores the same container stderr stream,
including mirrored JSONL and real stderr. Progress logs also stay in
`extension-events.jsonl`.

The runtime prompt instructs the agent to write exactly one conventional
commit subject to the absolute path `/artifacts/commit-message.txt` and
not to create `commit-message.txt` in the repository. Snapshot commits
use the trimmed first line from that file when present, and fall back to
`chore: snapshot changes` when the file is missing or empty.

Auth is copied into an ephemeral container home for the run and is not
persisted in the artifact directory.

Dogfood by inspecting:

- `prompt.txt`
- `commit-message.txt`
- `events.jsonl`
- `agent.stderr.log`
- `extension-events.jsonl`
- `snapshots.jsonl`
- the result commit on the worktree branch
- snapshot refs and commits under `refs/vibe/snapshots/...`
