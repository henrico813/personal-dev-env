---
name: design-doc
description: Use when the user asks to create a technical design document for a feature, system, or architectural change.
---

# Design Doc

## Overview

A design doc is a deliberate pause before implementation. It forces you to identify leverage points, surface hidden complexity, and align technical vectors before writing code. The cheapest bug to fix is the one you designed out of existence.

**Core principle:** A design doc answers "what should we build and why?" with enough rigor that reviewers can find flaws before they become code.

## Writing for Humans

The output of this process is a document for human readers. Humans do not process information taxonomically — they understand through narrative. Every section of the design doc should read as a story, not a classification scheme.

**What this means in practice:**

Tell the reader *how we got here*. A problem statement isn't a list of symptoms — it's the story of what users experience today, why the system works this way, and what changed to make the current approach insufficient. The reader should feel the problem before you propose fixing it.

Show the reasoning, not just the conclusion. The Solution section must walk the reader through the decision-making process: what options existed, what tradeoffs each carried, why this one won. A reader who only sees the chosen approach can't evaluate whether it's the right one. A reader who sees the journey can spot where the logic breaks.

Connect past, present, and future. Every design exists in a history. How did the system arrive at its current state? How does this change fit into where the system is going? A design doc without historical context is a design doc that will be misunderstood by the next engineer who reads it in six months.

Write prose, not bullet points. Bullet lists are for scanning; design docs are for understanding. When the doc needs to convey *why* — and it almost always does — use paragraphs. Reserve lists only for genuinely enumerable things (API fields, migration steps, config values).

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
- **Map the blast radius.** Which teams, services, and systems are affected? What's the rollback story?
- **Surface competing timeframes.** What does "done by Friday" look like vs "done right for the next two years"? Name the tradeoff explicitly.
- **Catalog existing patterns.** What conventions does the codebase already follow? A design doc that fights the codebase's grain needs strong justification.
- **List open questions.** What do you still not know? What assumptions are you making?

### Step 3: Spawn Parallel Agents for Comprehensive Research

For non-trivial designs, dispatch subagents to research in parallel:

**Agent 1 — Codebase Audit:**
Search for all files, modules, and interfaces affected by this change. Produce a dependency map and identify hot files (changed in >50% of recent PRs).

**Agent 2 — Prior Art & Patterns:**
Search for existing patterns in the codebase that solve similar problems. Look for internal libraries, shared utilities, or conventions that should be reused rather than reinvented.

**Agent 3 — Risk & Edge Cases:**
Identify failure modes, edge cases, data migration concerns, backwards compatibility issues, and performance implications. Focus on stateful systems and data model changes — these are hardest to fix later.

Synthesize all agent outputs before proceeding. If agents surface contradictory information, resolve it before writing the doc.

### Step 4: Create a Detailed Plan

Write the design doc with this structure. Every section is required unless explicitly marked optional.

---

# [Feature/System Name]: Design Doc

## Overview

One paragraph that tells the story: what exists today, what's changing, and why. A busy engineer should understand the scope and motivation after reading this alone. This is the doc's thesis statement — write it last, after everything else is solid.

## Problem Statement

Tell the story of the problem. Start with the user's or system's experience today — what actually happens, concretely. Then explain how we got here: what design decisions or changing conditions created the current situation. Finally, describe the impact — not as a bullet list of affected parties, but as a narrative that makes the reader feel why this matters now and what happens if we do nothing.

Ground the narrative in concrete examples or data. A reader should finish this section understanding the problem well enough to propose their own solution.

### Goals

State what success looks like. Be specific and measurable:
- **In scope** — exactly what this design covers
- **Success criteria** — how we know this worked (metrics, behaviors, thresholds)

## Solution

This is the heart of the doc. Write it as a narrative that walks the reader through your reasoning.

**Start by situating the decision on Larson's staircase.** Before proposing anything, name the type of problem you're solving. Is this a hot spot — a localized pain point we can fix directly? A best practice the team hasn't adopted yet? An investment in a leverage point (interface, stateful system, or data model) that will preserve quality as the system evolves? Or a vector alignment effort that changes how the organization builds software? The answer determines the appropriate weight of the solution. A hot spot fix that arrives as a platform redesign is over-engineered. A leverage point investment that arrives as a quick patch is under-designed.

**Then present the landscape of alternatives.** Before the chosen approach, describe the options you evaluated. For each, explain what it would look like, what it gets right, and where it falls short. This isn't a formality — it's how the reader builds the mental model needed to evaluate your recommendation. Minimum two alternatives for any non-trivial decision. Explain why each was rejected in terms of the tradeoffs, not just "we preferred X."

