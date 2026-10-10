# Pull-Request Write Approval

Coding agents post to your GitHub pull requests: they open them, comment,
change titles, and merge. A duplicate or unwanted post is visible to
everyone and has to be cleaned up by hand. On a full install, each of
these posts waits until you approve it, then happens once.

This page covers what you see, what the installer puts on your machine for
it, and why. How the program records a request and runs it only once is in
its own docs: [How a request runs gh once](../../../pdev2/docs/explanation/request-flow.md)
and [Approving in the Herdr popup or on the phone](../../../pdev2/docs/explanation/popup-and-phone-approval.md).

## What You See

1. An agent runs `pde-gh-write pr comment 12 --body "Looks good"` instead
   of `gh pr comment 12 ...`. `gh` is GitHub's command-line tool.
2. The command prints a request ID, a long string that names this exact
   request, and waits up to 90 seconds:

   ```
   pde-gh-write: waiting for approval in the Herdr popup or on the phone, or run pde-pr-approve ID in a terminal
   ```

3. If the agent runs inside Herdr, the terminal program that holds the
   agents' terminals, a small approval window, the popup, opens on top of
   them with APPROVE and DECLINE buttons.
4. If `moshi-hook` is paired with your phone, the Moshi app asks:
   `Approve gh pr comment in current repository (12)?`
5. You answer in one of those places, or with `pde-pr-approve ID` in a
   terminal. The first answer counts. After an approval, the waiting
   command runs `gh` once; after a decline, nothing is posted.

Moshi's servers receive only that question: the operation, the repository
(or `current repository` when the agent named none), the PR number or
branch, and the title when there is one. The body stays on your machine,
so use the popup or the terminal when the body matters. The popup and the
terminal work without Moshi.

How to answer is in the [how-to guide](../how-to/pr-write-approval.md).

## Which Machines Get It

The installer has two profiles, picked with `pde-installer install full`
or `pde-installer install terminal`. The full profile is the complete
install, including the AI agents. The terminal profile is a small install
of terminal tools without them.

Two tools put the approval files in place. The installer's build step
compiles this repository's own programs, such as `planner` and `vibe`.
chezmoi copies the files in this repository's `chezmoi/` directory into
your home directory; the installer runs it near the end of each install,
and running it is called an apply.

On a full install you get:

| File | Put there by | What it does |
|---|---|---|
| `~/.local/bin/pde-gh-write` | the build step, from `pdev2/` | Records the request, waits, and runs `gh` after approval |
| `~/.local/bin/pde-pr-approve` | the build step, as a link to `pde-gh-write` | Shows a request and records your answer |
| `~/.local/bin/gh` | chezmoi | The guard, described below |
| `~/.local/share/pde/herdr-plugins/pde-approval/herdr-plugin.toml` | chezmoi | Tells Herdr how to open the popup |

A terminal install gets none of them. If an earlier install left
`pde-gh-write`, `pde-pr-approve`, or the guard there, chezmoi deletes
them. Without `pde-gh-write`, a guard left behind would refuse every PR
write and offer no way to approve it.

Earlier versions installed the approval commands as bash scripts that
chezmoi copied to every machine, and showed requests in a tmux popup.

## Why One Program Under Two Names

`pde-pr-approve` is a symbolic link to `pde-gh-write`: a second name for
the same file. The program checks the name it was started under and shows
the approval screen when it is `pde-pr-approve`. With one file, the two
names always run the same build. The build step rebuilds the program when
files in `pdev2/` change, and recreates the link if it is missing. If a
later install step fails, the installer puts back the previous files.

## Why a Guard

The guard is a small script named `gh` in `~/.local/bin`, which your shell
searches before the real `gh`. It refuses PR writes such as
`gh pr create`, `gh pr merge`, and `gh api` calls that change pull
requests, and tells the agent to use `pde-gh-write`. Everything else, such
as `gh pr view`, goes to the real `gh`. After you approve, `pde-gh-write`
runs the real `gh` with `PDE_GH_WRITE=1`, which the guard lets through.

The guard stops careless mistakes, not a hostile agent that goes looking
for another copy of `gh`. Agents that were already running when you
installed may need a restart to find it.

Approval is not a permission rule in the agent harness, the program that
runs the agent, such as OpenCode or Claude Code. Harnesses apply different
rules, and you may set one to approve every command the agent asks to run.
A separate check on your machine works the same under every harness and
setting.

## Why the Popup Is Linked on Every Install

Herdr opens a popup only for a plugin it knows. A plugin manifest is a
small file that tells Herdr the plugin's name, the popup's size, 40 by 16
characters, and the command to run in it. This one names the command by
its full path, `~/.local/bin/pde-pr-approve --herdr-popup`, so Herdr does
not rely on its own search path to find it.

Linking tells Herdr where the manifest is, with
`herdr plugin link DIRECTORY`. After every apply, chezmoi runs a script
that does this, and it does only what is needed:

- Already linked to this manifest: nothing.
- Linked to another manifest, such as the older one in `pdev2/plugin/`:
  unlink it, then link this one. Herdr knows a plugin by its ID,
  `pde.approval`, alone, so only one can be linked.
- Not linked: link it.
- Herdr not installed or not running, or `jq` (a tool that reads JSON,
  used here to read Herdr's plugin list) missing: print
  `Herdr is unavailable; link later with: herdr plugin link DIRECTORY`,
  or the same message starting with `jq`.
- Herdr cannot list its plugins, or refuses the unlink or link: print
  the error, such as `Herdr plugin link failed: ...`.

The script never fails the install. Herdr may be missing or stopped
during an install, and the popup is not needed to finish it; the message
tells you the command to run later.
