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

Before `vibe run` starts Docker, it reads `--prompt-file` as UTF-8,
renders the immutable executor prompt contract in `src/prompts.rs`, and
requires provider auth via supported env vars or a readable
`~/.pi/agent/auth.json`; missing auth fails early as `setup_error`.
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
  --stderr-level info \
  --commit-message "docs: update README note"
```

## Runtime model

- the managed worktree stays the canonical git state
- Docker is only the execution boundary
- bundled Docker, hook, and extension assets are extracted under
  `~/.local/share/vibe/<version>/`
- Vibe mounts the target worktree, shared git metadata, and `/artifacts`
- the container runs as the host UID/GID and sets git `safe.directory`
- `prompt.txt` stores the raw UTF-8 supervisor prompt
- `system-prompt.txt` stores the rendered executor system prompt
- `combined-prompt.txt` stores the system prompt plus task prompt
- `system-prompt-versions.txt` stores the executor contract version plus per-prompt versions
- `run-agent.sh` reads only `VIBE_COMBINED_PROMPT_FILE` inside Docker

Artifacts land under `~/.local/state/vibe/<repo>/<slug>/runs/.../`, where
`<slug>` is the normalized `--key` value.
`stdout` returns one machine-readable Vibe JSON result. `events.jsonl`
always stores the full raw Pi JSONL stream. `stderr` is a structured
presentation channel controlled by `--stderr-level` or
`VIBE_STDERR_LEVEL`, and `agent.stderr.log` stores the same structured
or raw stream seen by the caller. Docker build logs are always
suppressed. Other container stderr remains pass-through today and is not
level-filtered. Progress logs also stay in `extension-events.jsonl`.

Supported stderr levels are `error`, `warn`, `info`, `debug`, and
`trace`. Use `info` for Codex-supervised runs, because it emits compact
human-readable progress without replaying the full machine log into the
supervisor context. Setup failures continue to surface in the final JSON
result.

| Signal | error | warn | info | debug | trace |
| --- | --- | --- | --- | --- | --- |
| structured warnings or fallbacks | no | yes | yes | yes | no |
| structured lifecycle summaries | no | no | yes | yes | no |
| failed tool summaries | yes | yes | yes | yes | no |
| successful tool summaries | no | no | yes | yes | no |
| snapshot subject | no | no | yes | yes | no |
| changed filenames | no | no | no | yes | no |
| diff stat | no | no | no | yes | no |
| docker build logs | no | no | no | no | no |
| other container stderr | pass-through | pass-through | pass-through | pass-through | pass-through |
| raw JSONL | no | no | no | no | yes |

The runtime prompt instructs the agent to keep exactly one conventional
snapshot subject in the absolute path `/artifacts/commit-message.txt`,
not to create `commit-message.txt` in the repository, and not to run
`git commit`. If the task is clear, the agent may write an initial
single-line subject before editing repository files and should update
that same one-line artifact before the run finishes based on the actual
changes. The subject should omit the optional scope by default, such as
`feat: add setting`, unless the user explicitly asks for a scope. Vibe
uses the trimmed first line from that artifact for snapshot commits when
present, and falls back to `chore: snapshot changes` when the file is
missing or empty.

Auth is copied into an ephemeral container home for the run and is not
persisted in the artifact directory.

Dogfood by inspecting:

- `prompt.txt`
- `system-prompt.txt`
- `combined-prompt.txt`
- `system-prompt-versions.txt`
- `commit-message.txt`
- `events.jsonl`
- `agent.stderr.log`
- `extension-events.jsonl`
- `snapshots.jsonl`
- the result commit on the worktree branch
- snapshot refs and commits under `refs/vibe/snapshots/...`