**Present the chosen approach as a design narrative.** Walk the reader through the decision: what it is, why it wins given the tradeoffs, and how it fits into the system's existing architecture and direction. The reader should understand not just *what* we're building but *why this and not that*.

**Focus the technical detail on leverage points.** Larson identifies three places where extra investment preserves quality over time: interfaces, stateful systems, and data models. These are where the design doc earns its keep. For each that this change touches:

*Interfaces:* What contracts are changing between systems or components? Are the new interfaces durable — do they expose the essential complexity while hiding the accidental? Have you tested the interface mentally against multiple real clients, not just the one that motivated this change?

*Data models:* How does the data model change? Is the new model rigid (prevents invalid states) and tolerant of evolution (won't require another migration in six months)? Represent multiple real scenarios against the proposed model to verify it holds.

*Stateful systems:* What state transitions change? State is the hardest thing to fix later and accumulates complexity faster than anything else. Describe the failure modes and consistency guarantees explicitly.

For each of these, include code sketches or schema definitions where they clarify the design — not as full implementations, but as contracts that a reviewer can evaluate.

**Explain how this fits into the system's trajectory.** A design doesn't exist in isolation. Describe where the system has been (the history that created today's state), where it's going (the technical direction the team is aligned on), and how this change moves along that vector. If this change diverges from the current direction, the justification must be explicit and strong. A new team member reading this doc in six months should understand the reasoning without any oral tradition.

---

### Step 5: Review Based on Larson's "Manage Technical Quality" Principles

After drafting the doc, review it against these quality lenses. These catch problems that only emerge when reading the complete draft — gaps between sections, unstated assumptions, and tensions the author is too close to see.

**Weight calibration:**
- Re-read the staircase classification from the Solution section. Does the *actual scope* of the design match the stated problem type? A hot spot diagnosis that produced a platform-scale solution means either the diagnosis was wrong or the solution drifted. Reconcile before publishing.
- Could a simpler intervention — a hot spot fix, a process change, a best practice adoption — deliver most of the value? If yes and the doc doesn't explain why the heavier approach is necessary, that's a gap.

**Timeframe honesty:**
- Does the doc acknowledge tension between short-term shipping pressure and long-term quality investment? Is the tradeoff named explicitly, or is it hidden behind optimistic language? Larson: "you'll do very different work getting that critical partnership out the door for next week's deadline versus building a platform that supports launching ten times faster next quarter."

**Measurability:**
- Are the success criteria precise enough to evaluate in 30/60/90 days? Could two engineers independently look at the same data and agree on whether the design succeeded? If not, sharpen them.

**Feedback loops:**
- Does the design include natural checkpoints where assumptions can be validated before committing further? Larson's core insight is that quality management is iterative — there's no winning, only learning. A design that ships as a single irreversible change has no room to learn.

**Narrative quality (final pass):**
- Read the doc start to finish. Does it tell a coherent story — from problem through reasoning to solution — or does it read like a filled-in template? If the latter, rewrite the connective tissue until a human would actually want to read it.
- Are decisions explained with their history and reasoning, or just stated as conclusions? Every "we will do X" needs a "because Y" that the reader can evaluate.

After this review, revise the doc and present the final version to the user. Flag any unresolved tensions or open questions that need human judgment.

## Common Mistakes

- **Solutioning before understanding.** If you jump to Step 4 without Steps 1-3, the doc will be confidently wrong.
- **Boiling the ocean.** The design should solve the stated problem, not redesign the entire system. Scope creep in a doc becomes scope creep in implementation.
- **Misdiagnosing the problem type.** Treating a hot spot as a platform problem (or vice versa) produces a solution at the wrong weight. If the staircase classification doesn't match the scope of the solution, one of them is wrong.
- **Vague success criteria.** "Improve performance" is not a goal. "P99 latency under 200ms for the /search endpoint" is.
- **Ignoring existing patterns.** A novel approach in a codebase with established conventions needs explicit justification. Don't fight Conway's Law without good reason.
- **Skipping alternatives.** If you only considered one approach, you didn't design — you guessed.
- **Writing a filled-in template.** If the doc reads as disconnected sections with no narrative thread, humans will skim it and miss the reasoning. The sections should flow into each other — the problem motivates the goals, the goals constrain the solution, the solution addresses the risks.
