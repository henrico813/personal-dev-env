# Command Reference

## Checkout Selection

Every command except `version` needs the repository root. Resolution order is:

1. `--repo-root PATH`
2. `PDE_REPO_ROOT`
3. the current directory and its parents

A valid root contains `chezmoi/`, `planner/go.mod`, and
`pde-installer/go.mod`.

## Commands

### `pde-installer install [terminal|client|full|i3|sway] [--color-profile NAME] [--dry-run]`

Reconciles tools and managed home configuration for one selection. A fresh HOME
must specify `terminal`, `client`, or `full`; bare `install` exits with an error
when no saved or legacy selection exists. An explicit selection is saved in
`~/.config/pde/config.json`. Bare `install` reuses the saved selection. Existing
`paths.env` state or a config containing `install_path` without `profile` is
legacy full state and is saved during the next install.

`install i3` or `install sway` saves that desktop as `desktop` in `config.json`,
next to the saved profile, which it keeps. With no saved or legacy profile, it exits
before any change and says to install a profile first. Installing a profile, and
bare `install`, keep the saved desktop. A desktop needs Ubuntu 24.04 or newer
and adds its apt packages to the same `apt-get install`. The installer then
builds the desktop's wallpaper player, `xwinwrap` for i3 or `mpvpaper` for
Sway, from a pinned archive into `~/.local/bin`. chezmoi applies desktop
config only on a machine with that desktop. Changing the desktop, like
narrowing the profile, leaves the old packages and files in place. To stop
managing a desktop, delete the `desktop` key from `config.json`.

`--color-profile` accepts `tokyo-night`, `everforest-dark`, or `gruvbox-dark`.
This per-machine setting is independent of the component selection. Missing
state defaults to `tokyo-night`; the next successful install saves it as
`color_profile`. A valid explicit flag can replace an invalid saved color value.

Every selection renders tmux, Powerlevel10k, fzf, and bat colors. Client and
full installs also render Alacritty, WezTerm, Neovim, and OpenCode; terminal
installs leave any retained files unchanged until the next client or full
install. The OpenCode modifier sets the selected theme and bidirectional agent
cycling in `cli.json` while retaining other parsed values. It emits normalized
JSON, so original formatting is not retained. Restart OpenCode after changing
profiles because it reads CLI configuration at startup.

When `cli.json` lists Herdr's `./herdr-opencode` plugin, the modifier adds
`-herdr.opencode.session-selection` and `./pde-herdr-opencode` after it. The
first turns Herdr's plugin off; the second runs it through a wrapper that keeps
the pane working while background subagents of its session run. Herdr's entry
stays, so `herdr integration status` still shows opencode as current. After
installing or uninstalling Herdr's OpenCode integration, run
`pde-installer install`, then restart OpenCode: installing after an uninstall,
or installing Herdr after PDE, lists Herdr's entry after the PDE entries, so
both would report, and an uninstall leaves the wrapper without Herdr's plugin to
load.

The modifier also lists `./pde-usage` last in `plugins`, once. In an OpenCode
session, that plugin adds two lines above the prompt with the number of
subagents, how many still run, their tokens and API price, and the fullest
subagent context as a share of the model's window. Inside a subagent's view,
the lines show that subagent's task, tokens, API price and context instead.
Before the first reply they say `no usage yet`. A phone-width screen hides
the sidebar, so these lines are the part that shows in a phone terminal. On a wider screen, a
Usage panel in the sidebar lists each subagent's model, tokens, API price and
context, largest first, then the Claude plan limits that the OpenCode Claude
plugin saved last. The price uses the rates listed for the subagent's
provider, or else the rates Anthropic or OpenAI list for the model. The rates
come from OpenCode's model list, then from the models.dev catalog, which the
plugin downloads once each time OpenCode starts. The price is not what a plan
or local model bills. The plan line covers only the window the latest Claude
reply reported, five-hour or weekly, with a share only when that reply carried
one. The plugin does nothing in Claude Code.

Rendering is static and does not emit terminal palette escape sequences. An SSH
client keeps its local terminal palette while a remote PDE prompt and tmux
session use the remote host's selected RGB colors.

A successful explicit argument may switch either direction; a pre-commit failure
preserves the prior selection. `install client` fails before any change while
`~/.local/share/pde/releases` holds release tools client leaves out, such as Go
from a past full install, because client replaces that folder without them. Run
`install full` instead, or delete that folder first to give those tools up.
Switching from full to terminal stops reconciling full-only components but does
not uninstall existing full-only artifacts. Switching from terminal to full adds
the full inventory.

Every install reconciles packages, tools, runtimes, local builds, and managed
home configuration in dependency order. It migrates older PDE settings,
configures Git and chezmoi-managed content, and runs source-managed scripts.
It rejects UID 0. `--dry-run` prints ordered actions and does not mutate HOME.

An install from `main` prints one warning line on stderr when the install
record names another branch. The line names that branch, its commit and its
install time, so a branch install under test is never replaced silently. The
install still runs, and `--dry-run` prints the warning too.

The removed `update`, `config`, and `--profile` interfaces have no aliases.

Installation also configures Git's template directory at
`~/.config/git/template`. Future `git init` and `git clone` operations receive
its `commit-msg` checker. To add it to an existing repository without a
`commit-msg` hook, run:

```bash
repository=/path/to/repository
hook="$(git -C "$repository" rev-parse --path-format=absolute --git-path hooks/commit-msg)"
test ! -e "$hook"
git -C "$repository" init --template="$HOME/.config/git/template"
```

### `pde-installer doctor`

Checks the non-root user, common archive commands and a fetcher, Ubuntu and
package-manager requirements, repository metadata, and writable managed
destinations for the resolved profile. The Ubuntu check also requires the saved
desktop's release. C and C++ compilation and build-command
checks are full-profile-only; common archive and fetch checks apply to every
profile. It exits with an error if any applicable check fails.

For the client and full profiles, `doctor` also reports whether the managed
Copilot plugin and its local credential file are present. Missing Copilot files
are warnings, not doctor failures. The credential check does not validate GitHub
access; use `:Copilot auth info` in Neovim for the authenticated account's live
status.

### `pde-installer list`

Prints tab-separated columns:

```text
OWNER  ITEM  REQUESTED  INSTALLED  STATUS
```

Status words vary by owner. Common values include `missing`, `installed`,
`current`, `outdated`, `drifted`, and `unavailable`.

### `pde-installer version`

Prints the commit, branch, and UTC time of the last install on this machine
that put every component in place. It reads them from
`~/.local/state/pde/install.json`:

```text
commit 3f9c2e1a...
branch main
installed 2026-11-10T08:30:00Z
```

Each real install writes the file after every component is in place; `--dry-run`
writes nothing. A detached checkout records `HEAD` as its branch, and a field
that git cannot read records `unknown`. The record does not show uncommitted
changes in the checkout. `version` needs no checkout and works from any folder.
With no record it exits with an error that says to run `pde-installer install`.

## Global Option

`--repo-root PATH` selects a checkout instead of using the environment or
current directory.
