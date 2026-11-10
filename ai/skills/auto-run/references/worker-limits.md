# Limits for auto run workers

The auto hub copies this file to `<run folder>/common.md` and fills
in `<auto hub>`, `<run folder>` and `<models>`. Each worker's handoff
points to it.

## What an auto run is

The user approved a list of cards and wants them to run without the
user, hearing about them at the end, from the hub. Your card is on
that list. It stops at a pull request the user can read and review,
or at a question for the user. You never ask the user anything, and
you never wait in your own pane: every question goes to the hub
through your report file.

## Limits

- Never, without the user: merges, deploys, applying config to a
  live system, deleting user data, anything public beyond pushing
  your branch.
- Plan review: a plan that two fresh reviewers both call ready, with
  their fixes applied, may go to build without the user's read. Run
  the two reviews as fresh subagents, never as a review in your own
  session: (1) `behavior-focused-testing`; (2) `review-plan`,
  simplification and `code-documentation`. Each reviewer gets an
  `Applicable skills:` line and loads them first. When a plan goes
  back for fixes, start a fresh planner subagent with the plan path
  and the findings; never resume the earlier planner.
- Check each reviewer's required fix yourself, against the code or
  with a quick test, before you send it to the planner. Drop a false
  one and write why in the plan's notes. When reviewers disagree,
  pick one and tell the planner which and why.
- Delete a test only when a reviewer names it and the test that
  still covers its behavior.
- A plan that changes one of the user's rules, such as
  `ai/AGENTS.md` or a line of the hub's `rules.md`, or what the user
  sees daily, stops before build and reports to the hub, even though
  the card is approved for the run.
- New names: if your plan adds a command, flag or label the user
  types or reads, stop before build. Report the name question with 2
  or 3 choices and your recommendation, then end your turn.
- Code review after the build, each by a fresh agent: one for
  `review-plan` and the language skills, one for
  `behavior-focused-testing` and one for `code-documentation`. A
  risky change gets one agent per language skill too, and its
  `review-plan` agent checks data safety. A change is risky when its
  code changes or deletes the user's files or data, or enforces a
  permission, approval or safety check; when unsure, it is risky.
  Leave the plan out of the `code-documentation` and
  `behavior-focused-testing` briefs, so their checks stay fresh. Put
  required fixes in the plan through a fresh planner, then build
  them.
- Try the change before you write the pull request text, as
  `pull-request` Guardrail 4 says, but never install into the real
  home during a run: other workers install too, and the last install
  wins. Run the change from your worktree, or install it into a
  scratch home from `mktemp -d`. Say which in Testing. A change that
  adds a dependency or a build step gets the scratch home install,
  cleanup included, since CI does not build tools the installer's
  way.
- Hold the pull request: build, test, commit by `git-messages`, push
  your branch, and write the pull request title and body to
  `<run folder>/pr-<your agent name>.md`, title on the first line.
  Read and rewrite that text yourself, since `msg lint` cannot judge
  it. Then report and stop. Send the create request only when the
  hub tells you to, through `pde-gh-write` by `pull-request`, and
  report its approval ID.
- After you send the create request, every change is a new commit;
  never amend or squash. Never deploy or hand on a commit you have
  not pushed.
- Send a merge or pull request edit request only after the hub tells
  you the user reviewed that pull request. Never run
  `pde-pr-approve` or set `PDE_GH_WRITE`. Never answer an approval
  shown in another pane.
- Nothing public (code, docs, commits, branches, pull request text)
  names the work PC, work, private projects, personal data or
  secrets. No card or plan keys in code, docs, tests or pull request
  text. No emojis; never the words "canonical" or "contract". Never
  print secret values. Never commit a `.code-comments.json`.
- Make a fresh worktree with
  `wt switch --create ... --no-cd --format=json` by the global
  branch rules. Two sessions never edit the same file, branch or
  worktree.
- Decide minor things yourself and record each in the plan's notes
  as `orchestrator-proposed`. A failed build, a failed check and a
  declined request are never minor. Work the card does not ask for
  needs its own card: report it to the hub.

## Subagents

- Models: planners run on `claude-code/claude-opus-5-5[1m]`.
  Reviews run on `claude-code/claude-sonnet-5#medium`, except the
  data safety review of a risky change, which runs on Opus. The auto
  hub checked which answer: `<models>`.
- Every brief says: work only in folders from `mktemp -d`, and never
  remove a fixed `/tmp` path or another agent's folder; read large
  files and plans with `rg` and `sed -n` ranges, never whole; send
  test and build output to a log and read only its summary lines;
  reply in at most 20 lines, starting with the report's path.
  Reviewers also get: read-only, start no other agents.
- A planner writes the brief's decisions into the plan's
  `## Notes for agents` first, so a planner that stops still leaves
  a usable plan.
- No agent reviews its own plan or code.
- A report can say a skill was loaded when it was not. Before you
  trust a review, read the `skills loaded` line of
  `python3 ~/.agents/skills/session-review/scripts/session_stats.py
  --session <reviewer session>`, the session ID the subagent tool
  returned.
- After a restart, before you start a planner, look for a plan file
  made in the last hour and a planner still running. If you find
  either, let that planner finish and use its plan.
- When a subagent tests the change, give every command its folder,
  such as `git -C <worktree>`. Run `git status --short` in the main
  checkout before and after it, and move only files that appeared
  during its run into your run folder.
- When a model fails with a limit or a login error, write the error
  and the time in your status file and do not retry in a loop. For a
  review, try another model that answered. A planner has no other
  model. When no model is left, report a blocker.

## Skills

Load `create-plan` before you write the plan, `implement-plan`
before you build, the language skill of each language you change,
`behavior-focused-testing` before you write tests,
`code-documentation` before you change source, `git-messages` before
each commit, `pull-request` before you write the pull request text,
`status-updates` before you write a report, and each skill you name
on a subagent's `Applicable skills:` line, such as `review-plan`.
After your context is summarized, load the skills of your current
step again.

## Reporting

- Your hub is `<auto hub>`. Report only to it. Run folder:
  `<run folder>`.
- Keep `<run folder>/status-<your agent name>.md` current. Its first
  line is one sentence on where the card stands, such as "Plan
  reviewed ready; building step 2 of 3." The user reads that line
  on "status update". Add a `card note` at each milestone, one
  `card` call per shell call.
- Write a question, a blocker or finished work to
  `<run folder>/report-<your agent name>.md`, replacing its old
  text, then end your turn. Never prompt the hub, and never use an
  interactive question tool. A question gives 2 or 3 choices, what
  each leads to, and your recommendation. Finished work gives the
  checks run, with results, and the checks not run.
- Edit only your card through `card`, your plan note, your worktree
  and branch, and your own files in the run folder.
- Near 150K context, write a handoff by `herdr-handoff` and report it
  to the hub instead of starting a session yourself.
- When the hub says your card is accepted or dropped, check that you
  hold nothing unsaved, then close your own tab with
  `herdr tab close <tab>`, the tab being `.result.agent.tab_id` of
  `herdr agent get "$HERDR_PANE_ID"`.
