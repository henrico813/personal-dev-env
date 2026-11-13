# The loop for each pull request

A worker builds its card's pull requests with this loop: every pull
request a roadmap item's roadmap page lists, or a card's one pull
request. You orchestrate. Agents write the plans, you build them with
`planner export` and `git commit-tree`, and fresh agents review them.
You decide, check, ship and keep the item's page current.

The item's page is where the user reads each pull request's state.
For a roadmap item it is the roadmap page. For a card it is the card,
through `card note`, and your status file. The prompts are in
[prompts.md](prompts.md) and the commands in [build.md](build.md).

## Contents

- Before the first pull request
- Review tiers
- The loop
- Plans that wait
- After a pull request opens
- Design check
- Deciding things yourself
- Status lines
- Never
- Your last report

## Before the first pull request

1. For a roadmap item, confirm its inputs: an approved design, a
   roadmap page in the vault that lists the pull requests, and the
   branch the first pull request stacks on. If the design or roadmap
   is not approved, report a question to the hub and stop.
2. Load the domain skills of every pull request, such as
   `go-development`, besides the skills common.md's Skills names.
3. Group the pull requests into stacks. A pull request that needs
   another's code stacks on it; ones that do not can run in parallel
   stacks. A card is one stack of one pull request, on the branch the
   hub named.
4. Add a status line for each pull request to the item's page. Back
   up a vault page before every edit.
5. Before you build past the first pull request of a stack, find
   each question whose answer would hold every pull request of that
   stack, and report it to the hub. Keep building other stacks while
   it waits.

## Review tiers

Give each pull request a tier from its plan's diff, after
`planner check` passes on the first plan and again after each
revision. When unsure, pick the higher tier. Count changed lines
without lock files and generated files. A pull request's tier sets
its reviews. The run's tier, in common.md, sets which plans stop
before build, for the user's plan review or for a question to the
hub, as Plans that wait says. When a pull request's tier is higher
than the run's, the run's tier still decides whether its plan
waits. Write the higher tier in your status file and your last
report.

| Tier | A pull request is in it when |
| --- | --- |
| T1 quiet | It changes under 100 lines in at most 10 files, adds no dependency and has no T3 reason. |
| T2 skim | It is neither T1 nor T3. |
| T3 full | It has a T3 reason, listed below. |

A pull request has a T3 reason when it:

- touches a live service's files, secrets, sign-in, sops files,
  systemd units, network setup or a data schema
- deletes files
- changes one of the user's rules: a skill file, `ai/AGENTS.md`,
  `CLAUDE.md` or the front hub's `rules.md`, the user's rules file in
  the front hub's run folder, which no pull request shows
- has code that changes or deletes the user's files or data, or
  enforces a permission, approval or safety check

Write each pull request's tier on the item's page and in your status
file. Each tier gets these reviews from fresh reviewers. A fresh
reviewer never wrote the plan or code it reviews:

| Tier | Plan review, before build | Code review, after build and before the request is filed |
| --- | --- | --- |
| T1 | One reviewer for `review-plan`, `code-documentation`, `behavior-focused-testing` and the domain skills, on the review model. | One reviewer for `review-plan` and the language skills, on the review model. |
| T2, T3 | One reviewer per skill, every one on the strongest model: `review-plan` with the language skills, `code-documentation`, `behavior-focused-testing`, and each other domain skill the change names. Skill text gets the reviewers in `skill-development`'s "Review a skill change" instead of one `skill-development` reviewer. | The same reviewers, every one on the strongest model. The `review-plan` reviewer checks data safety. |

Every pull request, in any tier, gets a fresh
`code-documentation` reviewer and a fresh `behavior-focused-testing`
reviewer in its code review, before the user sees it. They run on
the review model for T1, never on the cheap model, and on the
strongest model for T2 and T3, as `model-routing` names the roles.
Language skills
get no reviewer of their own: you load them while you build, and the
`review-plan` reviewer loads them.

## The loop

