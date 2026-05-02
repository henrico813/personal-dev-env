# Vibe

Day-1 MVP for running one planner step through a Docker-contained Pi runtime.

## Prereqs

- run from a git checkout of the target repo
- docker available locally
- `planner` available in `~/.claude/bin` or `PATH`
- provider auth available via env vars or `~/.pi/agent/auth.json`

## Build

```bash
make -C vibe build
```

## Install

```bash
make -C vibe install
```

This installs `vibe` to `~/.local/bin`.

## Run

```bash
./vibe/target/release/vibe run "/absolute/path/to/PDEV-040 Some Plan.md" --step 1 --model anthropic/claude-sonnet-4-6
```

## Runtime model

- the plan worktree stays the canonical git state
- Docker is only the execution boundary
- Vibe mounts the target worktree, shared git metadata, and `/artifacts`
- the container runs as the host UID/GID and sets git `safe.directory`

Artifacts land under `~/.local/state/vibe/<repo>/<branch>/runs/.../`.
`stdout` returns one machine-readable JSON result. Progress logs stay on `stderr`, `events.jsonl`, and `agent.stderr.log`.

Auth is copied into an ephemeral container home for the run and is not persisted in the artifact directory.

Dogfood by inspecting:

- `events.jsonl`
- `agent.stderr.log`
- `extension-events.jsonl`
- `snapshots.jsonl`
- final step commit on the worktree branch
- snapshot ref and commits under `refs/vibe/snapshots/...`
