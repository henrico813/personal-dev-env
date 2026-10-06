---
name: code-search
description: Finds code references and definitions with few, focused searches. Use when searching a codebase for where a name is defined or used, or before deleting or renaming a file.
---

# Code Search

Use focused, iterative searches instead of reading a large result set.

1. List matching files first with `rg -l --sort path`, then read only the needed
   line ranges from those files. If `rg` is not installed, use `git grep -l`.
2. When a match is a constant or function definition, search for that name too.
   Files often use a constant such as `errNotFound` instead of the text it holds.
3. Search name variants when names cross languages or tools, such as
   `create-plan` and `create_plan`.
4. For work tied to a commit, search that commit with `git grep <pattern> <commit>`
   instead of searching the working tree.
5. Before deleting or renaming a file, search both its full path and file name.
6. If a search returns more than about 50 files, narrow the query before reading
   the results.

Example: to find uses of an error message, `rg -l --sort path 'not found'` finds
its constant definition, then `rg -l --sort path errNotFound` finds the files
that use it; read only the matching ranges.
