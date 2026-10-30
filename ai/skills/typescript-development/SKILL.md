---
name: typescript-development
description: >-
  Plans, writes, reviews, refactors, and tests maintainable TypeScript
  and JavaScript code. Use when planning changes to or working on .ts,
  .tsx, .mts, .cts, .js, .jsx, .mjs, or .cjs files, including plain
  JavaScript typed with JSDoc and @ts-check; on types and tsconfig,
  modules and package.json, lint and format setup, errors, promises
  and cancellation, or tests; and when inspection shows the target is
  JavaScript or TypeScript. Do not use it for basic syntax
  explanations, README-only edits, or merely because a repository
  contains JavaScript.
---

# TypeScript and JavaScript development

Write plain, typed, maintainable TypeScript and JavaScript. The
project's runtime, framework, and config come first; the defaults here
apply to new code and to choices the project has not made.

## Inputs

Read these before deciding anything:

- The request and the repository's agent instructions.
- `package.json`, especially `type`, `engines`, `exports`, `scripts`,
  and `packageManager`.
- The lockfile, which names the package manager.
- Every `tsconfig*.json` or `jsconfig.json`.
- Lint and format config, such as `eslint.config.*`, `.prettierrc*`,
  `biome.json`, or `.oxlintrc.json`.
- CI workflows, the runtime the code ships to, and the framework's docs
  when the code uses one.
- The code and tests the task touches.

## Workflow

1. Load this skill before repository research when the task is clearly
   JavaScript or TypeScript, or as soon as inspection shows it is.
2. Find the rows below that the task touches. Open each reference and
   read the sections that apply before deciding. A typo fix needs no
   reference. Loading this `SKILL.md` or the shared testing and
   documentation skills does not count as reading a reference.
   The defaults list does not replace these reference rows. Open each
   reference row that the task touches.
