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
Rules. Say what input the program checks and which rules it checks
that input against. Give one failing example. Then say which rules
today's tool runs, which file holds the rules, and why that file has
its name.>

### The rules

<How each rule is named in replies and what it quotes. A Rule, "Breaks
when" and <Cases> table, one row per rule. The <Cases> column lists
the cases that use the rule, such as Go, Python, Rust. Then what a
finding's detail holds. When the code keeps the rules in one type,
such as an enum, name each value of that type that is not a rule,
such as a failure another step reports.>

### Which <cases> use which rules

<Name it after the cases, such as languages. Say where each case lists
its rules, which rules each case uses, and why a case leaves some
out.>

### Failing or only reporting

<When a finding fails and when it is only reported. A reported
finding carries a note that says why it does not fail, such as
`approved` for text the user approved. Then a numbered list of these
notes, in the order the check tests them, each with its reason.>

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
