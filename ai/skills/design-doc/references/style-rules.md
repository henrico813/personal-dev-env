---
id: "DESIGN-<Title> - <Kind> Rules"
aliases: []
tags:
  - "#DesignDoc"
date_created: <YYYY-MM-DD>
project: <Project>
topics:
  - "[[<topic>]]"
---

# <Title> - <Kind> Rules
---

Part of [[DESIGN-<Title>]]. Read its index first.

## <Kind> Rules
---

<Name the page after the rules the program checks, such as Style
Rules. Say what is checked against what, with one failing example,
which rules today's tool runs, which file holds the rules, and why
that file has its name.>

### The rules

<How each rule is named in replies and what it quotes. A Rule, "Breaks
when" and <Cases> table, one row per rule. Then what a finding's
detail holds, and any values in the rule type that are not rules.>

### Which <cases> use which rules

<Name it after the cases, such as languages. Say where each case lists
its rules, which rules each case uses, and why a case leaves some
out.>

### Failing or only reporting

<When a finding fails and when it is only reported. Then a numbered
list of report notes in the order they apply, each with its reason.>

### What the check reads

<Which lines, which copy of a file and which older version the rules
read, with one example. Then a bullet for each special case.>

### How the pieces fit

<A code block of pseudocode for each step that runs the rules. Then
what each type holds and why the functions have their names.>

### Choices that lost

<Weigh at least two options for each non-trivial choice. Bullets:
each rejected option, what it would have looked like, what it gets
right, and why it lost, in terms of trade-offs.>
