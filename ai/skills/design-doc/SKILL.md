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
on the direction the system should take before writing code. A
leverage point is an interface, saved state or data model, where
extra care keeps quality as the system changes. The cheapest bug to
fix is the one you designed out of existence.

## When to use this skill

Use this skill to write a technical design document that reasons through the problem, alternatives, and chosen direction before implementation. Do not use it for a code-bearing implementation proposal or for plan execution; use create-plan for proposals and implement-plan for execution instead.

**Core principle:** A design doc answers "what should we build and why?" with enough rigor that reviewers can find flaws before they become code.

## When you are a delegated agent

When another agent gave you this task, research directly instead of
starting the parallel agents in Step 3. In Step 1, state your
understanding of the problem in your report and continue instead of
waiting for the user to confirm it. Write the main page, which is the
doc's index, to the path you were given. Write each part, one of the
doc's other pages, beside it. Step 4 says which pages to write. Do
not start reviewers. Whoever gave you the task starts the fresh
review that follows Step 5. Report only the open questions that pass
the test in Step 2, each with its example and your recommendation.

Write the doc in parts, one part per writer. After each section you
finish, run `pde-context --context`. It prints your context, such as
`86k of 1M (9%)`. Once the first number is past 150k, stop. Before
you stop, write each choice you made so far as a bullet in the main
page's Decisions. Then end your report with `Left:` and the pages and
sections not yet written. A page you stopped partway counts as not
written, and `Left:` names its missing sections. When the command
prints `no session ID` instead of a number, stop after each page.

When a writer's report ends with `Left:`, the agent that delegated
to it starts a new writer with the same brief and the `Left:` list.
The new writer reads the brief's inputs, such as a digest, and the
pages already written. It then writes what is left.

## Writing for Humans

The output of this process is a document for human readers. Humans
do not take in information as lists of categories. They understand
through narrative. Where the design doc explains why, it should read
as a story, not a classification scheme.

**What this means in practice:**

Tell the reader *how we got here*. A problem statement isn't a list of symptoms — it's the story of what users experience today, why the system works this way, and what changed to make the current approach insufficient. The reader should feel the problem before you propose fixing it.

Show the reasoning, not just the conclusion. Wherever the doc weighs
a choice, it must walk the reader through the decision-making
process: what options existed, what tradeoffs each carried, why this
one won. A reader who only sees the chosen approach can't evaluate
whether it's the right one. A reader who sees the journey can spot
where the logic breaks.

Connect past, present, and future. Every design exists in a history. How did the system arrive at its current state? How does this change fit into where the system is going? A design doc without historical context is a design doc that will be misunderstood by the next engineer who reads it in six months.

Write prose where the doc explains why, such as the Overview's
problem story and each part's account of its choices. The templates
in Step 4 say which sections are lists, such as the Dictionary and
Decisions. They also say which are tables, such as the Rules page's
rule table and the Interfaces page's error-code table. Keep each list
and table in the form its template sets. Give each item that records
a choice or rule its reason. Give every choice one bullet in the main
page's Decisions. When a part compares the options, the bullet gives
the winner and its reason, and links to that part. Otherwise, explain
each losing option in a nested bullet or short paragraph under the
bullet.

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
  [larson-technical-quality.md](references/larson-technical-quality.md)
  explains each type.
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
  and your recommendation in the same message you ask it in. Ask
  every question that passes in one message. Go on without waiting
  for the answers. Put each question still unanswered in the main
  page's Open questions section.

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

Unless you are a delegated agent, start subagents to write the pages.
Pick their model with the `model-routing` skill, which names a cheap
model for long drafts. Check each page against Step 5 as it comes
back. Send your findings to the subagent that wrote it, until the
page has no findings left. After the last page, run all of Step 5 on
the whole doc.

Write the design doc as a main page and the parts it needs. The main
page is the index of the parts. Put the pages in the project's folder
of the default vault. The Planning Docs section of your AGENTS.md
says how to find that folder. It also points to the vault's rules for
names and frontmatter. Name the main page `DESIGN-<Title>.md` and
each part `DESIGN-<Title> - <Page name>.md`. When the vault's rules
set another prefix, use it in place of `DESIGN-` everywhere. That
includes each `id` and each link in the template lines outside the
`<...>` notes. The next paragraph explains those notes. In the
frontmatter, set `project` to the project folder's name. Set `topics`
to links to the subjects the design is about, such as `"[[Rust]]"`.
When the vault's frontmatter rules differ from a template's
frontmatter, follow the vault's rules.

Each page has a template in [references/](references/). The template
lists every heading of the page in order. Under most headings, a note
in `<...>` says what to write there. This skill calls that whole note
a `<...>` line, even when it wraps over several lines. Leave a heading
with no `<...>` line as the template has it. Keep each heading, its
level and its order. Do not add, rename or reorder a section. Drop a
section only where the rules below allow it. When a heading's `<...>`
line says "one heading per", write that heading once for each item
the line names. Under each heading, write what its `<...>` line asks
for, then delete the line. Use prose wherever your text explains why.
Delete a `<...>` line that asks for no text under its heading, such
as a note on row order. Keep every line outside `<...>`. Fill in each
placeholder in those lines and in the headings, such as `<Title>`.
In a `<...>` line, "an X and Y table" means a table with columns X
and Y.

A placeholder means the same thing on every page. `<Name>` is the
name of the main page's `<Name> Process`, such as Review. `<Tool>` and
`<program>` both stand for the program's short name, such as review
tool.

