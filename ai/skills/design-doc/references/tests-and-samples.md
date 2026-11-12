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

<One heading per test file, in build order. For tests inside a source
file, write the file's path and the name of its test block, such as
`` `src/check.rs`, `mod tests` ``. For an outside test file, write its
path. When the heading covers only one group of the file's tests, add
a comma and that group's topic, such as
`` `tests/check_comments.rs`, style rules ``. Under it, one bullet per
test name with one or two sentences on why it matters, grouped under
plain labels such as the step it covers. With no test file, drop
this heading and say why in one line under `## Tests and Samples`.>

### Left untested

<What has no test and why, such as needing a killed process, and when
to add it. These are the design's known gaps.>

### Sample files and first expected results

<Short sample files, one per case, then the exact results the first
code must return for them, and which results the user checks by
hand. These are the first measurable results.>
