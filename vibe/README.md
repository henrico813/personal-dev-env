# Vibe

Day-1 MVP for running one planner step through a Docker-contained Pi runtime.

## Build

```bash
make -C vibe build
```

## Run

```bash
./vibe/target/release/vibe run "/absolute/path/to/PDEV-040 Some Plan.md" --step 1 --model anthropic/claude-sonnet-4-6
```

Artifacts land under `~/.local/state/vibe/<repo>/<branch>/runs/.../`.

Dogfood by inspecting:

- `events.jsonl`
- `agent.stderr.log`
- `extension-events.jsonl`
- final step commit on the worktree branch
- snapshot ref and commits under `refs/vibe/snapshots/...`
