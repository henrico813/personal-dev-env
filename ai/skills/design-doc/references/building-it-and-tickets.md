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

<A numbered list of build steps in order. Each names what to build and
in which files, then a measurable check that proves it before the
next step, such as matching today's tool on real data or timing an
install. The last item says how, after release, to tell the problem
is gone, with a measure two people would agree on, such as the
number of failures in the first month of real use.>

## Implementation Tickets
---

<One paragraph: how many tickets, how they group into pull requests,
who builds and who reviews, and whether the tickets are real issues
yet. Then: `Each PR's "Reads" line names the parts of this design its
PR touches. Its agents read those parts after the index and this
part, with each Design Edits entry those parts point to.`>

### Phase <n>: <name>

<One heading per phase, in build order. For each PR, a Ticket and Does
table, or a PR and Tickets table for several PRs. Then what the PR
needs from earlier PRs, the types or error codes it adds, and a
"Reads:" line of part links separated by semicolons. With no phases
yet, keep the heading once with a one-line note on why.>

### Build order

<One order that meets every dependency above, then bullets of which
PRs can be built side by side once their dependencies land.>

### Where the user reviews closely

<Bullets: each risky PR the owner reads closely, what could go wrong
there, and the promise it keeps. Then how the other PRs are checked,
such as trying a screen on the phone.>

### Defaults the tickets start from

<Bullets: a choice each ticket starts from unless its plan finds a
better one, such as which library to use.>

### Where the code lives

<Bullets: the new code's folder, its installed name, how it runs
beside today's tool until the switch-over, and how to switch back if
the change fails.>

### Who builds and who reviews

<Bullets: who builds the PRs, which ones the owner reads closely, how
review effort matches risk, and when the owner reviews the comments.>

### Still to do before these become real tickets

<Bullets of checks or decisions still owed before the tickets are
filed.>
