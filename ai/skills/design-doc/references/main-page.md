---
id: "DESIGN-<Title>"
aliases: []
tags:
  - "#DesignDoc"
date_created: <YYYY-MM-DD>
project: <Project>
topics:
  - "[[<topic>]]"
---

# <Title>
---

Every agent reads this index and each part its work touches. Each PR's Reads line names the parts for that PR. A pointer beside replaced text leads to its entry in [[DESIGN-<Title> - Design Edits|Design Edits]]; read each entry the parts you read point to. The orchestrator reads all of Design Edits.

| Part | Read this part when |
| --- | --- |
| [[DESIGN-<Title> - Design Edits\|Design Edits]] | You follow a pointer from a part; the orchestrator reads it all |
| [[DESIGN-<Title> - UI\|UI]] | You work on the <main> screen or its keys |
| [[DESIGN-<Title> - Skills\|Skills]] | You need what each skill runs, and when |
| [[DESIGN-<Title> - Interfaces\|Interfaces]] | You work on commands, agent replies or saved files |
| [[DESIGN-<Title> - <Kind> Rules\|<Kind> Rules]] | You work on the <rules> <step> checks, or which <cases> use each |
| [[DESIGN-<Title> - How Each Step Works\|How Each Step Works]] | You work on a step's inputs, outputs or call stack |
| [[DESIGN-<Title> - Types for <Module group>\|Types for <Module group>]] | You need <language> signatures in <the files and folders this page covers> |
| [[DESIGN-<Title> - Tests and Samples\|Tests and Samples]] | You add or change tests, or need the sample <items> |
| [[DESIGN-<Title> - Building It and Tickets\|Building It and Tickets]] | You need the build order, checkpoints, tickets, or a PR's Reads line |

A reviewer checks each Skipped row against the page table in Step 4 of the design-doc skill, and reports a wrong skip as a required fix.

<Keep every row, in this order, and add one Types row per Types page.
For a page Step 4 of the skill skips, remove the link and write
"Skipped:" and one line on why its condition does not hold in place
of "Read this part when". Keep the reviewer line above.>

In this note: [[#Overview]], [[#Dictionary]], [[#<Name> Process]], [[#Decisions]], [[#Project File Structure]], [[#Open questions]], [[#Notes for Agents]].

## Overview
---

<Open with the problem, in one or more paragraphs: who has it, what
happens to them today with one concrete example, how the system came
to work this way, and what it costs if nothing changes. Then one
paragraph on the design: what changes, the promises it keeps, where
its saved state lives, which skills change, and what the first
version leaves out. A busy reader gets the scope and the reason from
this alone. Write it last.>

## Dictionary
---

<A numbered list. Each item defines one **term** in the owner's words.
Use each term the same way on every page. Define each term that the
lines outside `<...>` use on any page, such as part, Reads line and
orchestrator. Then one paragraph for the plain words the rules use,
such as where a branch starts.>

### UI terms

<A Term and Is table naming each window, screen, prompt and notice
the user sees.>

## <Name> Process
---

<Name it after the main process, such as Review Process. This is the
chosen approach at a glance. A Step and "Who does what" table, one
row per step, saying whether the agent, the program or the user acts,
with the function each program step runs. Then which file and command
serve each step. Last, say whether any step can be skipped, and why.>

## Decisions
---

<One bullet per decision, one or two sentences. The first bullet
names the problem type (hot spot, best practice, leverage point or
vector alignment), says in plain words what that type means, and says
why the design's weight fits it. Each bullet
gives its reason and the cost it accepts. Weigh at least two options
for each non-trivial choice. Under its bullet, give each option that
lost a nested bullet or short paragraph: what it would look like,
what it gets right, and where it falls short. A choice against the
codebase's patterns or direction says why. Add "See
[[part#section]]" when a part explains it. Include what the design
leaves out and the names it picks.>

## Project File Structure
---

<A text block of the source and test tree, each file or folder with
its job in a few words. Name files after the steps and Dictionary
terms, in plain words a new maintainer can follow without the earlier
discussion. Then "Each step's file follows the [[#<Name> Process]]
table." and bullets on how the files work together: the one file that
lays out the whole process, which file calls which as the user's main
action passes through them, which existing code it reuses or
replaces, and what is left to the implementation.>

### <Case> rules

<Name it after the folder whose code differs per case, such as
Language rules. Numbered rules: what only that folder decides and the
domain rules its code follows. End with a bullet on what adding a
case takes. When no folder's code differs per case, drop this heading
and say why in one line under Project File Structure.>

## Open questions
---

<Numbered. Only questions whose answer would change an interface,
saved data or a trade-off. Each says when it must be decided, such as
before a named ticket, and gives the recommended answer.>

## Notes for Agents
---

<Bullets for agents only: source designs and research, backups of
earlier versions, related open pull requests, approaches tried and
rejected with why agents must not bring them back, and source facts
left out. The history a human reader needs goes in the Overview.>
