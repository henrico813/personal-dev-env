# Design choices

Read this before adding a function, class, union, module, or other
abstraction, or before changing how state and dependencies are shaped.

## Contents

- [Start with a function](#start-with-a-function)
- [Use a class for state with rules](#use-a-class-for-state-with-rules)
- [Model states with unions](#model-states-with-unions)
- [Separate decisions from effects](#separate-decisions-from-effects)
- [Pass dependencies in](#pass-dependencies-in)
- [Keep data read-only at
  boundaries](#keep-data-read-only-at-boundaries)
- [Choose the right collection](#choose-the-right-collection)
- [Avoid abstractions without a present
  need](#avoid-abstractions-without-a-present-need)
- [Follow the project where style guides
  disagree](#follow-the-project-where-style-guides-disagree)

## Start with a function

Use an exported function for an operation or a transformation. A module
is already a namespace, so do not wrap functions in a class of static
methods or an object literal only to group them. Keep helpers
module-private by not exporting them.

Prefer `function` declarations for named top-level functions; they read
clearly and can be called before their line. Use arrow functions for
callbacks.

When a function such as `map` takes a callback, pass an arrow function
rather than a named function whose extra parameters could receive
surprise arguments. `["10", "10"].map(parseInt)` returns `[10, NaN]`
because `map` passes the index as the radix.

## Use a class for state with rules

Use a class when state, invariants, and lifetime belong to one owner: a
connection pool, a cache with eviction, or an object with a `close()`
method. Keep fields `readonly` unless a method changes them, and
initialize every field in its declaration or the constructor so
instances keep one shape.

Prefer composition to inheritance. Extend a class only for a real "is a"
relation or when a framework or platform requires it, such as `extends
Error`.

## Model states with unions

Use a union of object types with a shared literal field, called a
discriminated union, when a value can be in one of several states.
Separate booleans can contradict each other; a union cannot.

```ts
type Download =
  | { state: "waiting" }
  | { state: "running"; receivedBytes: number }
  | { state: "failed"; error: Error }
  | { state: "done"; path: string };

function describe(download: Download): string {
  switch (download.state) {
    case "waiting":
      return "Waiting";
    case "running":
      return `${download.receivedBytes} bytes`;
    case "failed":
      return download.error.message;
    case "done":
      return download.path;
    default:
      return assertNever(download);
  }
}

function assertNever(value: never): never {
  throw new Error(`Unexpected value ${String(value)}`);
}
```

The `never` check turns a new state that the `switch` misses into a type
error. Use literal unions or `as const` objects rather than `enum` in
new code; [Typing](typing.md#write-erasable-syntax) explains why.

## Separate decisions from effects

Keep rules in functions that take values and return values, and keep
file, network, clock, and process access in thin functions around them.
The rules become easy to test, and failure handling for the effects
stays in one place. In [config.ts](../examples/src/config.ts),
`parseConfig` checks values and `loadConfig` only reads and parses the
file before calling it.

## Pass dependencies in

When a test needs to replace a dependency, such as the clock, `fetch`,
or a storage client, take it as a parameter or an options field with a
default. This is plainer than mocking a module import, and it shows the
dependency in the signature. Do not add a dependency-injection container
or a registry for this.

## Keep data read-only at boundaries

Accept `readonly T[]` and objects with `readonly` fields in public APIs,
and do not change arguments the caller passed in. Return new values
instead. Do not export a mutable binding with `export let`; export a
function that reads the value.

## Choose the right collection

Use `Map` and `Set` for keys that come from data. Plain objects inherit
keys such as `toString`, and a key like `__proto__` can change an
object's prototype. Use plain objects for records with fixed, known
keys.

## Avoid abstractions without a present need

Do not add a base class, generic repository, plugin system, event bus,
or wrapper around a library for one use. Add a boundary when it owns a
real rule, resource, or variation that exists today. Three similar lines
are often clearer than a helper with flags.

## Follow the project where style guides disagree

The Google, Airbnb, and Standard style guides disagree on the points
below. Follow the project's config and nearby code first. For new code:

- **Default exports.** Google bans them, Airbnb prefers one in a module
  with a single export, and Standard allows them. Use named exports, as
  [API design](api-design.md#prefer-named-exports) explains.
- **Semicolons.** Standard leaves them out; Google and Airbnb require
  them. Leave them to the formatter. Either choice is a preference, not
  a correctness rule.
- **Import extensions.** Google's JavaScript guide requires `.js` in
  import paths, and Airbnb's lint config forbids extensions. Follow the
  module resolution setting: write extensions under `nodenext`, and
  follow the project under `bundler`.
  [Packages](packages.md#write-import-paths-the-runtime-accepts)
  explains why.
- **Namespace imports.** Google allows `import * as name` for a large
  API, and Airbnb bans wildcard imports. Use named imports, and a
  namespace import only when a file uses many names from one large
  module.
