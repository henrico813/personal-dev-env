# Rules for an auto run

The auto hub copies this file to the run folder as `common.md`, and
fills in each `<...>`. It is the run's one rules file. Every worker's
handoff and every planner's and reviewer's brief points to it, and
none copies its rules. Planners and reviewers follow The run and
Every agent; workers follow the whole file.

## Contents

- The run
- Every agent
- Workers: what an auto run is, Limits, Subagents, Skills, Reporting

## The run

- Auto hub: `<auto hub>`. Run folder: `<run folder>`.
- Run tier: `<run tier>`. pr-loop.md's Plans that wait says which
  plans stop before build in it, for the user's plan review or for a
  question to the hub.
- The user is away until `<YYYY-MM-DD HH:MM>`.
- `<rule>` until `<YYYY-MM-DD HH:MM>`, one line for each of the
  user's rules that ends at a set time.
- Models that answered the auto hub: `<models>`.

A rule with an `until` time holds until that time passes or the hub
says it ended. The hub then adds `(ended)` to its line. The away line
is one of these rules: it also ends when the user tells the hub they
are back.

## Every agent

- Decide minor things yourself and say so. Never ask the user, even
  after the away line in The run ends.
- Work only in folders you make with `mktemp -d`, except that you
  write your full report to the file your brief names. Never remove a
  fixed `/tmp` path or use another agent's folder.
- Read a skill's SKILL.md, then only the references your step needs.
- Work on large files and plans with scripts, `git show`, `rg` and
  `sed -n` ranges, never by reading them whole.
- Send test, lint and build output to a log file in your folder, and
  read only its summary lines, such as counts and the names of failed
  tests. Never run a whole test suite or linter; a script runs them
  once for each plan version.
- Do not read the design digest whole. Ask it questions with the
  digest reader, as "Ask a digest a question" in
  `~/.agents/skills/auto-run/references/build.md` says, and read
  whole only the passages your work changes.
- Never edit or commit a real `.code-comments.json`.
- Nothing public (code, docs, commits, branches, pull request text)
  names the work PC, work, private projects, personal data or
  secrets. No card or plan keys in code, docs, tests or pull request
  text. No emojis; never the words "canonical" or "contract". Never
  print secret values.
- The user's style rules: `<style rules>`.
- When `<run folder>/feedback.md` exists, follow it too. It lists the
  user's feedback on the run's pull requests.
- Reply in at most 20 lines, starting with the report's path.

Reviewers also get:

- Read-only: do not edit the worktree, commit, push or install. You
  are a delegated reviewer: review directly and start no other
  agents. Report in review-plan's format with file:line evidence.
- For each bug you want put back in the final pass, write a line to
  run as is from the repository root:
  `plant <file> '<old text>' '<new text>' <test command>`, where the
  test command is its words, or one quoted string run by `sh -c`,
  such as `'cd app && npm test'`. Put each line in a `sh` code block,
  starting with `plant`. The plant function swaps the old text, which
  must appear once in the file, for the new, runs the test command,
  and passes only when a test fails. The new text must still build,
  or a build error passes the line with no test catching the bug. Do
  not run the lines; a script runs them after the build.
- Read the test and lint results in the suites summary, and run at
  most the one test a finding needs.

## Workers

The user approved a list of cards and wants them to run without the
user, hearing about them at the end, from the hub. Your card is on
that list. Build its pull requests by
`~/.agents/skills/auto-run/references/pr-loop.md`. You never ask the
user anything, and you never wait in your own pane: every question
goes to the hub through your report file.

### Limits

- Never, without the user: merges, deploying to a live system,
  applying config to a live system, deleting user data, anything
  public beyond pushing your branches and filing their pull request
  requests.
- Send a merge or pull request edit request only after the hub tells
  you the user reviewed that pull request. Never run
  `pde-pr-approve` or set `PDE_GH_WRITE`. Never answer an approval
  shown in another pane.
