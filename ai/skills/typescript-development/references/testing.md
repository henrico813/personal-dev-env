# Testing

Read this before planning, writing, changing, or reviewing JavaScript or
TypeScript tests. Load and follow the `behavior-focused-testing` skill
as well; its rules on value, names of seven words or fewer, and scope
apply here. This page adds the JavaScript mechanics.

## Contents

- [Pick the runner](#pick-the-runner)
- [Group tests by subject](#group-tests-by-subject)
- [Test through the public API](#test-through-the-public-api)
- [Prefer real parts to mocks](#prefer-real-parts-to-mocks)
- [Control time and randomness](#control-time-and-randomness)
- [Assert on behavior](#assert-on-behavior)
- [Test user interfaces as users use
  them](#test-user-interfaces-as-users-use-them)
- [Test public types](#test-public-types)

## Pick the runner

Use the runner the project already has, whether Vitest, Jest,
`node:test`, Mocha, AVA, or Playwright Test. Do not switch runners
inside a task. For a project without one, use Vitest in a Vite project,
because it reuses the Vite config and transforms, and `node:test` in a
plain Node project, because it needs no dependency and runs `.ts` test
files through Node's type stripping.

`node:test` finds `*.test.ts` and `*.test.js` files with `node --test`.
The runner reports each failure itself, and the promises that
`describe()` and `test()` return never reject, so tell
typescript-eslint's `no-floating-promises` rule they are safe, as [the
example config](../examples/eslint.config.js) does.

## Group tests by subject

Put the tests for one function, class, or feature in one `describe`
block named after it, and follow the file layout nearby tests use. Write
each test name as a short plain sentence about behavior: `test("rejects
with the failed status", ...)`. Use a table of cases with a loop when
cases share setup and assertions, and give each case a name.

## Test through the public API

Call what real callers call: exported functions, a class's public
methods, an HTTP handler through a request, or a package through its
name. Do not export a private helper only to test it.
[package.test.ts](../examples/test/package.test.ts) imports the example
package by its name, so the import goes through `package.json` `exports`
as a caller's import does.

## Prefer real parts to mocks

Use the real thing when it is fast and local: a temporary directory from
`mkdtemp` for file code, and a `node:http` server on port 0 for HTTP
clients. Mocking `fetch` tests the mock's idea of HTTP, not the request
your code sends. A fake server must be able to give every answer the
real one gives that your code handles, such as an error status, a slow
reply, and no reply;
[fetch-json.test.ts](../examples/test/fetch-json.test.ts) does this.

Replace a dependency only at a boundary you do not control or cannot
run. Prefer passing the dependency in over module mocking such as
`vi.mock()` or `mock.module()`, which ties tests to import paths.
Restore every mock, timer, environment variable, and global you change,
in `afterEach` or `after`.

## Control time and randomness

Use fake timers, such as `vi.useFakeTimers()` or `mock.timers` in
`node:test`, instead of real sleeps for code that waits. A short real
timeout is acceptable when the test needs the real event loop, as with a
socket. Pass a seed or a generator for random values, and a fixed clock
for dates.

## Assert on behavior

Use `node:assert/strict`, or the runner's `expect`. Check the observable
result: return values, thrown or rejected errors, files written, and
requests sent. Match errors by `name`, `code`, and fields rather than
the whole message, unless the message is the behavior. Await
`assert.rejects` and `expect(...).rejects`; a missing `await` makes a
failing test pass.

Use snapshots only for output whose exact text is the behavior, such as
a generated file or a CLI help screen, and review every snapshot change.

## Test user interfaces as users use them

Testing Library's [guiding
principles](https://testing-library.com/docs/guiding-principles) make
the case for this: a test earns more trust the closer it comes to how
people use the software.

Find elements by role, label, and visible text, not by class names or
test IDs, and act through user events. Test framework-specific behavior
the way the framework's docs say. Use Playwright for behavior that needs
real browsers, such as layout, navigation, or differences between
browser engines.

## Test public types

When a library's types are part of its API, check them. Use the runner's
type assertions, such as Vitest's `expectTypeOf`, or a type-test file
that `tsc` checks, where each `// @ts-expect-error` line says which
misuse it rejects.
