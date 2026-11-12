---
id: "DESIGN-<Title> - How Each Step Works"
aliases: []
tags:
  - "#DesignDoc"
date_created: <YYYY-MM-DD>
project: <Project>
topics:
  - "[[<topic>]]"
---

# <Title> - How Each Step Works
---

Part of [[DESIGN-<Title>]]. Read its index first.

## How Each Step Works
---

<One or two sentences: what every step returns, such as a `Result` so
a refusal is a value and not a crash, and a link to the Types pages.>

### Call Stacks

#### Step <n>: `<function>`

<One heading per step the program or the user runs, in process order.
For a step no function runs, name what the user does instead. Under
it: what the step does in one sentence, "In: ... Out: ...", a code
block of pseudocode naming each module call, then bullets for each
rule, edge case and refusal, each with its reason. Say each state
change the step makes, what a crash or failed write leaves, and what
stays consistent.>

#### Any time: `<function>`

<One heading per call with no step number: its inputs, its output, and
when an agent calls it. With no such call, keep the heading once with
a one-line note on why.>
