# API design

Read this before changing exports, a package entry point, a public
signature, error types callers check, or anything other packages or
modules depend on.

## Contents

- [Decide what is public](#decide-what-is-public)
- [Prefer named exports](#prefer-named-exports)
- [Shape parameters and results](#shape-parameters-and-results)
- [Keep imports one-way](#keep-imports-one-way)
- [Treat types as part of the API](#treat-types-as-part-of-the-api)
- [Make errors part of the API](#make-errors-part-of-the-api)
- [Change public APIs gently](#change-public-apis-gently)

## Decide what is public

A package's public API is what its `package.json` `exports` field lets
callers import, plus anything its docs promise. Everything else is
private, even when it is exported from a file. List entry points in
`exports` on purpose;
[Packages](packages.md#declare-entry-points-with-exports) shows the
shape. Adding `exports` to a package that had none blocks deep imports,
so treat it as a breaking change.

Inside a project, export only what other modules use. Keep the export
list of an entry module explicit:

```ts
export { ConfigError, loadConfig, parseConfig } from "./config.ts";
export type { Config, LogLevel } from "./config.ts";
```

Avoid `export * from` an internal module in a public entry point; it
publishes every later export of that module without review.

## Prefer named exports

Use named exports. A named export has one name in every file that
imports it, and a misspelled import fails. A default export can be
imported under any name, which makes searching and renaming harder. Use
a default export only when the project or a framework or tool requires
one, such as a config file or a route module.

## Shape parameters and results

Take an options object instead of more than two positional parameters or
any boolean flag; `save(doc, { overwrite: true })` reads better than
`save(doc, true)`. Give options defaults in the destructuring, `{
timeoutMs = 10_000 } = {}`.

Return one kind of "missing" value. Use `undefined` unless the
surrounding API uses `null`. Return the same kind of result on every
path: a function that returns a promise must always return one, never a
plain value on some paths.

Accept an `AbortSignal` as `options.signal` in any API that does
long-running I/O, so callers can cancel it.
[Concurrency](concurrency.md#cancel-with-abortsignal) covers how.

Use a default parameter value instead of reassigning a parameter in the
body. Reassigning hides the value the caller passed, and a falsy check
such as `limit = limit || 10` also replaces a real `0` ([Airbnb, default
parameters](https://github.com/airbnb/javascript#es6-default-parameters)).

Return an object, not an array, when a function gives back several named
results. Callers then pick values by name, and adding a result later
does not shift the others ([Airbnb, object over
array](https://github.com/airbnb/javascript#destructuring--object-over-array)).
A tuple fits only a fixed pair whose order is obvious, such as `[key,
value]`.

## Keep imports one-way

Avoid circular imports between modules. ES modules allow a cycle, but
then one module in it runs before the other has finished, so a top-level
use of an import can read a `const` or class that is not set yet and
throw a `ReferenceError`. Move the part both modules need into a third
module that both import. The
[`import/no-cycle`](https://github.com/import-js/eslint-plugin-import/blob/main/docs/rules/no-cycle.md)
rule finds cycles; it is slow on large projects, so run it the way the
project already does.

## Treat types as part of the API

For a library, a type change can break callers even when the JavaScript
does not change. Making a parameter required, widening a return type,
adding a member to a union that callers `switch` over, or removing an
exported type is a breaking change. Making a parameter optional or
narrowing a return type usually is not.

Do not expose a dependency's types in your public signatures unless you
mean to. Doing so ties your callers to that dependency's version.

## Make errors part of the API

Document what a public function throws or rejects with. Give each error
a stable `name` or `code` that callers can check, and keep messages for
humans; callers should not parse message text. [Errors and
resources](errors-resources.md) covers error classes and `cause`.

## Change public APIs gently

Keep an old name working while callers move. Mark it with `@deprecated`
and say what to use instead:

```ts
/** @deprecated Use `loadConfig` instead; it also checks the file. */
export function readConfig(path: string): Promise<Config> {
  return loadConfig(path);
}
```

Remove deprecated APIs only in a major version. Follow the project's
versioning and changelog rules.
