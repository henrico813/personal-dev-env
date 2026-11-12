# What every design doc says

Every design doc must say all of the following. Each `<...>` line in
a template names the pieces its section holds.

- **The problem, first.** Say who has the problem and what happens to
  them today, with a concrete example or data. Say how the system came
  to work this way and what happens if nothing changes. The clearer the
  problem, the more obvious the solution. If no solution is obvious
  yet, work on the problem before the design. A reader who finishes
  the problem story should be able to propose their own solution. The
  Overview opens with it, in one or more paragraphs before the design.
  The UI, Skills and `<Kind> Rules` pages, when the doc has them, also
  say what happens today in their area.
- **The problem type.** Name its place on Larson's staircase, which
  [larson-technical-quality.md](larson-technical-quality.md) sums up,
  and say in plain words what that type means:
  - A hot spot is a local pain point the design fixes directly.
  - A best practice is a known good way of working that the team has
    not adopted yet.
  - A leverage point is an interface, saved state or data model where
    extra care keeps quality as the system changes.
  - A vector alignment effort changes how the whole team or
    organization builds software.

  A hot spot fix that arrives as a platform redesign is
  over-engineered. A leverage point investment that arrives as a
  quick patch is under-designed. The problem type goes in the first
  Decisions bullet.
- **Scope and success.** Exactly what the design covers, what it
  leaves out, and how anyone can tell it worked. The Overview ends
  with what the first version leaves out. Decisions lists everything
  else it leaves out. The Steps and checkpoints section of the
  Building It and Tickets page gives a measurable check for each build
  step. Its last item says how to tell, after release, that the
  problem is gone. It uses a measure two people would agree on.
- **The chosen approach.** What it is, why it wins given the
  trade-offs, and how it fits the system's existing architecture and
  direction. The Overview says it in brief. The Process table and
  Project File Structure show how it works. Each Decisions bullet
  gives one choice with its reason.
- **Alternatives.** Weigh at least two options for each non-trivial
  choice. For each option that lost, say what it would look like,
  what it gets right, and where it falls short. Explain why it lost
  in terms of the trade-offs, not just "we preferred X". When a part
  compares a choice's options, the choice's Decisions bullet links to
  that part. The `<Kind> Rules` page does this in its "Choices that
  lost" section. The Skills page does it in its account of each
  choice. Otherwise, each losing option goes in a nested bullet or
  short paragraph under the choice's Decisions bullet.
- **Trade-offs and risks.** Each Decisions bullet says the cost it
  accepts, including short-term shipping against long-term quality.
  The How Each Step Works page gives failure modes, edge cases and
  refusals. On the Building It and Tickets page, the "Where the user
  reviews closely" section names the risky pieces and what each must
  not break. The same page's "Where the code lives" section gives the
  way back if the change fails. The "Left untested" section of Tests
  and Samples gives the known gaps.
- **Leverage points.** Interfaces, saved data and state are where
  extra care keeps quality as the system changes. They are also the
  hardest to fix later.
  - Interfaces: does each one show callers the complexity they need
    and hide the rest? Test it against every real caller, not only
    the one that motivated the change. This goes on the Interfaces
    page.
  - Saved data: does the model rule out invalid states and still
    allow change without another migration in six months? Try it
    against several real cases. This goes in each saved file's
    section of Interfaces, and in the Types pages.
  - State: which state changes happen, what a crash or failed write
    leaves, and what stays consistent. This goes in How Each Step
    Works.
  - Sketch code as signatures and schemas, not full implementations,
    so a reviewer can judge them before code exists. The Types pages
    and the JSON examples hold these.
- **History and direction.** Where the system has been, where it is
  going, and how this change moves it along. How the system got here
  goes in the Overview's problem paragraphs, so a reader six months
  later needs no one to explain it. A change that goes against the
  current direction needs a strong reason, given in its Decisions
  bullet. Notes for Agents keeps only what agents need, such as
  sources, backups and approaches they must not bring back.
- **Open questions.** List only the questions that pass the test in
  the skill's Step 2. Put each in the Open questions section, with
  when to decide it and your recommendation.

Keep the doc short and reviewable. Give the riskiest pieces of the
design the most detail and keep the rest brief, so a page with little
to say stays short. Skip each page whose condition in the skill's
Step 4 does not hold. A good doc in front of reviewers now beats a
better one later.
