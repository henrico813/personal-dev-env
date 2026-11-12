---
id: "DESIGN-<Title> - Building It and Tickets"
aliases: []
tags:
  - "#DesignDoc"
date_created: <YYYY-MM-DD>
project: <Project>
topics:
  - "[[<topic>]]"
---

# <Title> - Building It and Tickets
---

Part of [[DESIGN-<Title>]]. Read its index first.

## Building It
---

### Steps and checkpoints

<A numbered list of build steps in order. Each step names what to
build and in which files. Then it gives a measurable check that
proves the step works before the next one starts, such as matching
today's tool on real data or timing an install. The last item says
how to tell, after release, that the problem is gone. Use a measure
two people would agree on, such as the number of failures in the
first month of real use.>

## Implementation Tickets
---

<One paragraph: how many tickets, how they group into pull requests,
who builds and who reviews, and whether the tickets are real issues
yet. Give each PR a short name, such as P1 and P2. Number the PRs in
the order you first planned them. Use these names on every page.
Then copy these two sentences as plain text, without the backticks:
`Each PR's "Reads" line names the parts of this design its
PR touches. Its agents read those parts after the index and this
part, with each Design Edits entry those parts point to.`>

### Phase <n>: <name>

<One heading per phase, in build order. A phase is a group of PRs
that build one piece of the design, such as the foundation. Give each
PR a Ticket and Does table, or give several PRs one shared table with
the columns PR and Tickets. Then, for each PR, say what it needs from
earlier PRs and which types or error codes it adds. Give each PR a
"Reads:" line of part links separated by semicolons. With no phases
yet, drop this heading and say why in one line under
`## Implementation Tickets`.>

### Build order

<One order that meets every dependency above, then bullets of which
PRs can be built side by side once their dependencies merge.>

### Where the user reviews closely

<Bullets: each risky PR the user reads closely, what could go wrong
there, and what it must not break. Then how the other PRs are
checked, such as trying a screen on the phone.>

### Defaults the tickets start from

<Bullets: a choice each ticket starts from unless its plan finds a
better one, such as which library to use.>

### Where the code lives

<Bullets: the new code's folder, its installed name, how it runs
beside today's tool until the switch-over, and how to switch back if
the change fails.>

### Who builds and who reviews

<Bullets: who builds the PRs and how review effort matches risk. The
section "Where the user reviews closely" lists the PRs the user reads
closely. When the user reviews code comments as a separate step, say
when.>

### Still to do before these become real tickets

<Bullets of checks or decisions still owed before the tickets are
filed.>
