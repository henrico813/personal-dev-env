---
name: design-doc
description: 'Write a technical design document covering the problem, evaluated alternatives, and chosen direction, for requests like "write a design doc for X" or "I need a design document for this feature"; for a code-bearing implementation proposal use create-plan.'
metadata:
  pde-workflow: "true"
---

# Design Doc

## Overview

A design doc is a deliberate pause before implementation. It forces
you to identify leverage points, surface hidden complexity, and agree
on the direction the system should take before writing code. The
cheapest bug to fix is the one you designed out of existence.

## When to use this skill

Use this skill to write a technical design document that reasons through the problem, alternatives, and chosen direction before implementation. Do not use it for a code-bearing implementation proposal or for plan execution; use create-plan for proposals and implement-plan for execution instead.

**Core principle:** A design doc answers "what should we build and why?" with enough rigor that reviewers can find flaws before they become code.

## When you are a delegated agent

When another agent gave you this task, research directly instead of
starting the parallel agents in Step 3. In Step 1, state your
understanding of the problem in your report and continue instead of
waiting for the user to confirm it. Write the main page to the path
you were given and each sub-page beside it, and do not start
reviewers. Report only the open questions that
pass the test in Step 2, each with its example and your
recommendation.

## Writing for Humans

The output of this process is a document for human readers. Humans
do not process information as lists of categories — they understand
through narrative. Where the design doc explains why, it should read
as a story, not a classification scheme.

**What this means in practice:**

Tell the reader *how we got here*. A problem statement isn't a list of symptoms — it's the story of what users experience today, why the system works this way, and what changed to make the current approach insufficient. The reader should feel the problem before you propose fixing it.

Show the reasoning, not just the conclusion. Each page that settles a
choice must walk the reader through the decision-making process: what
options existed, what tradeoffs each carried, why this one won. A
reader who only sees the chosen approach can't evaluate whether it's
the right one. A reader who sees the journey can spot where the logic
breaks.

Connect past, present, and future. Every design exists in a history. How did the system arrive at its current state? How does this change fit into where the system is going? A design doc without historical context is a design doc that will be misunderstood by the next engineer who reads it in six months.

Write prose where the doc explains why, such as the Overview's
problem story and each sub-page's account of its choices. The templates
in Step 4 say which sections are lists or tables, such as the
Dictionary, Decisions, rules and error codes. Keep those as lists and
give each item its reason. Under a Decisions bullet, explain each
option that lost in a nested bullet or a short paragraph.

## Initial Response

When the user requests a design doc, respond with:

1. A brief acknowledgment of the scope
2. A statement of what you'll need to gather before writing
3. Begin Step 1 immediately — don't wait for permission to start researching

**Tone:** Direct, collaborative. You're a senior engineer scoping work with a colleague, not a secretary taking dictation.

## Process Steps

### Step 1: Gather Context

Before writing anything, you MUST understand the problem space. Collect:

**From the user:**
- What problem are we solving? (symptom vs root cause — dig for root cause)
- Who is affected and how urgently?
- What constraints exist? (timeline, compatibility, team size, infra)
- What has already been tried or considered?

**From the codebase:**
- Read relevant source files, configs, and existing docs
- Identify the interfaces, stateful systems, and data models this touches — these are your quality leverage points
- Map the dependency graph: what calls what, what breaks if this changes
- Find related tests, CI config, and deployment patterns

**From project history:**
- Search for prior discussions, RFCs, or ADRs on this topic
- Check git log for recent churn in affected areas (hot spots)
- Identify existing technical debt in the area

DO NOT proceed to Step 2 until you can articulate the problem in your own words and the user confirms you have it right.

### Step 2: Analyze and Research

With context gathered, analyze before proposing solutions:

- **Identify the real problem type.** Is this a hot spot fix, a best practice adoption, a leverage point investment, or a vector alignment effort? The answer determines the weight of the solution.
- **Map what a failure would reach.** Which teams, services, and
  systems are affected? What's the rollback story?
- **Surface competing timeframes.** What does "done by Friday" look like vs "done right for the next two years"? Name the tradeoff explicitly.
- **Catalog existing patterns.** What conventions does the codebase already follow? A design doc that fights the codebase's grain needs strong justification.
- **List open questions.** What do you still not know? What
  assumptions are you making? Test each question before you bring it
  to the user: would either answer change an interface, a data model,
  or a tradeoff a future reader needs to see? If not, it is an
  implementation question; decide it yourself and record the
  decision. For each question that passes, give a concrete example
  and your recommendation in the same message you ask it in.