Run every step for one pull request before its child starts
building. Planners and reviewers run no test suites or linters, since
a script runs them once per plan version. Before an agent reviews a
plan version, the local model runs the suites on it once, as
build.md's Run the suites once per plan version says. Reviewers read
that summary and run at most the one test a finding needs. Every job
a script checks goes to the local model first, as build.md's Send
script-checked jobs to the local model says.

1. **Plan.** For a roadmap item, or a card that links a design, write
   the pull request's design digest with build.md's Write a design
   digest. The planner and the plan reviewers ask the digest
   questions with build.md's Ask a digest a question, and never read
   the design notes. For any other card, they read the card. Then
   delegate the plan with `create-plan`. A planner stops once its
   context passes 150k and reports what is left. Start the next
   planner with prompts.md's Next part until the plan is whole.
   The plan lists each place it differs from the design under
   `### Design edits to apply` in its `## Notes for agents`. For a
   child of an unbuilt parent, make a commit of the parent's planned
   tree with build.md's Base a child on an unbuilt parent, and give
   it to the planner as its base.
   Run build.md's Set up a PR's folder as soon as the plan file
   exists, since the setup reads the plan's key. Until then, the
   first planner writes its report to a folder you make with
   `mktemp -d`. Give the plan its tier as Review tiers says.
2. **Plan review.** Fresh reviewers review the plan before build, as
   Review tiers says for its tier, with prompts.md's Fresh plan
   review. Check each required fix yourself, against the code or
   with a quick test, before you send it on; drop a false one and
   write why in the plan's notes. When a reviewer asks for changes,
   start a new planner with prompts.md's revision message, which
   names the plan, the review's report file and your decision on
   each optional item. Then run the design check below on the
   revised plan, give it its tier again, and follow Plans that wait
   before you build.
3. **Build.** Write each plan step's commit message, then run
   build.md's build script. It commits each step's tree on the step
   before, starting at the plan's base, makes the pull request's
   worktree with `wt` and runs the plan's checks. A stop at `check`
   or `lint` needs a fix to the plan or a message, then a rerun. Any
   other failed build command means a stale plan or a broken tool:
   stop this pull request and its children, put the error on the
   item's page as a blocking question, report it to the hub, and keep
   building other stacks.
4. **Code review.** After the build, start the code review's fresh
   reviewers for the pull request's tier together, as Review tiers
   says, with prompts.md's Code review, and wait for all of them.
5. **Revise.** Verify every required fix a reviewer asks for, in the
   code or by a quick test, and drop any that is false, saying why.
   Settle disagreements between reviewers, decide the optional items,
   and start a new planner for the fixes. Run the design check on the
   revised plan, then commit its tree as one fix commit. When the
   revised plan's tier is higher, run the code review of the new tier
   before the final pass. Delete a test only when a reviewer's
   finding names it and names the test that still covers its
   behavior.
6. **Final pass.** Run the suites on the plan's last version, as
   build.md says, and the comment checker and any comparison script
   the plan names, with output to a log in the PR folder. Read only
   the summary lines. A `plant` line puts one bug back into the code
   and passes only when a test fails; common.md's Reviewers also get
   shows its form. For each deleted test, write a `plant` line for
   the bug it caught, run by the test the reviewer named, in a `.md`
   file in the PR folder. Then run build.md's Put back the bugs, and
   send each `FAIL` line to a new planner as a fix. Handle each
   `stale` line yourself, as build.md says. The local model runs
   these scripts, as build.md's Send script-checked jobs to the local
   model says.
