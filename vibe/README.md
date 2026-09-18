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

The live integration test is compiled but not executed by `make check` or
`make test`. Run it explicitly on Linux with:

```bash
make -C vibe integration
```

The explicit test requires Docker, GNU `timeout`, network access, and valid
`~/.pi/agent/auth.json` credentials. It runs the actual Vibe binary and Pi
provider call. The test uses an isolated home for Vibe state but links the real
Pi agent directory so OAuth refreshes persist. Pi may update the credentials as
it would during a normal Vibe run.

## Install

```bash
make -C vibe install
```

If you use PDE, `pde-installer install` installs the
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
`OPENCODE_API_KEY`, `GEMINI_API_KEY`, `DEEPSEEK_API_KEY`, or the Azure pair
`AZURE_OPENAI_API_KEY` + `AZURE_OPENAI_BASE_URL`. For environment
authentication, Vibe selects the credential group matching the provider prefix
in `--model` and forwards only that group; unknown providers require Pi file
authentication.

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

Choose a `provider/model` selector before invoking Vibe. Vibe passes `--model`
unchanged, and Pi uses that selector during the agent run. Bare model names can
be ambiguous across Pi providers.
When Pi auth is used, Vibe mounts the host Pi agent directory writable so Pi can
persist refreshes and locks. Authentication data is not copied into run artifacts.

Add `--insecure-tls` only if you need to bypass certificate verification in
Docker; it sets `NODE_TLS_REJECT_UNAUTHORIZED=0` inside the container and
reduces TLS security.

Use `--base <revision>` to seed a new managed worktree from any Git revision,
such as a local `feature/demo` branch or `origin/feature/demo`. Without it,
Vibe keeps the existing behavior of fetching and branching from resolved
remote `main`. Reusing a `--key` keeps its existing managed branch and must omit
`--base`; Vibe rejects `--base` when that branch or worktree already exists.
Concurrent runs whose keys normalize to the same slug are rejected.

## Runtime model

- the managed worktree stays the canonical git state
- Docker is only the execution boundary
- bundled Docker, hook, and extension assets are extracted under
  `~/.local/share/vibe/<version>/`
- Vibe mounts the target worktree, shared git metadata, and `/artifacts`
- when present, host `~/.agents/skills` is mounted read-only at
  `/vibe-home/.agents/skills` so Pi can load supervisor-named shared skills
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
Wrapper-owned recovery state now lives in authoritative `run.json` beside
derived `summary.json`, best-effort derived `result.json`, and `vibe.log`.
Each key also maintains append-only `runs_index.jsonl` as a discovery aid,
not the source of truth for a run. Vibe seeds an empty `snapshots.jsonl`
for every run so no-op runs still have a durable snapshot log path.

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

Vibe's Docker image pins its own `@earendil-works/pi-coding-agent` release.
The `--model` selector is passed unchanged to that runtime, and Vibe does not invoke the host `pi` executable.
The host Pi agent directory is mounted writable at `$HOME/.pi/agent` inside the container.
This lets Pi persist rotated OAuth credentials and share its refresh lock across host and Vibe processes.
When `auth.json` supplies authentication, the same mount exposes sibling configuration such as `models.json` and `models-store.json`.
Auth is never copied into the run artifact directory.
Prefer provider API keys for disposable or concurrent automation.
The shared-skill mount is independent of provider authentication. A directory
missing at setup or launch-time revalidation is allowed for standalone Vibe
use. A non-directory, symlinked, changed, unreadable, or Docker-unsafe resolved
path fails validation instead of letting Docker create, redirect, or rewrite
it. These checks narrow same-user replacement races but cannot eliminate the
interval between the final check and Docker resolving the bind source. Pi disables normal skill discovery and loads the mounted host directory
explicitly, so a project skill cannot shadow a reviewed host skill. Vibe treats
every installed host skill as trusted executor input; the read-only mount
protects host files from writes but does not vet their instructions.

Recovery notes:

- `run.json` is the only authoritative per-run record.
- `summary.json` is the default status-shaped derived view.
- `result.json` is the saved command-result view derived from `run.json`.
- `run-state.json` is no longer part of the supported status path.
- `runs_index.jsonl` is a best-effort lookup index that may be rebuilt.
- Late persistence failures populate durable `persistence_error` fields without rewriting the execution `status`.
- `vibe status --key ...` reads the latest readable persisted state for the normalized key.
- `vibe status` must be run from inside the target repo checkout.
- Latest-run lookup is run.json-only.
- `vibe status --long` shows the full saved run record.

Dogfood by inspecting:

- `prompt.txt`
- `system-prompt.txt`
- `combined-prompt.txt`
- `system-prompt-versions.txt`
- `commit-message.txt`
- `events.jsonl`
- `agent.stderr.log`
- `extension-events.jsonl`
- `run.json`
- `summary.json`
- `runs_index.jsonl`
- `result.json`
- `vibe.log`
- `snapshots.jsonl`
- the result commit on the worktree branch
- snapshot refs and commits under `refs/vibe/snapshots/...`
