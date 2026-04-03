---
description: Complete documentation workflow - diagnose documentation problems, fix at the right level, commit changes
---

## Getting Started

Before beginning, gather context about the codebase:

```bash
pwd
git log --oneline -5
find . -type d -not -path '*/\.*' -not -path '*/node_modules/*' -not -path '*/dist/*' -not -path '*/__pycache__/*' 2>/dev/null | head -30
```

If the user specified a scope (a file, directory, or module), focus the entire workflow on that area. If no scope was given, work across the full codebase.

Execute a documentation workflow that diagnoses the actual problem before writing anything. Documentation exists at multiple levels — inline comments, docstrings, module docs, READMEs — and the right fix depends on where comprehension breaks down.

## Core Principle: Documentation Tells the Story of Why

Every piece of documentation — from a one-line comment to a directory README — exists to answer *why*, not *what*. The code already says what it does. Documentation explains the reasoning, history, and intent that the code cannot express.

A docstring that says `def process_data(input): """Processes the data."""` is documentation debt, not documentation. A comment that says `# increment counter` above `counter += 1` is noise. Useful documentation tells the reader something they cannot derive from reading the code: why this approach was chosen, what invariants must hold, what will break if assumptions change, what the original designer was thinking.

When writing or reviewing any documentation at any level, apply this test: **does this tell the reader something they couldn't figure out by reading the code itself?** If not, it's either noise or it needs to be rewritten to convey the actual intent.

## Phase 1: Diagnose the Documentation Problem

Before writing anything, determine what kind of problem you're solving. Different problems demand different interventions.

**Run a structured scan of the codebase:**

```bash
# README coverage: which directories have them, which don't
find . -type d -not -path '*/\.*' -not -path '*/node_modules/*' -not -path '*/dist/*' -not -path '*/__pycache__/*' | while read dir; do
  if [ -f "$dir/README.md" ]; then echo "HAS_README: $dir"
  else echo "NO_README: $dir"; fi
done

# Docstring coverage: functions/classes without docstrings (Python example)
grep -rn "def \|class " --include="*.py" . | head -50

# Complex code without comments: files with high cyclomatic complexity or length
find . -name "*.py" -o -name "*.ts" -o -name "*.js" | xargs wc -l 2>/dev/null | sort -rn | head -20

# Recently changed files: where is active development happening
git log --name-only --pretty=format: -20 | sort | uniq -c | sort -rn | head -20
```

Adapt the scan commands to the actual languages in the codebase.

**Then classify the problem on Larson's staircase:**

**Hot spot** — Documentation is mostly fine, but a few specific areas are causing confusion. Symptoms: repeated questions about the same module, bugs from misunderstood interfaces, new team members struggling with the same files. The fix is targeted: go to the specific pain points and write the missing context. Don't touch anything else.

**Best practice adoption** — The codebase lacks a documentation convention entirely. No consistent docstring style, no README pattern, no standard for when comments are expected. The fix is establishing a convention and rolling it out incrementally — not all at once. Pick the highest-value area first, document it well, use that as the example for the rest.

**Leverage point investment** — The core interfaces, data models, or stateful systems are poorly documented. The code that everything else depends on is the code that's hardest to understand. The fix is deep, careful documentation of these critical areas — the kind where you exercise the docs against multiple real scenarios, not just describe the happy path.

**Drift** — Documentation exists but has fallen out of sync with the code. READMEs describe architectures that were refactored away. Docstrings promise behavior the function no longer delivers. The fix is targeted correction, not rewriting from scratch. Use `git log` to find where code changed but docs didn't.

If the user specified a scope, focus the diagnosis on that area. Present your diagnosis to the user before proceeding. Name the problem type and explain why — don't just jump to writing.

## Phase 2: Prioritize by Leverage

Not all documentation is equally valuable. Prioritize based on where comprehension failures are most expensive.

**Tier 1 — Leverage points (fix these first):**

*Interfaces between systems or components.* These are the contracts that other code depends on. A poorly documented interface gets misused, and the resulting bugs are expensive because they cross boundaries. For each public interface, check: does the documentation explain what the contract promises, what the caller is responsible for, and what happens on failure? Does it explain *why* the interface is shaped this way — what design decisions constrained it?

*Data models and schemas.* The data model constrains what the system can represent. Documentation here should explain not just the fields but the invariants: what states are valid, what transitions are allowed, why the model is structured this way and not some other way. A reader should understand the design intent well enough to know whether a proposed change would violate the model's assumptions.

*Stateful systems and state transitions.* State is the hardest part of any system to understand from code alone. What states can the system be in? What transitions are valid? What happens on failure — does it retry, roll back, or leave things in an intermediate state? These are the places where missing documentation directly causes incidents.

**Tier 2 — High-churn code:**

Files that change frequently (identified in the Phase 1 scan) are files where developers are actively working. Documentation here has the highest read rate and the highest drift risk. Check these for accuracy, not just existence.

**Tier 3 — Complex internals:**

Long functions, deeply nested logic, non-obvious algorithms. These need inline comments that explain *why* — not what the code does, but why it does it this way. What's the algorithm? What edge case does this branch handle? Why is this ordering important?

**Tier 4 — Directory orientation:**

READMEs that help a developer navigate the codebase structure. These are useful but they're the *least* valuable documentation layer — a developer who finds the right file but can't understand it is worse off than a developer who takes an extra minute to find the file but understands it immediately.

Present the prioritized list to the user. Be explicit about what you're fixing and what you're deliberately skipping.

## Phase 3: Review Documentation Quality

Use the **docs-reviewer** agent to analyze the prioritized areas. The agent should evaluate at every documentation level, not just READMEs:

