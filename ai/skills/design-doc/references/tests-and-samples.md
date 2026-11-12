---
id: "DESIGN-<Title> - Tests and Samples"
aliases: []
tags:
  - "#DesignDoc"
date_created: <YYYY-MM-DD>
project: <Project>
topics:
  - "[[<topic>]]"
---

# <Title> - Tests and Samples
---

Part of [[DESIGN-<Title>]]. Read its index first.

## Tests and Samples
---

<One or two sentences: where a module's own tests go, and what the
folder of outside tests holds.>

### `<test file>`

<One heading per test file, in build order. For a module's own tests,
write `` `<path>`, `mod tests` ``. For an outside test file, write its
path, and add a comma and a topic when its tests split, such as
`` `tests/check_comments.rs`, style rules ``. Under it, one bullet per
test name with one or two sentences on why it matters, grouped under
plain labels such as the step it covers. With no test file, keep the
heading once with a one-line note on why.>

### Left untested

<What has no test and why, such as needing a killed process, and when
to add it. These are the design's known gaps.>

### Sample files and first expected results

<Short sample files, one per case, then the exact results the first
code must return for them, and which results the owner checks by
hand. These are the first measurable results.>