### Step 3: Research in Parallel with Subagents

For non-trivial designs, dispatch subagents to research in parallel:

**Agent 1 — Codebase Audit:**
Search for all files, modules, and interfaces affected by this
change. Produce a dependency map and identify the files that change
most often, such as those changed in more than half of recent PRs.

**Agent 2 — Prior Art & Patterns:**
Search for existing patterns in the codebase that solve similar problems. Look for internal libraries, shared utilities, or conventions that should be reused rather than reinvented.

**Agent 3 — Risk & Edge Cases:**
Identify failure modes, edge cases, data migration concerns, backwards compatibility issues, and performance implications. Focus on stateful systems and data model changes — these are hardest to fix later.

Synthesize all agent outputs before proceeding. If agents surface contradictory information, resolve it before writing the doc.

### Step 4: Write the design doc pages

Write the design doc as a main page and the sub-pages it needs, in
the project's vault folder. The main page is the index, and it calls
each sub-page a part. Name the main page `DESIGN-<Title>.md` and each
sub-page `DESIGN-<Title> - <Page name>.md`. Each page has a
template in [references/](references/) that lists every heading of
the page in order, with a line under each in `<...>` on what to write
there. Keep each heading, its level and its order. Add no section,
rename or reorder none, and drop one only as the rules below allow. A
heading whose line says "one heading per" repeats once for each item.
Under each heading, write what its `<...>` line asks for, in prose
where it explains why, then delete the line. Delete a `<...>` line
that asks for nothing, such as a note on row order. Keep every line
outside `<...>`. Fill in each placeholder in those lines and in the
headings, such as `<Title>`. In a `<...>` line, "an X and Y table"
means a table with columns X and Y.

Write each page whose condition below holds, and skip the rest. The
table's order is the order the main page's index lists them.

| Page | Template | Write it when |
| --- | --- | --- |
| Main page, the index | [main-page.md](references/main-page.md) | Always |
| Design Edits | [design-edits.md](references/design-edits.md) | Always |
| UI | [ui.md](references/ui.md) | The change adds or changes a screen, popup, prompt or key a person uses, or a notice shown on one. Commands, their output and error text go in Interfaces instead |
| Skills | [skills.md](references/skills.md) | The change adds or changes a command, tool or reply that a skill or agent prompt uses. Rewording a skill alone does not count |
| Interfaces | [interfaces.md](references/interfaces.md) | The change adds or changes a command, option, MCP tool or reply, or a saved file that another tool or a later version reads |
| `<Kind> Rules`, such as Style Rules | [style-rules.md](references/style-rules.md) | The change adds or changes a set of named rules the program checks input against |
| How Each Step Works | [how-each-step-works.md](references/how-each-step-works.md) | Always |
| `Types for <Module group>` | [types-for-module-group.md](references/types-for-module-group.md) | One page per group of source folders whose types or signatures the change adds or changes |
| Tests and Samples | [tests-and-samples.md](references/tests-and-samples.md) | Always |
| Building It and Tickets | [building-it-and-tickets.md](references/building-it-and-tickets.md) | Always |

When you are unsure whether a condition holds, write the page. A
skipped page keeps its row in the main page's Part table, with no
link. In place of "Read this part when", write "Skipped:" and one
line on why its condition does not hold, such as "Skipped: the change
adds no screen, prompt, key or message." Name a skipped row after its
page in the table, with `<Kind> Rules` as Rules. With no Types page,
keep one row named Types. A reader then sees every page the skill
offers and why each missing one is missing.

Inside a page you write, a section with nothing for this design, such
as MCP for a tool with no MCP server, keeps its heading. Under it,
write one line that says so and why. A heading whose placeholder
names something this design does not have is dropped instead. Do not
invent a name to fill it. For example, drop `### <Case> rules` when no
folder's code differs per case, and drop
`### <Saved file> interface design` when nothing is saved. A "one
heading per" heading with no items is dropped the same way. In the
section that held a dropped heading, write one line on what was
dropped and why. Design Edits' `E<n>` heading is the one exception,
dropped with no note. Two pages differ from the rest:

- Design Edits starts with its opening rules and a blank Edits
  section, with no `E<n>` heading and no note, because nothing has
  changed yet.
  An entry is added each time a pull request changes the design.