**For docstrings (functions, methods, classes, modules):**
- Does the docstring explain *why* the function exists, not just restate its name?
- For public interfaces: does it specify the contract — parameters, return values, error conditions, side effects?
- For complex logic: does it explain the algorithm or approach and why it was chosen?
- Is the docstring accurate to the current implementation? Check for drift against the actual code.
- Module-level docstrings: do they explain the design intent of the module — why these things are grouped together, what mental model a reader should hold?

**For inline comments:**
- Are complex code blocks explained? A function over 30 lines with no comments is suspect.
- Do comments explain *why*, not *what*? Comments that restate the code are noise — flag them for removal or rewriting.
- Are there "TODO" or "HACK" comments that indicate known debt? Note these but don't resolve them — that's a code change, not a documentation change.
- Are non-obvious decisions explained? Magic numbers, unusual orderings, defensive checks, workarounds — these all need a comment explaining the reasoning.

**For READMEs:**
- Does the README tell the story of the directory — why it exists, how it fits into the system, what design decisions shaped it?
- Is it accurate to the current code? Check for references to files, functions, or patterns that no longer exist.
- Does it help a new developer orient themselves, or does it just list files?

**For type annotations and API contracts:**
- Are public interfaces typed? Missing types on a public function are a documentation gap.
- Do type annotations match actual behavior?

Provide the agent with the prioritized list from Phase 2, recent git changes for drift detection, and the user's scope if one was specified. Wait for the review to complete before proceeding.

## Phase 4: Write Documentation at the Right Level

Use the **docs-writer** agent to implement the review findings. The agent should write documentation at whatever level the problem exists — not default to READMEs for everything.

**Docstring guidelines:**

Module docstrings should read as a brief narrative: why does this module exist, what's the mental model, how does it relate to the rest of the system. One to three paragraphs. A developer reading this should understand the module's role well enough to predict what they'll find inside.

Class docstrings should explain the design intent — what abstraction does this represent, what invariants does it maintain, why is it structured this way. Not a list of methods (the code shows that) but the conceptual model.

Function docstrings scale with visibility and complexity. Public interface functions need full contract documentation: what the parameters mean (not just their types — their semantics), what the return value represents, what exceptions can occur and when, any side effects. Internal utility functions need less — but if the logic is non-obvious, explain the approach and why it was chosen.

**Inline comment guidelines:**

Comment *why*, never *what*. If you find yourself writing a comment that restates the code, delete it. If you find complex code with no comments, don't describe the mechanics — explain the reasoning. Why this algorithm? Why this ordering? What invariant does this maintain? What breaks if this assumption changes?

Flag magic numbers with their origin. `timeout = 30` needs a comment: where did 30 come from? Is it empirically derived, specified in a standard, or arbitrary?

**README guidelines:**

A directory README is orientation, not exhaustive documentation. It tells a developer: here's what this directory is for, here's the key design decision that shaped it, here's where to look next. Write it as a brief narrative, not a file listing. If the directory contains a public interface, the README should point to it and summarize the contract. If the directory has a non-obvious structure, explain why it's organized this way.

Don't create READMEs for every directory. Leaf directories with a few utility files don't need them. Create READMEs where a developer would plausibly arrive and wonder "what is this and why is it here?"

**Across all levels, apply the narrative test:** does this documentation tell the reader something they couldn't derive from the code? Does it explain the history and reasoning, not just the current state? Would a developer new to this codebase, reading this documentation, understand not just what the code does but why it was built this way?

Pass the complete findings from Phase 3 to the agent. Wait for implementation to complete before proceeding.

## Phase 5: Verify and Commit

**Verify before committing.** This is the feedback loop the documentation workflow needs.

```bash
# Review all changes
git diff --stat
git diff
```

For each change, check:
- Does the new documentation pass the "why not what" test?
- For docstrings: does the docstring match the actual function behavior? Read the function and its docstring together.
- For READMEs: does the README accurately describe the current state of the directory?
- For comments: does each comment add information the code doesn't already convey?
- Did any documentation get *removed* that shouldn't have been? Drift corrections should update, not delete, unless the documented feature no longer exists.

If anything fails verification, fix it before committing. Don't commit documentation that's wrong — inaccurate docs are worse than missing docs.

**Commit with a clear message:**

```bash
git add -p  # Stage selectively — review each hunk
git commit -m "docs: <brief description>

<one line per major change: what was documented and why>"
```

Use `git add -p` not `git add .` — review each change as you stage it. Documentation changes should be intentional, not bulk.

**Present a summary to the user:**
- What problem type was diagnosed (hot spot, best practice, leverage point, drift)
- What was prioritized and why
- What was written at each level (docstrings, comments, READMEs)
- What was deliberately skipped and why
- Any remaining documentation debt identified but not addressed

## Guidelines

- **Diagnose before writing.** Phase 1 determines everything that follows. Skipping it produces documentation at the wrong level, in the wrong places, solving the wrong problem.
- **Leverage points first.** Interfaces, data models, and stateful systems get documented before utility functions and directory READMEs. This is where comprehension failures are most expensive.
- **Documentation lives closest to the code it describes.** A function's documentation is its docstring. A module's documentation is its module docstring. A directory's documentation is its README. Don't put function-level docs in a README or module-level docs in a top-level wiki.
- **Execute phases sequentially.** Each phase depends on the output of the previous one. Don't write documentation before the review is complete. Don't commit before verification passes.
- **If the user specified a scope, focus the entire workflow on that scope.** But still diagnose the problem type within that scope — a focused area can still have different documentation problems.
- **Never add component documentation to CLAUDE.md.** CLAUDE.md is for agent instructions, not human documentation.