- Two sessions never edit the same file, branch or worktree.
  The one exception is the run's `requests.sh` and `requests.log`,
  to which workers only append, as Reporting says.
- Work the card does not ask for needs its own card: report it to
  the hub.

### Subagents

- Pick each subagent's model as `model-routing` says. Planners run
  on the strongest model, since only it has the long context a large
  plan needs. Reviewers run on the model pr-loop.md's Review tiers
  gives for the pull request's tier.
- Pull request text is yours or the review model's, never the cheap
  model's.
- Every job a script checks goes to the local model first, as
  build.md's Send script-checked jobs to the local model says. Brief
  it and check its result yourself, as `model-routing` says.
- When a model fails with a limit or a login error, write the error
  and the time in your status file and do not retry in a loop. Pick
  the next model as `model-routing` says. When `model-routing` leaves
  no model to try, report a blocker.
- A report can say a skill was loaded when it was not. Before you
  trust a review, read the `skills loaded` line of
  `python3 ~/.agents/skills/session-review/scripts/session_stats.py
  --session <reviewer session>`, the session ID the subagent tool
  returned.
- After a restart, before you start a planner, look for a plan file
  made in the last hour and a planner still running, as pitfalls.md's
  first entry says. If you find either, let that planner finish and
  use its plan.

### Skills

Load `create-plan`, `review-plan`, `pull-request`, `git-messages`,
`comment-review` and `status-updates` before your first pull request,
the language skill of each language a pull request changes, and each
skill you name on a subagent's `Applicable skills:` line, such as
`code-documentation`. After your context is summarized, load the
skills of your current step again.

### Reporting

- Your hub is the auto hub above. Report only to it.
- Keep `<run folder>/status-<your agent name>.md` current. Its first
  line is one sentence on where the card stands, such as "PR 2 of 4
  reviewed; building it." The user reads that line on "status
  update". Below it, keep each pull request's status line from
  build.md's Print a PR's status line, with its tier. Add a
  `card note` at each milestone, one `card` call per shell call,
  since each writes to the vault on the network drive and can be
  slow.
- Write a question, a blocker or finished work with
  `hub report --run "<run folder>" <your agent name> <file>`, which
  replaces your old report whole. After a question, park that pull
  request and its children, as pr-loop.md's Plans that wait says,
  keep building other stacks, and end your turn when no stack can go
  on. After a blocker or finished work, end your turn at once. Update
  your status file and card before the report, never after. Never
  prompt the hub. Ask questions as `ai/AGENTS.md` says. A
  question gives 2 or 3 choices, what each leads to, and your
  recommendation. Finished work gives the checks run, with results,
  and the checks not run. Progress-only news, such as a passed
  rehearsal, goes only in your status file.
- Wait outside your chat on a long job you start yourself, such as
  a build. A job a local model runs for you, as Subagents says, ends
  before the model replies, so it needs no wait. First
  run `hub wait --run "<run folder>" <your agent name>` with
  `--file <summary file> --deadline <minutes>`. Then start the job
  with `setsid nohup bash <script> > <log> 2>&1 &`, so it outlives
  your tool call, and end your turn. A plain `&` job can end with the
  tool call. The script writes its summary file in one step at its
  end, and the watch prods you when that file changes or the
  deadline passes.
- Edit only your card through `card`, your plan notes, your item's
  roadmap page, your worktrees and branches, and your own files in
  the run folder.
- Every worker shares two files in the run folder, which the auto
  hub makes before workers start. Only append to them, with `>>`:
  `requests.sh`, the requests queued for the user, and
  `requests.log`, one line for each new request filed, which ship.sh
  writes.
- Near 150K context, write a handoff by `herdr-handoff`. It points to
  this file, your status file and each PR folder's `status.txt`, and
  copies none of their rules. Record it in your status file and
  report it to the hub instead of starting a session yourself.
- When the hub says your card is accepted or dropped, check that you
  hold nothing unsaved, then end your turn. Never close your own tab;
  the hub closes or parks it.
