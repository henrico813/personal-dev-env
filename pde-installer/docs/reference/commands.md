# Command Reference

## Checkout Selection

Every command except `version` and `update` needs the repository root.
`update` finds its checkout as its section below says. The others resolve the
root in this order:

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
session, that plugin adds a short line above a parent session's prompt with
the number of subagents and their API price, such as `3 agents · $1.94 cost`.
Setting `PDE_USAGE_LINE=left`, or the plugin option `line: "left"`, shows the
fullest subagent's context left instead. When a subagent finishes, a green
line above it names the subagent, its tokens, run time and price for 10
seconds. Inside a subagent's view, two lines above the prompt show its model,
a context bar, its tokens, price and whether it still runs. The Usage panel in
the sidebar shows the group totals, how many subagents still run, their tokens
and API price, and the plan lines of the plans the session used, plus any plan
at 80% or more. A phone-width screen hides the sidebar, so `/usage` opens a
screen with the same totals and every plan line, and esc goes back.
The price uses the rates listed for the subagent's provider, or else the rates
Anthropic or OpenAI list for the model. The rates come from OpenCode's model
list, then from the models.dev catalog, which the plugin downloads once each
time OpenCode starts. The price is not what a plan or local model bills, and a
model with no listed rate shows `$0 cost`. The plugin does nothing in Claude
Code.

A plan line shows how much of a plan's limit is used, such as `Claude weekly 48%
used`, with its reset time on the line below. A number 10 minutes old or more
shows its age, such as `· 2h ago`. When the latest check failed, the last good
number stays with its age and `check failed` below it, and a window that has
reset since shows `unknown`. A plan line turns the warning color at 80% and the
error color at 95%. `Claude limit reached until 14:00` shows while the OpenCode
Claude plugin reports that Claude has stopped answering. A machine with no
Claude Code login or plan shows no Claude line.

`pde-quota` keeps these numbers in `~/.local/state/pde/quota/`, one file per
source, and the plugin only reads them. `pde-quota show` prints the same plan
lines in any shell, `--json` prints them with their numbers for other tools, and
`--width N` sets the line width, 37 by default. When a number is due, `show` and
the plugin start `pde-quota refresh` in the background. It asks Claude Code for
the account's plan usage at most every 5 minutes, through `claude -p` in an
empty folder with the user's own Claude Code settings skipped for that call, so
their "no non-essential traffic" setting does not empty the answer. Telemetry,
error reports and auto-update stay off, and no model runs. A lock keeps many
OpenCode windows to one check, and `checks.log` beside the files records each
check. A Claude Code status line can pipe its JSON into `pde-quota
save-claude-status`, which saves its two plan windows, prints nothing and never
fails. Deleting the folder is safe; the next refresh rebuilds it.

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

After every component is in place, install looks for processes of the current
user that still run a binary it deleted from `HOME`. When `hub-watch.service`
runs a replaced `hub`, install restarts it with `systemctl --user try-restart`.
For each other such process it prints a warning line on stderr with the PID and
process name, so you can restart it yourself. A failed check or restart only
warns. `--dry-run` prints the planned check.

The removed `config` and `--profile` interfaces have no aliases.

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

### `pde-installer update`

Fast-forwards the checkout to the newest `main` and installs it. A timer or a
script can run it from any folder, with no terminal.

It finds the checkout from `--repo-root`, then `PDE_REPO_ROOT`, then
`install_path` in `~/.config/pde/config.json`, which every install saves. It
never uses the current directory.

It fetches `main` from the remote that the checkout's `main` tracks, or from
`origin`. A GitHub SSH address is fetched over HTTPS, so no SSH key is needed.
Set `update_url` in `config.json` to fetch from another address. A fetch that
fails or takes more than two minutes is tried once more.

It skips the update, and changes nothing, when the checkout is not on `main`,
has uncommitted changes to tracked files, or has commits that `main` does not
have. Untracked files do not block it. It never stashes, resets, or forces.

When `install.json` already names the fetched commit on `main`, it stops there.
Otherwise it builds `pde-installer` from the checkout into
`~/.local/state/pde/update-stage`, runs `install` with that build, and only
then moves the build to `~/.local/bin/pde-installer`. A failed build or install
keeps the old installer, and a failed install puts back what it changed. The
checkout stays at the new commit, so the next run tries the install again.

The install runs with no terminal. When an apt package is missing, `sudo`
fails at once instead of waiting for a password; run `pde-installer install` by
hand then. Install also skips its question about restarting OpenCode servers.

Each run writes `~/.local/state/pde/update.json`:

```json
{
  "result": "installed",
  "commit": "3f9c2e1a...",
  "reason": "",
  "checked_at": "2026-11-13T08:30:00Z"
}
```

`result` is `installed`, `current` when that `main` was already installed,
`skipped`, or `failed`, and `reason` says why for the last two. On `installed`
and `current`, `update` prints `update <result>: main is at <commit>`.
Otherwise it exits with an error that gives the result and reason. A second
update started while one runs exits at once with an error.

## Global Option

`--repo-root PATH` selects a checkout instead of using the environment or
current directory.