7. **Ship.** You or the review model write the pull request text,
   never the cheap model. `msg lint` cannot tell whether it reads
   well, so read it yourself and rewrite it until it says who is
   affected and why, gives the final pass's real numbers, uses plain
   words and follows `git-messages` item 5 on pull request text for
   a reader who missed the work. Then run build.md's ship
   script. It stops when the branch's tree differs from the plan, the
   text is bad, a check fails, the branch is wrong, the parent moved
   or the branch is already pushed. It squashes onto the parent,
   pushes, and stops.
   Try the pushed commit with build.md's Check a deploy, as
   pull-request Guardrail 4 says, but never
   install into the real home during a run: other workers install
   too, and the last install wins. Run it from the worktree, or
   install it into a scratch home from `mktemp -d`; a change that
   adds a dependency or a build step gets the scratch home install,
   cleanup included, since CI does not build tools the installer's
   way. In a brief for an agent that tries it, name the folder in
   every command, as in `git -C <worktree>`. Take `git status --short`
   of the main checkout both before that agent starts and after it
   ends, and move into the PR folder only the files that showed up
   while it ran. Add the one-line result, and where it ran, to
   Testing in `$D/FILE`. Write the same line, starting with the
   pushed commit's full hash, to `tested` in the PR folder.
   Then file the request:
   - While the user is away, as common.md's The run says, do not
     rerun ship.sh. Add its line to the run's `requests.sh`, which
     the user runs when back, and set NEXT to
     `queued in requests.sh`. The auto hub made that file; only
     append to it. This snippet adds the line only when that file
     does not hold it yet:

     ```sh
     . <PR folder>/env
     q='<run folder>/requests.sh'
     line=$(printf 'bash %q; sleep 60' "$D/ship.sh")
     grep -qxF "$line" "$q" || printf '%s\n' "$line" >> "$q"
     ```

     Each rerun of ship.sh skips the steps it finished and files the
     request. The `sleep` keeps requests a minute apart, since Moshi
     refuses phone notifications that come too fast. The lines keep
     merge order:
     before you add a child's line, add its parent's line the same
     way when the parent is yours, its `request.result` starts with
     `exit 3` and its line is not there yet.
   - When the user is around, rerun ship.sh now. First read the last
     line of `<run folder>/requests.log`; when it was filed less than
     a minute ago, wait for the rest of that minute. The rerun saves
     and runs the request script, which calls
     `pde-gh-write pr create --base <parent branch>`. When it files a
     new request, ship.sh adds a line to `requests.log`: the time, the
     branch and the PR folder.
   - A child stacks on its parent's branch with `--base`, and its
     line comes after its parent's. Hold a child only when it cannot
     be tried until its parent merges into the default branch, such
     as a change that needs the parent installed. A held pull request
     skips the try and files no request until its parent merges: add
     no line to `requests.sh`, set NEXT to
     `held until <parent branch> merges` and print its status line.
     After the merge, the session that carries it reruns ship.sh, as
     Your last report says.
8. **Queue the comment review.** The comment review is the user's
   review of the pull request's new and changed code comments, one
   per screen in glimmer, before the pull request merges. For the
   bottom pull request of a stack, follow `comment-review` in its
   worktree now, whether or not the user is away: `check-comments`,
   `make-feedback-request-list`, with an `--input` file of change
   requests for approved comments to change, your Agent Suggestions,
   and `send-agent-suggestions`. For a child, run nothing yet: put
   its worktree and those commands on the item's page, keep its input
   file in its PR folder, and run them after the hub says the user
   answered the parent's comment review.

Right before each pull request's build script, run build.md's Move a
PR's base. It fetches the parent branch, sets the plan's
`Base commit:` line to the parent's tip and runs `planner check`
again, so a parent that moved stops the pull request before the
build, not at Ship.

## Plans that wait

These rules say when a plan stops before build for the user. The
run's tier, in common.md, decides. The fresh reviewers' plan review
always runs first.

In any run, a plan that changes the front hub's `rules.md` waits for
the user's plan review before build, since no pull request shows that
file. Report it to the hub as a question with the plan's path, and
park that pull request and its children; keep building other stacks.
To park a pull request is to stop it and its children until the hub
sends the answer, with NEXT set to `parked until the hub answers`.

