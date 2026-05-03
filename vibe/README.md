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

## Test

```bash
make -C vibe check
```

The default suite stays unit-focused.

```bash
make -C vibe test
```

- `make -C vibe check` runs fmt, clippy, tests, and build
- `make -C vibe test` runs the Rust test suite only
- unit tests cover result models, paths, planner helpers, prompt rendering, and
  snapshot parsing with production APIs
- real Docker and provider flows stay in manual smoke coverage, where external
  process boundaries are easier to debug deliberately

Live Docker or provider smoke checks remain opt-in manual verification.

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
vibe run "/absolute/path/to/PDEV-040 Some Plan.md" --step 1 --model anthropic/claude-sonnet-4-6
```

`vibe run --step N` refuses when step `N` already has a Vibe
result commit in the plan branch history. To address review feedback,
append a new follow-up implementation step to the plan and run that new
step.

For the MVP, step identity is the numeric planner step. After Vibe has
committed a step, do not insert, remove, or reorder implemented steps in
a way that changes already-run step numbers.

## Runtime model

- the plan worktree stays the canonical git state
- Docker is only the execution boundary
- bundled Docker, hook, and extension assets are extracted under
  `~/.local/share/vibe/<version>/`
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
- result commit on the worktree branch
- snapshot ref and commits under `refs/vibe/snapshots/...`
