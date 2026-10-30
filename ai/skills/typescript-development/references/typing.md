# Typing

Read this before changing types, `tsconfig` strictness, narrowing, type
assertions, generics, or JSDoc types in JavaScript files.

## Contents

- [Start strict](#start-strict)
- [Type untrusted data as unknown](#type-untrusted-data-as-unknown)
- [Narrow with real checks](#narrow-with-real-checks)
- [Check external data at runtime](#check-external-data-at-runtime)
- [Annotate the public surface](#annotate-the-public-surface)
- [Keep type-level code simple](#keep-type-level-code-simple)
- [Write erasable syntax](#write-erasable-syntax)
- [Type plain JavaScript with JSDoc](#type-plain-javascript-with-jsdoc)

## Start strict

Use `strict: true` in new projects. TypeScript 6 and later turn it on by
default, but writing it out keeps the setting visible to readers and to
older compilers. Do not loosen strictness to make an error go away.

These flags catch more bugs and are worth proposing for new code.
Turning one on in an existing project usually reports many errors, so do
it as its own change, not inside an unrelated task:

- `noUncheckedIndexedAccess` types `items[i]` and `record[key]` as
  possibly `undefined`.
- `exactOptionalPropertyTypes` stops code from writing `undefined` into
  an optional property, so "missing" and "set to undefined" stay
  different.
- `noImplicitOverride` requires `override` on methods that replace a
  parent method, so a renamed parent method cannot leave a stale
  override behind.

From TypeScript 6, `types` defaults to `[]`, so global types such as
`@types/node` load only when listed, for example `"types": ["node"]`.
Match the `@types/node` major version to the oldest Node version the
project supports; a newer major lets code call APIs the oldest runtime
does not have.

## Type untrusted data as unknown

Use `unknown` for any value the program did not create: `JSON.parse`
results, `response.json()`, message events, file contents, and `catch`
variables. `unknown` forces a check before use; `any` turns checking off
for every value it touches.

```ts
const data: unknown = JSON.parse(text);
```

Use `any` only when no precise type is practical, such as a partial test
double, and write the reason beside it. Prefer `unknown`, a generic, or
a precise type first.

## Narrow with real checks

Narrow with checks that run: `typeof`, `instanceof`, `Array.isArray`,
the `in` operator, a check of the literal field that tells union members
apart, such as `if (result.ok)`, and type guards.
A type guard (`value is T`) or assertion function (`asserts value is T`)
must check what it claims; a guard that lies is worse than a cast
because readers trust it.

Avoid `as T` and the non-null `!`. Both silence the checker without
adding a runtime check. When local facts make one safe, write why in a
comment beside it. Cast through `unknown`, never `any`, when two types
do not overlap.

Use an annotation, not an assertion, to type an object literal. `const
config: Config = {...}` reports a misspelled or extra field; `{...} as
Config` does not. Use `satisfies` to check a value against a type while
keeping its narrower inferred type:

```ts
const routes = {
  home: "/",
  settings: "/settings",
} satisfies Record<string, `/${string}`>;
```

## Check external data at runtime

Types disappear when the code runs, so a type annotation on parsed JSON
proves nothing. Check external data once, where it enters the program,
and pass a typed value inward.

For a small shape, a hand-written check is enough;
[config.ts](../examples/src/config.ts) narrows `unknown` field by field
and reports every bad field. For larger or nested shapes, use a schema
library and derive the static type from the schema so the two cannot
drift. Zod, Valibot, and ArkType are common choices. Standard Schema is
a small shared interface those libraries implement, so a function can
accept a schema from any of them. Use the library the project already
has; add one only when the task needs it.

## Annotate the public surface

Annotate the return type of exported functions and methods. The
annotation documents the API and turns an accidental change inside the
body into an error at the function, not at a distant caller. Let local
variables and private helpers infer their types.

Annotate where inference gives the wrong type: an empty collection (`new
Set<string>()`), a value that would widen, or a recursive function.

Interfaces and type aliases both describe object shapes. Follow the
project. When it has not decided, use `interface` for object shapes and
`type` for unions, tuples, and mapped or conditional types.

Do not put `| undefined` or `| null` inside a type alias; add it where
the alias is used, so readers see where a value can be missing. Use
optional properties and parameters (`name?: string`) instead of `name:
string | undefined` when the value can be left out.

## Keep type-level code simple

Use the simplest type that describes the code. A little repetition is
cheaper to maintain than a mapped or conditional type that readers must
evaluate in their heads. Use generics when a function really works for
many types and the type parameter appears in its inputs. A generic that
appears only in the return type, such as `function load<T>(): T`, is a
cast in disguise.

## Write erasable syntax

Node runs `.ts` files by replacing type syntax with spaces. It rejects
syntax that needs code generation: `enum`, `namespace` blocks with
runtime code, constructor parameter properties, and `import x =
require()` aliases. Do not write these in new code, even when a build
step would accept them, so the same file works under `tsc`, bundlers,
and Node.
In a new project, set `erasableSyntaxOnly` so `tsc` reports them. In an
existing project, turn it on only as its own change, because it reports
every existing enum and namespace as an error.

Use a union of string literals, or an `as const` array or object when
code also needs the values at runtime:

```ts
export const LOG_LEVELS = ["debug", "info", "warn", "error"] as const;
export type LogLevel = (typeof LOG_LEVELS)[number];
```

Assign fields in the constructor body instead of using parameter
properties. Use `import type` and `export type` for type-only imports
and exports; Node needs the `type` keyword to know which imports to
drop. In a new project, set `verbatimModuleSyntax` so the compiler
requires them, and in an existing project turn it on only as its own
change. Existing enums and namespaces can stay until the project decides
to move off them.

## Type plain JavaScript with JSDoc

TypeScript checks `.js` files that start with `// @ts-check`, or every
JavaScript file when `checkJs` is on in `tsconfig.json` or
`jsconfig.json`. In those files JSDoc is the type system:

```js
// @ts-check
/** @import { Config } from "./config.ts" */

/**
 * @param {Config} config
 * @param {(line: string) => void} write
 * @returns {number}
 */
export function report(config, write) {
  write(`port ${config.port}`);
  return 1;
}
```

Use `@typedef`, `@template`, `@satisfies`, and `@type` the same way.
TypeScript 7 checks JavaScript much more like TypeScript files, so write
JSDoc types in TypeScript syntax:

- Write function types as `(s: string) => void`, not Closure style
  `function(string): void`.
- Name a type inside its tag, `/** @typedef {T} Name */`, not on the
  next line.
- Write `typeof value` where a type is expected; a bare value name is
  not a type.
- `@enum` and `@class` no longer change meaning; use a `@typedef` over
  an `as const` object, and a `class` declaration.
- Postfix `!` and a standalone `?` are not types; write `T` and `any`.

[slug.js](../examples/src/slug.js) is a checked JavaScript module that
the TypeScript build emits declarations for.