In a T1 run, no other plan stops for the user's plan review, for any
reason. That includes a plan that changes a skill file,
`ai/AGENTS.md` or `CLAUDE.md`. Build every other plan; each pull
request ends open, and the user reviews it before merge. A question
that would stop a plan in another run,
such as a new name the user types or reads or a change to what the
user sees daily, stops nothing. Pick your recommendation, and write
the choice and its alternatives in the plan's notes and in the pull
request text. The user decides at the pull request review. A pull
request whose own tier is T2 or T3 still gets that tier's reviews.

In a T2 or T3 run:

- A plan that changes what the user sees daily stops before build
  and goes to the hub as a question.
- A plan that adds a command, flag or label the user types or reads
  stops before build. Report the name question with 2 or 3 choices
  and your recommendation. Park only that pull request and its
  children, and keep building other stacks.
- These two stops are questions to the hub. `decisions.md` keeps
  them for the user. They are not the user's plan review; only a
  plan that changes the front hub's `rules.md` waits for that.
- No other plan waits for the user's plan review, whatever its own
  tier. That includes a plan that changes a skill file,
  `ai/AGENTS.md` or `CLAUDE.md`. The user reviews its pull request
  before it merges.
- For a plan whose own tier is T2 or T3, write a summary of at most
  16 lines, with the plan's decisions listed, to `summary.md` in the
  PR folder, and name it in your status file. The end brief links it
  for the user's pull request review, before merge. A T3 plan still
  gets the higher-risk plan review and code review that Review tiers
  gives it.

In any run, wording or a format the user's rules already fix, such as
the lines of a review list, is not a new choice and stops no plan,
even in what the user sees daily. Follow the rule and cite it in the
plan's notes.

## After a pull request opens

- Work on a pull request after it ships, such as a fix round or a
  carry of a parent's new commit, starts in a new session, not in
  the session that built it. The auto hub starts that session by
  `herdr-handoff` when the work comes, such as the user's feedback
  or a held pull request's parent merging. The session reads the PR
  folder's `status.txt`, the plan and common.md.
- Every later change, whether a fix, a quality pass or a parent's new
  commit, goes through the same loop: a new planner revises the plan,
  fresh reviewers give it a plan review, you run the design check,
  commit the plan's tree from `planner export` as a new commit, start
  the code review for its tier as Review tiers says, run the final
  pass, and push it.
  Update the pull request text once the hub says the user reviewed
  that pull request.
- A parent's new commit reaches its children in stack order, with
  build.md's "Carry a parent's commit to a child". If the child's
  plan does not apply on the new parent, park the child and its
  children and put the error on the item's page as a blocking
  question. Never resolve a conflict by hand.
- A feature the roadmap or card does not list needs the user's
  approval and its own pull request.
- When the hub sends findings from `<run folder>/feedback.md`, the
  user's feedback on another pull request of the run, fix each
  through the loop above before the user reviews this one.

## Design check

The design is the approved design for a roadmap item, or the design a
card links. A design is one note, or an index note that links its
part notes. For a card with no design, check the plan against the
card instead: its problem, its acceptance criteria and each note it
links, as steps 3 and 4 say.

You check each plan against the design yourself, after the fresh
reviewers' plan review and after every revision, before you commit
its tree. Because build.sh builds the branch from the plan, checking
the plan counts as checking the pull request.

1. Read the whole design before your first check, and again after
   each summary of your context. At every check, reread the
   design-edits section and the parts the pull request touches. A
   rule can sit in any part, not only in the parts the plan cites.
2. Search the whole design with `rg -n` for each file, type,
   function, command, flag and error code the plan adds or changes.
   Read every passage that names one. Add each one the digest's terms
   lack, and write the digest again for later planners and reviewers.
3. Send each difference to the planner as a fix. When the plan is
   better than the design and the difference is minor, have the
   planner add it to `### Design edits to apply` instead, with the
   design text it replaces and the new text. A difference in what the
   user sees, or in a promise the design makes, is never minor. For
   one of those, the plan keeps the design's version, and you put the
   difference on the item's page as "your call" and report it to the
   hub. When the design's version cannot work, park the pull request
   and report it to the hub as a blocking question.