3. Decide with the project's config and conventions first, then the
   [defaults](#defaults-for-new-code). For React, Vue, Svelte, Angular,
   Next, Electron, and other frameworks, follow the framework's docs;
   this skill does not cover them.
4. Make the smallest complete change. Do not add a dependency, build
   tool, config file, or new pattern unless the task needs it. Do not
   edit files for a review-only request.
5. Run the [tool checks](#tool-checks). Fix what the change broke and
   run them again until they pass, or report what still fails.
6. Review the diff and report as [Output](#output) says.

| Task touches | Read first |
| --- | --- |
| Types, `any` or `unknown`, narrowing, assertions, generics, `enum` or other non-erasable syntax, or `tsconfig` strictness | [Typing](references/typing.md) |
| Plain JavaScript with JSDoc types or `@ts-check` | [Typing](references/typing.md) + [Documentation](references/documentation.md) |
| Functions, classes, unions, state, dependencies, or a new abstraction | [Design choices](references/design-choices.md) |
| Exports, public signatures, error types callers check, or deprecation | [API design](references/api-design.md) |
| `package.json`, ESM or CommonJS, import paths, module resolution, builds, dependencies, lockfiles, or publishing | [Packages and modules](references/packages.md) |
| Throwing, catching, error causes, result objects, or cleanup | [Errors and resources](references/errors-resources.md) |
| Promises, parallel work, timeouts, `AbortSignal`, CPU-heavy work, or reviews of code that starts many promises, loops over remote calls, or uses `Promise.all` | [Concurrency](references/concurrency.md) |
| Tests, test runners, fakes, mocks, or timers | [Testing](references/testing.md); cleanup in tests also [Errors and resources](references/errors-resources.md) |
| Doc comments, lint or type suppressions, or `@deprecated` | [Documentation](references/documentation.md) |
| `tsc`, ESLint, Prettier, Biome, Oxlint, or check commands | [Tooling](references/tooling.md) |
| Naming, imports, or language features the project and tools do not settle | [Google TypeScript guide, condensed](references/google-ts-guide.md) |

The [example index](references/examples.md) links decisions to complete,
tested source in `examples/`. Read the examples; do not copy the package
as a starter. Run it only when revising or validating this skill.


## Defaults for new code

Each default applies only where the project has not decided. Do not
migrate existing code to a default unless the task asks.

- **Strict types.** Use `strict: true`. Type untrusted input as
  `unknown` and narrow it before use. Use `any` only with a comment that
  says why. Check external data at runtime, because types vanish when
  code runs; a schema library such as Zod or Valibot is an option, not a
  requirement.
- **Erasable syntax.** Do not write `enum`, `namespace` with values, or
  constructor parameter properties in new code. Node runs `.ts` files by
  stripping types and rejects syntax that needs code generation. Use a
  literal union or an `as const` object instead.
- **Named exports.** Use named exports. Use a default export only when
  the project or a framework requires one. A named export keeps one name
  everywhere it is imported.
- **Return types.** Annotate the return types of exported functions and
  let local code infer. The annotation stops a change inside the body
  from silently changing the public API.
- **ES modules.** Set `type` in `package.json`, and set `exports` in a
  package that others install. Write file extensions in relative imports
  for Node. Use `moduleResolution` `nodenext` for code Node runs or
  others install, and `bundler` for an app a bundler builds.
- **Lint and format.** Lint with ESLint flat config and
  typescript-eslint `recommendedTypeChecked`. Format with Prettier. Keep
  Biome, or Oxlint with Oxfmt, only in a project that already uses them.
  Set up ESLint and Prettier together in a new package. Run one
  formatter only. Semicolons and quotes are the formatter's choice.
- **Errors.** Follow the project's error model: thrown errors, error
  classes, or result objects. Keep the original error with `new
  Error(message, { cause })`. Never leave a rejected promise unhandled.
- **Tests.** Use the project's test runner. Without one, use Vitest in
  a Vite project and `node:test` in a plain Node project.

## Tool checks

Use the project's scripts first, such as `npm run lint` and `npm test`,
because they carry its flags. Use the package manager its lockfile
names: npm for `package-lock.json`, pnpm for `pnpm-lock.yaml`, Yarn for
`yarn.lock`, and Bun for `bun.lock`. Use npm when there is no lockfile.

When the project has no script for a check, run the tool it installs.
For an npm project:

```sh
npm ci
npx --no -- prettier --check <changed files>
npx --no -- eslint <changed files>
npx --no -- tsc --noEmit -p tsconfig.json
npm test
```

Run `npm ci` only when `node_modules` is missing, because it deletes and
reinstalls every package;
[Packages](references/packages.md#install-from-the-lockfile) gives the
commands for other package managers. `--no` stops `npx` from downloading
a package the project does not have, and `--` keeps `npx` from reading
the tool name as the value of `--no`. [Tooling](references/tooling.md)
covers TypeScript 7 beside typescript-eslint, project references, and
existing lint errors.

When a check is not configured, do not add the tool for an unrelated
change; say it was not run. When a tool or `node_modules` is missing and
cannot be installed, report each check that did not run and why. Never
install tools globally without the user's permission.

## Guardrails

- Do not change `tsconfig` strictness, module format, `target`,
  `engines`, the package manager, or the test runner unless the task
  asks.
- Never silence the compiler with `@ts-ignore`, `@ts-nocheck`, or `as
  any`, and never write `!` without a reason beside it. When a type
  error cannot be fixed, use `@ts-expect-error` with the reason beside
  it.
- Give every `eslint-disable` comment a rule name and a reason.
- Do not hand-edit lockfiles or generated files, and review lockfile
  changes in the diff.
- Do not reformat files the task does not touch.
- Do not run `npm publish`, change registry credentials, or print tokens
  or `.env` values.
- Load `behavior-focused-testing` when planning, writing, or reviewing
  tests, and `code-documentation` when planning, writing, changing, or
  reviewing source code or tests.

## Output

For a change, report what changed and why, each check run with its
result, checks that could not run, and remaining risks. For a review,
lead with findings: file and line, consequence, and whether each is a
defect or a preference. Never claim a check ran unless it did.

## Final checks

- No new `any`, unexplained assertion, or suppression.
- Untrusted data is checked where it enters.
- Errors keep their cause, and every promise has an owner.
- Public exports and `package.json` `exports` match what callers use.
- Configured format, lint, type, and test checks pass, or each gap is
  reported.
- The diff has no unrelated formatting, lockfile, or generated changes.

## Supporting files

The [bundle README](README.md) has version notes, sources and licensing,
and maintenance notes. Use [sources](references/sources.md) for
attribution and [evaluation](references/evaluation.md) only when
installing or revising this skill. The [example
package](examples/README.md) has the check commands.