The templates use the word orchestrator for the agent that builds the
design's pull requests in order, such as a session that runs the
`auto-run` skill. In the templates, the user is the person who
asked for the design. That person reviews the doc and its pull
requests, and often runs the program too. When other people run the
program, name them on the page instead of writing "the user".

Write each page whose condition below holds, and skip the rest. The
main page's index table lists the pages in this table's order.

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
skipped page keeps its row in the main page's index table, with no
link. In its "Read this part when" cell, write "Skipped:" and one
line on why its condition does not hold, such as "Skipped: the change
adds no screen, prompt, key or notice." Name a skipped row after its
page in the table above. For `<Kind> Rules`, name the row Rules. With
no Types page, keep one row named Types. A reader then sees every page
the skill offers and why each missing one is missing.

Some sections have nothing for this design, such as the MCP section for
a tool with no MCP server. Such a section keeps its heading. Under it,
write one line that says so and why. Drop a heading instead when a
placeholder in the heading, such as `<Saved file>`, names something this
design does not have. Do not invent a name to fill it. In
`### <Case> rules`, a case is one kind of input with its own code,
such as a programming language. For example, drop
`### <Case> rules` when no folder's code differs per case, and drop
`### <Saved file> interface design` when the change adds or changes
no saved file. Drop a "one heading per" heading with no items the
same way. In the section that held a dropped heading, write one line
on what was dropped and why.

Design Edits' `E<n>` heading is the one heading you drop with no
note. A new design has no edits yet, so the page starts with its
opening rules and an empty Edits section. Later, the orchestrator
adds an entry each time a pull request changes the design.

Split the signatures across Types pages, one page per group of source
folders. Give each Types page its own row in the index table. An agent
then reads only the signatures its work touches.

An agent reads the index and only the parts its task needs. Write
each part so it stands alone. A part names each interface it uses and
links to the part that defines it. Step 5's review and the
`auto-run` skill's design check still read every part.

Filling every template does not make the doc complete. Before
writing, read
[what-every-doc-says.md](references/what-every-doc-says.md). It lists
what every design doc must say and which section holds each piece.
Keep the doc short, and give the riskiest pieces of the design the
most detail.

Write each paragraph and list item on one line, so the page wraps on
a phone. Put a `---` line under each `#` and `##` heading.

### Step 5: Review Based on Larson's "Manage Technical Quality" Principles

After drafting the doc, review it against these quality lenses. These catch problems that only emerge when reading the complete draft — gaps between sections, unstated assumptions, and tensions the author is too close to see.

[references/larson-technical-quality.md](references/larson-technical-quality.md)
sums up the Larson ideas these lenses use, with links to his writing.
Read it when a lens below is unclear.

**Skipped pages:**
- For each "Skipped:" row in the index table, check the page's
  condition in Step 4's table against the design. When a skipped
  page's condition holds, the skip is wrong. Writing that page is a
  required fix. Check each dropped heading's note the same way,
  against its template's `<...>` line or Step 4's rule for dropped
  headings.
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
- The after-release measure is the last item in the Steps and
  checkpoints section of the Building It and Tickets page. Is it
  precise enough to evaluate in 30/60/90 days? Could two engineers
  independently look at the same data and agree on whether the design
  succeeded? If not, sharpen it.

**Feedback loops:**
- Does the design include natural checkpoints where assumptions can be validated before committing further? Larson's core insight is that quality management is iterative — there's no winning, only learning. A design that ships as a single irreversible change has no room to learn.

**Narrative quality (final pass):**
- Read the main page, then each part in its index order. Does the doc
  as a whole tell a coherent story — from problem through reasoning to
  solution — or does it read like a filled-in template? If the
  latter, rewrite the sentences that link one section to the next
  until a human would actually want to read it.
- Are decisions explained with their history and reasoning, or just stated as conclusions? Every "we will do X" needs a "because Y" that the reader can evaluate.

After this review, revise the doc. Then start one fresh agent that did
not write the doc, and have it review the doc under the `review-plan`
skill. Fix each item in its Required Corrections list. Then present
the final version to the user. Flag any unresolved tensions, and any
open questions that pass the test in Step 2.

## Common Mistakes

- **Solutioning before understanding.** If you jump to Step 4 without Steps 1-3, the doc will be confidently wrong.
- **Boiling the ocean.** The design should solve the stated problem, not redesign the entire system. Scope creep in a doc becomes scope creep in implementation.
- **Misdiagnosing the problem type.** Treating a hot spot as a platform problem (or vice versa) produces a solution at the wrong weight. If the staircase classification doesn't match the scope of the solution, one of them is wrong.
- **Vague success criteria.** "Improve performance" is not a goal. "P99 latency under 200ms for the /search endpoint" is.
- **Ignoring existing patterns.** A novel approach in a codebase with
  established conventions needs explicit justification. Code tends to
  follow the way the teams that own it are divided. Do not fight that
  division without good reason.
- **Skipping alternatives.** If you only considered one approach, you didn't design — you guessed.
- **Writing a filled-in template.** If the doc reads as disconnected
  sections with no narrative thread, humans will skim it and miss the
  reasoning. The pages should connect. The Overview states the
  problem, and the Decisions settle it. Each part explains its piece
  of the design in the terms the Dictionary defines. Each page links
  to the pages it relies on.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above.
When the user corrects how you research, structure or write a design
doc, or how you ask them questions about it, follow
[skill-lessons](../skill-lessons/SKILL.md) with the skill name
`design-doc`, and offer once to save the correction as a lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