4. Record each difference in the plan's notes, and whether it became
   a fix, an `orchestrator-proposed` design edit or a question.
5. When no fix is left to send, copy the plan's design edits into the
   design-edits section, as below.

The design-edits section is its own part note in a split design, or
the last section of a design kept in one note. You read all of it;
planners and reviewers read the entries the digest copies. Add it if
the design has none, and back up the design before
every edit. Each entry names the pull request, the design text it
replaces and the new text, and the replaced text gets a short pointer
to the entry. An entry replaces the text it names; the rest of the
design stays as approved. A later check of the same plan updates its
entries. When a pull request is parked or its request declined, mark
its entries parked.

## Deciding things yourself

- Decide minor things, say so in your report, and record them in the
  plan's notes as `orchestrator-proposed`. A failed build command, a
  failed check and a declined request are never minor.
- When reviewers disagree, pick one, and tell the planner which and
  why.
- Each test checks only what its name says, in the pull request that
  owns the behavior. A miss is a required correction, as
  `behavior-focused-testing` says. Never move a test into a child
  pull request.
- A question that blocks one pull request parks only that pull
  request; keep building the others. Put non-blocking questions on
  the item's page as "your call".
- Do not add code, steps or questions to carry old data, such as old
  reviews or approvals, into a new tool unless the design or roadmap
  asks for it. When old data blocks the new tool, leave it behind and
  write that as a known limit.
- When you need more than two commands or file reads to find why
  something failed or changed, read [pitfalls.md](pitfalls.md), then
  hand the search to a new agent. Brief it as prompts.md's opening
  says, and read only its report file. The checks this page tells you
  to do yourself stay with you.

## Status lines

- Write each pull request's status line from the output of build.md's
  status line snippet, never by hand. Rerun it before each loop step
  and after every push, with NEXT set to the step that comes next. It
  also writes the line at the top of the PR folder's `status.txt`.
  Below it, keep at most 19 lines: agents running and their report
  files, results you have not read, and open questions. A new session
  resumes from that file, the plan and common.md.
- Copy each status line to the item's page and to your status file.
  Detail belongs in the plans.

## Never

- Answer an approval request, run `pde-pr-approve` or set
  `PDE_GH_WRITE`.
- Send a merge or pull request edit request before common.md's
  Limits allows it.
- Edit a comment the user approved. Send the new wording as a reword
  in that pull request's comment review.
- Amend or squash a branch after its first push, or once another
  branch stacks on it; add a commit instead.
- Make a commit on a pull request's branch that you do not push, or
  try or hand the user a commit that is not pushed.
- Delete a test without a reviewer's finding that names the test still
  covering its behavior.
- Let an agent review its own plan or code.
- Hand-edit a plan's diffs or the branch's files. Every build commit
  comes from `planner export`.
- Create, copy, edit or commit a `.code-comments.json`.
- Remove a fixed `/tmp` path. Every agent works in its own folder from
  `mktemp -d`.

## Your last report

When every pull request of your card has shipped, is held or is
parked, rerun the status line snippet for each and copy its output
to the item's page. For each filed request, read its result in
`<PR folder>/request.result`. Rerun `bash <PR folder>/ship.sh` to
read it only when the request was filed less than four hours ago,
since a rerun after that files a fresh request. Never queue an expired
request again yourself: report its state and when it was filed, and
the auto hub queues it when it expires. A declined or interrupted
request is never filed again, and ship.sh refuses to; list it in
your report. For a held pull request, write on the item's page what
a new session does after its parent merges: set PARENT to the
default branch with `keep`, run
build.md's Carry a parent's commit to a child and Push a later round,
try it, add the result to Testing and `tested`, and run
`bash <PR folder>/ship.sh`.

Then report finished work to the hub, with, for each pull request:
what it does, its tier and changed lines, its stack, the worst bugs
the reviews caught, its request's state and when it was filed,
whether a comment review waits, its summary when it has one,
each choice you made in place of a question in a T1 run, and the
checks run and not run. Add your own slips and work left for
later.
