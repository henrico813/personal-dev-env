---
id: "DESIGN-<Title> - Design Edits"
aliases: []
tags:
  - "#DesignDoc"
date_created: <YYYY-MM-DD>
project: <Project>
topics:
  - "[[<topic>]]"
---

# Design Edits
---

Part of [[DESIGN-<Title>]]. Read its index first.

Agents build the <program> as <count> pull requests, and building them changed parts of the design. This note lists those changes.

<Keep the line above and fill in `<program>` and `<count>`, even in
a new doc where nothing has changed yet.>

- Each entry replaces the design text it names. The rest of the design stays as approved.
- Each replaced passage has a pointer to its entry, so a reader of that passage sees it changed.
- An entry from an open pull request describes the code as planned. It can change in that pull request's fix rounds.
- The orchestrator adds each merged pull request's design edits here.

An entry may name more than one passage when one change touches several. When a later merged pull request changed the same item, the entry gives the code as it is now and names both pull requests. "<PR> adds it" marks a part of the design that a later pull request still builds. Entry IDs never change, so an entry added later sits beside its pull request's entries with a higher number, such as E82 after E9.

<Keep the paragraph above. Fill in `<PR>` with a pull request's
short name, such as P12.>

## Edits
---

<Empty until the first pull request changes the design.>

### E<n> <Short title>

<One heading per edit, numbered from E1. Bullets: PR (its number,
merged or open, and its plan), Replaces (the part, the section and
the exact text), New text, and Source (the plan decision or code it
comes from). Beside each replaced passage, add "(changed: see
[[DESIGN-<Title> - Design Edits#E<n> <Short title>|E<n>]])", or a
"Changed: see ..." line under the heading of a block that changed.>
