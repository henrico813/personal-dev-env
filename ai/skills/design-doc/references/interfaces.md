---
id: "DESIGN-<Title> - Interfaces"
aliases: []
tags:
  - "#DesignDoc"
date_created: <YYYY-MM-DD>
project: <Project>
topics:
  - "[[<topic>]]"
---

# <Title> - Interfaces
---

Part of [[DESIGN-<Title>]]. Read its index first.

## Interfaces
---

<One bullet per place the program meets something outside itself:
- Live calls, such as MCP tools and CLI commands: what they carry and
  every real caller.
- Saved files: what each holds and every step or version that reads
  it.
- The UI: what the user sees and runs, and every other tool that
  reads its state.
Then one line naming the outside tools that are not interfaces here,
and the file that hides each one.>

### MCP/CLI interface design

<A table of each MCP tool and what it does, with its step, then how
the CLI takes the same inputs. Then the reply's Field and Holds table,
one example reply, the rules every call follows, a table of every
error code with who sees it and what to do, and a table of the other
commands. Keep out of each reply what its callers do not need.>

### <Saved file> interface design

<One heading per saved file, such as Review or Approvals. Say where it
lives and when it goes away. Then a Field and Holds table, one JSON
example, a table for each set of values such as statuses, and its
rules, with an example for each rule that is not obvious. Show how
the fields rule out bad states, and what happens to a field from a
newer version. With no saved file, keep the heading once with a
one-line note on why.>
