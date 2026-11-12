# What every design doc says

Every design doc must say all of the following. Each template's
`<...>` lines name the pieces its section holds.

- **The problem, first.** Who has it, what actually happens to them
  today, with a concrete example or data, how the system came to work
  this way, and what happens if nothing changes. The clearer the
  problem, the more obvious the solution. If no solution is obvious
  yet, work on the problem before the design. A reader should finish
  it able to propose their own solution. The Overview opens with it,
  in one or more paragraphs before the design. The UI, Skills and
  `<Kind> Rules` pages also say what happens today in their area.
- **The problem type.** Name its place on Larson's staircase, and
  say in plain words what that type means:
  - A hot spot is a local pain point the design fixes directly.
  - A best practice is a known good way of working that the team has
    not adopted yet.
  - A leverage point is an interface, saved state or data model where
    extra care keeps quality as the system grows.
  - A vector alignment effort changes how the whole team or
    organization builds software.

  A hot spot fix that arrives as a platform redesign is
  over-engineered. A leverage point investment that arrives as a
  quick patch is under-designed. It is the first Decisions bullet.
- **Scope and success.** Exactly what the design covers, what it
  leaves out, and how anyone can tell it worked. The Overview ends
  with what the first version leaves out, and Decisions lists the
  rest. Steps and checkpoints gives a measurable check for each build
  step. Its last item says how, after release, to tell the problem is
  gone, with a measure two people would agree on.
- **The chosen approach.** What it is, why it wins given the
  trade-offs, and how it fits the system's existing architecture and
  direction. The Overview says it in brief, the Process table and
  Project File Structure show how it works, and each Decisions bullet
  gives one choice with its reason.
- **Alternatives.** Weigh at least two options for each non-trivial
  choice. For each option that lost, say what it would look like,
  what it gets right, and where it falls short, and explain why it
  lost in terms of the trade-offs, not just "we preferred X". It goes
  in a nested bullet or short paragraph under the choice's Decisions
  bullet. A part that settles a choice of its own compares the options
  there, such as Choices that lost on the `<Kind> Rules` page or the
  choices on the Skills page.
- **Trade-offs and risks.** Each Decisions bullet says the cost it
  accepts, including short-term shipping against long-term quality.
  How Each Step Works gives failure modes, edge cases and refusals.
  Where the user reviews closely names the risky pieces and the
  promise each keeps. Where the code lives gives the way back if the
  change fails, and Left untested gives the known gaps.
- **Leverage points.** Interfaces, saved data and state are where
  extra care keeps quality as the system changes, and they are the
  hardest to fix later.
  - Interfaces: does each one expose the essential complexity and
    hide the accidental? Test it against every real caller, not only
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
- **Open questions.** Only those that pass the test in the skill's
  Step 2, each with when to decide it and your recommendation, in
  Open questions.

Keep the doc short and reviewable. Give the riskiest parts the most
detail and keep the rest brief, so a page with little to say stays
short. A good doc in front of reviewers now beats a better one later.
