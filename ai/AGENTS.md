# AI Workflow Defaults

- Never use emojis.
- Never use the words canonical or contract.
- Keep source documentation useful and proportional to the code it explains.
- Before writing a commit message, load and follow the `git-messages` skill.
- Before opening, updating, or merging a pull request, load and follow the
  `pull-request` skill.
- Do not add AI attribution to commits or pull requests.
- Keep code comments and docstrings concise; prefer examples and 1-2 sentences.
- Never print secret values, as `sops -d` or `cat` of an `.env` or token file
  would. Run `pde-sops-peek FILE [KEY.PATH]` for a sops file's keys or one
  non-secret value. Tool output shows hidden values as `[redacted]`; never
  write that marker into a file.
- The user reads chat on a narrow screen and copies commands from
  it. Write each command you give in chat so it pastes whole: one
  line under about 35 characters, with a `cd` as its own command.
  When the steps don't fit, put them in a new script in the home
  folder, named so it replaces no file there, and give
  `bash ~/<name>.sh`. Never put a secret value in the script.
- Keep steps the user runs by hand to a few plain commands. When a
  change needs a migration, make `pde-installer install` do it. For a
  one-time change, a backup and a one-line restore command are
  enough.
- Agents have no passwordless sudo. Do not plan on `apt install` or
  other commands that need root. Get Python tools with `uvx`, and
  other programs from release archives under `~/.local/share`. When a
  step needs root, give the user one short command to run.
- Print counts and the first few failures, never whole lists such as
  every worktree. Count or filter with `jq` or `awk` first, since every
  later turn re-reads what you print.
- Decide minor choices yourself, such as a name or harmless wording.
  Say what you chose in one line and continue. Ask only when a wrong
  guess would cause rework.
- When a short message can mean more than one thing, ask before you
  answer. Ask in one line, and offer two or three readings to pick
  from.
- Ask the user questions as plain numbered text in your reply. Never
  use a tool that shows the user a question with clickable choices,
  such as `question`; the user cannot answer one on a phone.
- Before you offer to write a wrapper script, read the `--help` of
  the installed tools. One of them may already do the job.

## Skill Routing

- When the user names a skill or asks for a workflow that matches one, load that skill first, unless a command already supplied that workflow's instructions.
- After reading the user request and directly referenced context, compare the
  identified work with the available skill descriptions.
- Load each skill applicable to the requested or discovered work before
  repository research, planning, review, implementation, or substantive advice.
- Load a skill a handoff names for a later step when that step comes.
- Recheck skill applicability after a handoff, compaction, material goal change,
  or discovery of another domain the response will plan, review, implement, or
  test.
- Do not load a language skill merely because the repository contains that
  language. If a skill is loaded late, revisit affected prior work.
- When reviewing an implementation plan, pull request, commit, commit range,
  branch, or diff, load and follow the `review-plan` skill.
- When writing or revising an implementation plan, load and follow the
  `create-plan` skill.
- When implementing an approved plan, load and follow the `implement-plan`
  skill.
- When writing a design doc, load and follow the `design-doc` skill.
- When you want to put code comments in front of the user for review,
  load and follow the `comment-review` skill. Prepare reviews with
  `glimmer make-feedback-request-list`, then
  `send-agent-suggestions`.
- When cleaning up the worktree and branch of a completed plan, load and
  follow the `cleanup-plan` skill.
- When asked to have an agent do a task and review it until it is ready,
  load and follow the `delegate` skill. The subagent loads the skill for
  the task itself, such as `create-plan` or `implement-plan`; review its
  work with `review-plan` when it applies.
- When the user says "put this in my notes", or asks to add something
  to the daily note, without naming another note, load and follow the
  `obsidian-daily-note` skill. Do not ask which notes they mean, even
  though the message is short; the skill asks when the default vault
  has no daily note.
- When asked for an auto run, or to build several pull requests of
  an approved roadmap without the user reviewing each step, load and
  follow the `auto-run` skill.
- When asked to review a session for token use, efficiency, skill
  lessons or local model handoff, load and follow the
  `session-review` skill.
- When the user types `conclude-session` or asks to close out or
  wrap up a session, load and follow the `conclude-session` skill.
- When writing a status update, progress report, recap or handoff
  message to the user, load and follow the `status-updates` skill.
