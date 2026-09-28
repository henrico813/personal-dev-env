# Runtime inputs

## Docker mounts

A run uses these relevant mounts:

- the managed worktree, writable;
- the shared Git directory, writable;
- the run artifact directory at `/artifacts`, writable;
- any explicitly supplied runtime input mounts, read-only;
- host `~/.agents/skills` at `/vibe-home/.agents/skills`, read-only, when present;
- host `~/.pi/agent` at `/vibe-home/.pi/agent`, writable, only for file authentication;
- a container `/vibe-home` tmpfs owned by the host UID/GID.

Goog runs do not mount host Pi state. They create a temporary `models.json` in
the container home with `discoverModels: true`, so discovery runs every time.

## Provider credentials

Environment authentication is selected from the provider prefix before `/` in
`provider/model`. Vibe forwards the matching group only when every variable in
that group is set:

| Provider prefix alias | Variables |
| --- | --- |
| `anthropic` | `ANTHROPIC_API_KEY` |
| `openai`, `openai-codex` | `OPENAI_API_KEY` |
| `google`, `gemini` | `GEMINI_API_KEY` |
| `deepseek` | `DEEPSEEK_API_KEY` |
| `azure-openai` | `AZURE_OPENAI_API_KEY`, `AZURE_OPENAI_BASE_URL` |
| `opencode`, `opencode-go` | `OPENCODE_API_KEY` |
| `goog` | `GOOG_BASE_URL`, `GOOG_API_KEY` |

For `goog/<model>`, set `GOOG_BASE_URL` to an API root that implements `GET
/models`. Its chat endpoint must be compatible with Pi's internal
`openai-completions` adapter. `GOOG_API_KEY` is written literally as
`$GOOG_API_KEY` in the temporary Pi configuration; endpoints that do not need
a key should still set the variable to `unused`.

The endpoint advertises model IDs without the provider prefix. For example,
LiteLLM advertises `qwen3.8`, while callers select `goog/qwen3.8`.

If a non-Goog environment group is unavailable, Vibe falls back to a readable
`~/.pi/agent/auth.json`. That fallback requires the host Pi agent directory to
be writable so Pi can persist OAuth rotation and locks. It also exposes sibling
Pi configuration, including `models.json` and `models-store.json`, and all
credentials in `auth.json` to the container.

Goog requires both environment variables and never falls back to host Pi state.

Docker networking does not make host `localhost` reachable from the container;
use an address reachable from inside Docker for the Goog endpoint.

## Shared-skill setup and validation

Vibe checks `~/.agents/skills` during setup and again at launch. Missing is
allowed. Existing paths must be directories, readable, non-symlinked, safe for
Docker mount syntax, unchanged in device/inode, and must not overlap writable
mounts. A failure returns a setup error rather than allowing Docker to create,
redirect, or rewrite the source.

## Excluded from artifacts

Authentication data is not copied into the run artifact directory. The host Pi
directory and provider environment credentials are runtime inputs, not run
artifacts. The Docker command arguments are not recorded there either.