- Split the signatures across Types pages by group of source folders,
  and give each its own row in the index, so an agent reads only the
  signatures its work touches.

An agent reads the index and only the parts its task needs, so write
each part to stand alone. A part names each interface it uses and
links to the part that defines it. A design check, such as Step 5,
still reads every part.

The templates set where each piece goes, not whether it is there.
Before writing, read
[what-every-doc-says.md](references/what-every-doc-says.md). It lists
what every design doc must still say and which section holds each
piece. Keep the doc short: most detail for the riskiest parts.

Write each paragraph and list item on one line, so the page wraps on
a phone, and put a `---` line under each `#` and `##` heading.

### Step 5: Review Based on Larson's "Manage Technical Quality" Principles

After drafting the doc, review it against these quality lenses. These catch problems that only emerge when reading the complete draft — gaps between sections, unstated assumptions, and tensions the author is too close to see.

[references/larson-technical-quality.md](references/larson-technical-quality.md)
sums up the Larson ideas these lenses use, with links to his writing.

**Skipped pages:**
- For each "Skipped:" row in the Part table, check the page's
  condition in Step 4's table against the design. A skip whose
  condition holds is a required fix: write the page. Check each
  dropped heading's note the same way, against the condition in its
  template's `<...>` line.
- Anyone who reviews the doc runs this check too, and reports each
  wrong skip as a required fix.

**Weight calibration:**
- Re-read the problem type in the first Decisions bullet. Does the
  *actual scope* of the design match the stated problem type? A hot spot
  diagnosis that produced a platform-scale solution means either the
  diagnosis was wrong or the solution drifted. Reconcile before
  publishing.
- Could a simpler intervention — a hot spot fix, a process change, a best practice adoption — deliver most of the value? If yes and the doc doesn't explain why the heavier approach is necessary, that's a gap.

**Timeframe honesty:**
- Does the doc acknowledge tension between short-term shipping pressure and long-term quality investment? Is the tradeoff named explicitly, or is it hidden behind optimistic language? Larson: "you'll do very different work getting that critical partnership out the door for next week's deadline versus building a platform that supports launching ten times faster next quarter."

**Measurability:**
- Is the after-release measure, the last item in Steps and
  checkpoints on the Building It and Tickets page, precise enough to
  evaluate in 30/60/90 days? Could two engineers independently look
  at the same data and agree on whether the design succeeded? If not,
  sharpen it.

**Feedback loops:**
- Does the design include natural checkpoints where assumptions can be validated before committing further? Larson's core insight is that quality management is iterative — there's no winning, only learning. A design that ships as a single irreversible change has no room to learn.

**Narrative quality (final pass):**
- Read the main page, then each part in its index order. Does it tell
  a coherent story — from problem through reasoning to solution — or
  does it read like a filled-in template? If the latter, rewrite the
  sentences that link one section to the next until a human would
  actually want to read it.
- Are decisions explained with their history and reasoning, or just stated as conclusions? Every "we will do X" needs a "because Y" that the reader can evaluate.

After this review, revise the doc and present the final version to the
user. Flag any unresolved tensions, and any open questions that pass
the test in Step 2.

## Common Mistakes

- **Solutioning before understanding.** If you jump to Step 4 without Steps 1-3, the doc will be confidently wrong.
- **Boiling the ocean.** The design should solve the stated problem, not redesign the entire system. Scope creep in a doc becomes scope creep in implementation.
- **Misdiagnosing the problem type.** Treating a hot spot as a platform problem (or vice versa) produces a solution at the wrong weight. If the staircase classification doesn't match the scope of the solution, one of them is wrong.
- **Vague success criteria.** "Improve performance" is not a goal. "P99 latency under 200ms for the /search endpoint" is.
- **Ignoring existing patterns.** A novel approach in a codebase with
  established conventions needs explicit justification. Code tends to
  follow how the teams that own it are split, so don't fight that
  split without good reason.
- **Skipping alternatives.** If you only considered one approach, you didn't design — you guessed.
- **Writing a filled-in template.** If the doc reads as disconnected
  sections with no narrative thread, humans will skim it and miss the
  reasoning. The pages should connect — the Overview states the
  problem, the Decisions settle it, each part explains its share in
  the Dictionary's terms, and links lead between them.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above.
When the user corrects how you research, structure or write a design
doc, or how you ask them questions about it, follow
[skill-lessons](../skill-lessons/SKILL.md) with the skill name
`design-doc`, and offer once to save the correction as a lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
