---
name: go-development
description: >-
  Writes, refactors, tests, and reviews Go code using the Uber Go Style Guide,
  small interfaces, struct composition, and readable behavior tests. Use when
  implementing Go features, changing .go files, designing Go APIs or dependencies,
  writing Go tests, or reviewing Go changes. Also use when explaining Go
  composition with examples. Not for unrelated work in a repository that happens
  to contain Go.
---

# Go development

Write Go code that is straightforward to use, read, test, and change.

## Workflow

1. Read the request, repository instructions, relevant code and tests, `go.mod`,
   and existing check commands. Identify the behavior that must change and the
   behavior that must stay the same. Do not edit code for a review-only request.
2. Scan the contents of [Uber Go guidelines](references/uber-go-guide.md).
   Read every section relevant to the change before implementing or reviewing it.
   Apply the whole guide where relevant, not just its testing or composition
   sections. For a full-package review, read the complete reference.
3. For struct, interface, or dependency design, read
   [composition and reuse](references/composition.md). Identify what behavior is
   shared and what varies before choosing an abstraction.
4. For test changes, also load the `behavior-focused-testing` skill and read
   Uber's Test Tables section. Keep the behavior and reason for the test visible.
5. Check [Go compatibility](README.md#go-compatibility) before applying the
   guide's atomic, loop-capture, goroutine, or tooling examples.
6. Make the requested change. Format changed Go files and run the repository's
   relevant tests and checks. Review the diff for unrelated edits and accidental
   API changes. Report checks that failed or could not run.

## Compose behavior deliberately

- Define interfaces around capabilities a caller actually needs. A type satisfies
  an interface by having its methods; no declaration of inheritance is needed.
- Keep state and implementation in concrete types. Pass dependencies into the
  code that uses them instead of replacing package globals.
- Use named fields for dependencies and explicit forwarding methods for the
  operations the outer type should expose. Prefer unexported dependency fields
  when callers do not need direct access.
- Use embedding only when its promoted methods and exposed field deliberately
  belong in the outer type's API. Do not embed merely to avoid forwarding methods.
  Follow Uber's restrictions on public structs, zero values, and mutexes.
- Use a function when a function is enough. Do not create an interface for every
  struct, a shared base type for unrelated behavior, or callbacks to an owner
  merely to imitate an example.
- Explain a design through its actual values and calls. Use the original speaking
  example when helpful; do not spend time deciding whether to call it a mixin.

## Apply the guide without unnecessary changes

The Uber reference covers every source section, with repeated examples and
explanations condensed. It is not a verbatim copy. Use its linked original for
additional examples or ambiguous wording.

Follow explicit user requirements and repository policy when they differ from
these defaults. Preserve package-wide consistency. Do not mix in a new style
piecemeal or expand a small fix into a style migration. Explain a material
conflict instead of silently ignoring it.

Preserve the guide's qualifications: "usually" is not "always," a soft limit is
not a hard limit, and a performance suggestion is not a reason to optimize every
path. Language correctness and the project's supported Go version take priority
over an old example.

Do not add dependencies or change the toolchain merely because a reference uses
them. Use the project's checks first. Where appropriate and available, run
`gofmt` or `goimports`, `go test`, `go vet`, and the configured lint runner. Use the
race detector for relevant concurrency tests when the environment supports it.
Do not claim that a command ran unless it did.

## Result

For implementation work, return the change and a brief account of the behavior,
checks, and any important exception. For reviews, lead with actionable findings,
file locations, and consequences; distinguish correctness problems from style
preferences. For explanations, show the code and explain what happens in plain
English.

## Supporting files

- [Bundle README](README.md): compatibility, attribution, licensing, and maintenance.
- [Uber Go guidelines](references/uber-go-guide.md): all source sections.
- [Composition and reuse](references/composition.md): original code and walkthrough.
- [Runnable original example](examples/composition-original.go): explanatory source,
  not a production template or a file to reformat during unrelated tasks.