- When asked to hand work to a new session in Herdr, or to prepare
  long, repetitive work for a cheaper or local model, load and follow
  the `herdr-handoff` skill.
- When the user or a handoff makes this session the hub of other
  Herdr sessions, load and follow the `herdr-hub` skill.
- When choosing the model for a subagent, a new session, a worker or
  a job script, load and follow the `model-routing` skill. Its choices
  count as the user's explicit request to set a subagent's model.
- When creating, changing or reviewing an agent skill, or
  turning saved lessons into skill text, load and follow the
  `skill-development` skill.
- When delegating domain work, include an `Applicable skills:` line in the
  delegation prompt with each skill's exact name. Require the subagent to load
  available skills before working and to report required skills that are
  unavailable. Workflow skills are routed by the rules above; never name one
  on a Vibe worker's line. Load each skill you name on that line
  yourself too, so you can check the subagent's work against it.

## Code Search

Search with `rg`; for work tied to a commit, use `git grep <pattern> <commit>`. Before deleting or renaming a file, search for its path and file name.

Read a large plan with narrow `rg` for the names you need, not a broad
pattern over the whole file.

## Testing

- When planning changes to, writing, updating, or reviewing automated tests,
  load and follow the `behavior-focused-testing` skill.
- After changing anything a person sees in a terminal, such as a TUI
  screen, a Herdr popup or CLI output, load and follow
  the `screen-check` skill before you report the change done.
- When planning changes to, changing, or reviewing Go code, load and follow the
  `go-development` skill.
- When planning changes to, changing, or reviewing Rust code, load and follow
  the `rust-development` skill.
- When planning changes to, changing, or reviewing Python code, load and follow
  the `python-development` skill.
- When planning changes to, changing, or reviewing JavaScript or
  TypeScript code, load and follow the `typescript-development` skill.

## Source Documentation

- When planning changes to, writing, changing, or reviewing source code or
  automated tests, load and follow the `code-documentation` skill.
- During implementation, make supported source-documentation fixes within the
  current task. During review-only work, report findings without editing files.
- When delegating source or test work, include `code-documentation` in the
  applicable skill list.

## Planning Docs

- `notes` (alias `n`) is the preferred tool for viewing plans and
  cards. Give the user the command, and never run it yourself: it
  opens fzf and nvim. A first letter picks the note type:
  `n r <words>` for research notes, `n c <key>` for cards and
  `n p <key>` for plans, such as `n p PDEV-468`.
- Plans, issues, design docs, and research docs belong in the default
  vault. Find its folder with `nobs vault path` and documents in it with
  `nobs find`.
- Read `500 Zettelkasten/Projects/AGENTS.md`, relative to the vault
  root, before writing to the vault.
- Implementation plans are issues, not design docs.
- Use the project issue naming convention for implementation plans.
- Never commit vault documents to pull request branches.
- Back up managed config before replacing it.
- Use the installed `nobs` on `PATH`, which is `~/.local/bin/nobs` from
  pde-installer. Never use a `go build` from a `nobs` checkout: it has
  no release version, so it cannot start a broker unless `NOBS_VERSION`
  is set.
- There is no vault named `default`. The default vault is the one used
  when you leave out NAME or `--vault`. `--vault NAME` goes before the
  command: `nobs --vault NAME find "<reference>"`. `vault path` takes
  NAME after it instead (`nobs vault path NAME`). Putting `--vault`
  before `vault path` fails. Vault names come from `nobs vault list` and
  can differ between computers.
- Resolve plan and vault references with this guidance:
- 1. If the user-provided reference is an existing filesystem path, use it directly.
- 2. Track any vault name from the request. Without one, use the
  default vault.
- 3. For existing plans, docs, and notes, resolve with
  `nobs find "<reference>"`, or `nobs --vault NAME find "<reference>"`
  for a named vault.
- 4. Use `nobs vault path`, or `nobs vault path NAME`, only when
  determining a destination root for a new document or when the user
  explicitly asks for a vault root.
- 5. Ask only on `ambiguous`, `not_found`, or a real setup error: a
  nonzero exit, with a message on stderr, from a command written as
  this guidance shows. Report a setup error to the user with its
  message. Do not fix it yourself with `nobs vault set`,
  `vault default`, `vault rm`, or `tls`. An error from a command you
  wrote wrongly, such as "find requires exactly one reference", is not
  a setup error: fix your command and run it again.
- Do not use `nobs vault default` as a path lookup step. It prints the
  default vault's name, not its folder.
- Edit vault notes with your own file tools. Never use a `nobs` command
  that changes a note (`write`, `append`, `append-note`, `apply-patch`,
  `move`, `delete`). `write` and `append` change the file in place, so
  on a network drive a dropped connection can leave a note cut short,
  which sync then copies everywhere.
- Only `nobs read` accepts the full path that `nobs find` prints.
  `nobs find` and the other `nobs` commands take vault-relative paths.
- When the user asks you to sync a note to the vault, run
  `nobs sync-now`. Editing the note's file is not syncing. A sync can
  merge an older copy over your edits, so guard them:
  1. Ask the Herdr hub session that started you, or the user when no
     hub did, whether another session is editing vault notes. Wait
     until none is.
  2. Copy each note you edited to a folder outside the vault.
  3. Run `nobs sync-now`.
  4. Compare each note with its copy. Restore any note the merge
     changed, sync again, and tell the user.

## Branches and Worktrees

- Before naming a branch or writing a commit title, run
  `git config --local --get-regexp '^(msg|pde)\.'`. It prints nothing
  when the repository sets neither. A `pde.branchPattern` value
  replaces the branch rule below, and `msg.titlePrefix` allows a prefix
  before the commit type. Under a pattern, the branch starts with the
  ticket key the plan names, in the form the pattern asks for, and
  agents ask the user when the plan names none. The plan's own key is
  never the ticket key. Vibe keys and run folders still use
  `planner key`.
- Name a new branch `<type>/<words>`: a `git-messages` commit type or
  `vibe`, then lowercase letters and digits joined by single hyphens,
  at most 48 characters.
- When the work has a plan, start the words with the key that
  `planner key` prints, never one you type, and keep real words
  after it: `feat/abc-12-approvals-file`, never `feat/abc-12`. Run
  both lines in one shell call, so a file with no key stops before
  the branch exists:

  ```sh
  K=$(planner key "<plan file>") || exit 1
  wt switch --create "<type>/$K-<words>" --no-cd --format=json
  ```

  For plan `ABC-12 Approvals file.md` this makes
  `feat/abc-12-approvals-file`. The worktree folder and the pull
  request's branch then carry the key, so `rg`, `wt list` and
  `gh pr list` find all three.
  Without a plan, use plain words, such as `fix/login-timeout`.
- Keep plan and card IDs, such as `ABC-12`, out of pull request
  text, code, comments, tests, prompts and docs. Their readers cannot
  open the plan.
- Create a worktree only with
  `wt switch --create <branch> --no-cd --format=json`, and work in the
  `path` it prints. `--create` starts from the default branch, so a
  stacked branch adds `--base <parent>`. Vibe creates its own worktrees.
- Check out a pull request with
  `wt switch pr:<number> --no-cd --format=json`, and find a branch's
  worktree with `wt list --format=json`.
- Never run `wt merge` or `wt remove`. Merge with the `pull-request`
  skill, and remove worktrees with the `cleanup-plan` skill.

## Git Workflow

- Verify the exact files in a PR diff before merging.
- Use squash merges only.

## Context Size

- At each milestone, such as a merged PR, an approved plan or a
  delegated step's report, run `pde-context` where it is installed.
  When it says the context is above the 200k handoff level, propose
  handing the remaining work to a new session with `herdr-handoff`
  instead of waiting for compaction.

## Corrections

- Do not save memories in Claude Code memory files or in a `memory`
  tool's profile.
- When the user corrects agent behavior, apply the correction. When
  the correction is about a skill, check it as `skill-lessons` says
  and offer it once as a lesson. Never offer to add a correction as a
  rule to an `AGENTS.md` file.

- Agents change pull requests only through `pde-gh-write`; a human supplies approval through Herdr, Moshi, or `pde-pr-approve`. Never answer approval requests, run `pde-pr-approve`, or set `PDE_GH_WRITE`.
- Before sending a request to open or merge a PR, follow Guardrail 4
  of the `pull-request` skill.
