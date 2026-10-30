# Google TypeScript Style Guide, condensed

Source: [Google TypeScript Style
Guide](https://google.github.io/styleguide/tsguide.html), by Google,
licensed under [CC BY
3.0](https://creativecommons.org/licenses/by/3.0/); the license text is
in [CC-BY-3.0.txt](../CC-BY-3.0.txt). This page is a condensed and
modified adaptation. It keeps the rules that help outside Google,
shortens their examples and reasons, and leaves out rules about Google's
build tools, Closure, Angular, Polymer, and pure formatting. It is not a
copy, and Google does not endorse it. Reviewed October 28, 2026.

It also leaves out rules that the default lint setup in
[Tooling](tooling.md#lint-with-eslint-and-typescript-eslint) already
enforces, as it leaves out formatting.

The Google JavaScript Style Guide is not adapted here. Google no longer
updates it, and its Closure-only rules, such as `goog.module`, Closure
JSDoc types, clang-format, and the 80-column limit, do not apply to
TypeScript.

Read it for naming, imports, and language-feature questions that the
project's conventions and tools do not settle. The project's own rules
and configured tools come first.

## Contents

- [Where this skill differs](#where-this-skill-differs)
- [Naming](#naming)
- [Imports and exports](#imports-and-exports)
- [Variables, arrays, and objects](#variables-arrays-and-objects)
- [Classes](#classes)
- [Functions and this](#functions-and-this)
- [Strings and numbers](#strings-and-numbers)
- [Control flow](#control-flow)
- [Exceptions](#exceptions)
- [Type assertions](#type-assertions)
- [Disallowed features](#disallowed-features)
- [Type system](#type-system)
- [Comments](#comments)
- [Policies](#policies)

## Where this skill differs

The guide was written for Google's codebase. This skill keeps these
defaults instead:

- Do not write `enum` or parameter properties in new code, because
  Node's type stripping rejects them. The guide allows both.
- Use a default export only when the project or a framework requires
  one. The guide bans default exports.
- Allow `#private` fields. The guide bans them because of output size
  when compiling to old targets, which modern targets do not need.
- Leave quotes, semicolons, and other formatting to the project's
  formatter.
- Annotate the return types of exported functions. The guide leaves
  return types to the author.
- When a type error cannot be fixed, use `@ts-expect-error` with a
  reason, in any file. The guide allows it only in tests and prefers a
  cast with a comment, but a cast hides the error even after the cause
  is gone.
- Write the reason beside every `as` cast and `!` assertion. The guide
  lets you leave it out when the reason is obvious.

## Naming

- Use only ASCII letters and digits in names, with underscores only in
  `CONSTANT_CASE`. Use `$` only when a framework's convention asks for
  it.
- Use `UpperCamelCase` for classes, interfaces, types, enums,
  decorators, and type parameters; `lowerCamelCase` for variables,
  parameters, functions, methods, and properties; `CONSTANT_CASE` for
  module-level constants and static readonly class fields that never
  change.
- A single capital letter such as `T` is fine for a type parameter.
- Treat acronyms as words: `loadHttpUrl`, not `loadHTTPURL`, unless a
  platform name requires otherwise.
- Make names clear to a new reader. Do not use unfamiliar abbreviations
  or drop letters inside words (`cstmrId`). A short name is fine for a
  variable that lives ten lines or less and is not part of an exported
  API.
- Do not repeat type information in a name. No `_` prefix or suffix for
  private members, no `opt_` prefix for optional parameters, and no `I`
  prefix for interfaces. Name an interface for why it exists, such as
  `TodoItemStorage`.
- A local alias of an existing name keeps that name's format.

## Imports and exports

- Import with relative paths (`./foo`) inside one project, and keep
  `../` chains short.
- Use named imports for a few symbols with clear names. Use a namespace
  import (`import * as tableview from "./tableview"`) when a file uses
  many symbols from a large API with generic names.
- Rename an import only to avoid a collision or to clarify an unclear
  name.
- Use named exports. The guide gives two reasons: named imports fail
  when the name does not exist, and a default export can be imported
  under any name.
- Export only what other modules use.
- Do not export mutable bindings (`export let`). Compute a value first
  and export it as `const`, or export a getter function.
- Do not create classes with only static members to group functions;
  export the functions and constants.
- Use `import type` for symbols used only as types, and `export type` to
  re-export types.

## Variables, arrays, and objects

- Declare one variable per statement, and do not use a variable before
  its declaration.
- Do not use the `Object()` constructor; use an object literal.
- Do not add non-index properties to arrays; use a `Map` or an object.
- Spread only what matches the result: iterables into arrays, objects
  into objects. Do not spread `null`, `undefined`, or a value that might
  be one. Avoid spreading class instances; only their own enumerable
  properties are copied.
- Do not use plain `for...in`. Use `for...of` over `Object.keys`,
  `Object.values`, or `Object.entries`.
- Keep destructured parameters to one level of plain names. Give
  defaults on the left (`{ num = 1 } = {}`), and default an optional
  destructured parameter to `{}` or `[]`.

## Classes

- Do not add a constructor that does nothing or only calls `super` with
  the same arguments.
- Mark properties `readonly` when they are never reassigned after
  construction.
- Initialize fields where they are declared when they do not come from a
  parameter, and do not add or remove properties after the constructor
  finishes.
- Do not use `this` in static methods, and do not call a static method
  through a subclass that does not define it. Prefer a module-level
  function to a private static method.
- Getters must not change observable state. Do not write accessors that
  only pass a hidden field through; make the field public or `readonly`.
- Do not use `obj["foo"]` to reach a private member.
- Leave out the `public` modifier; members are public by default.
- `toString()` must always succeed and have no side effects.
- Do not change prototypes directly or define mixins on them.
- Do not define new decorators. Use the ones a framework provides.

## Functions and this

- Prefer function declarations for named functions. Use arrow functions
  for callbacks, and do not use `function` expressions except for
  generators. A nested function may be a declaration or an arrow
  function.
- Use a concise arrow body only when the caller uses the return value;
  otherwise use a block body.
- Do not rebind `this`. Use arrow functions or explicit parameters
  instead of `bind`, `call`, or `apply`.
- Do not store arrow functions in class properties to bind `this`,
  except for an event handler that must later be removed. Do not install
  handlers with `bind`, which creates a new function that cannot be
  removed.
- Keep parameter defaults simple and free of side effects. Use an
  options object when there are many optional parameters.
- Use `this` only in class constructors and methods, functions that
  declare a `this` type, and arrow functions inside those.

## Strings and numbers

- Do not continue a string literal across lines with a backslash. Use
  template literals for complex concatenation.
- Convert with `String()`, `Boolean()`, `Number()`, `!!`, or a template
  literal, never with `new`.
- Parse numbers with `Number()` and check for `NaN`. `Number("")` is
  `0`, so check empty input too. Do not use unary `+`, and do not use
  `parseInt` or `parseFloat` for base-10 input, because they ignore
  trailing characters (`parseInt("12 dwarves")` is `12`). For other
  bases, check the digits before calling `parseInt`.

## Control flow

- Use braces for every control-flow block. A one-line `if` may leave
  them out.
- Every `switch` has a `default` case, placed last.
- Use `===` and `!==`. `== null` is allowed to match both `null` and
  `undefined`.
- Add grouping parentheses whenever a reader could misread precedence.

## Exceptions

- Define an error subclass when callers need more information than
  `Error` carries.
- In a `catch`, narrow the value with `instanceof Error` before using
  it. Handle other thrown values only when an API is known to throw
  them, and say which API in a comment.
- Keep `try` blocks to the code that can throw.

## Type assertions

- `as` casts and `!` non-null assertions only silence the compiler.
  Prefer a runtime check. When local facts make an assertion safe, write
  the reason beside it unless it is obvious.
- Use `as` syntax, not `<Foo>x`.
- When types do not overlap, cast through `unknown`, not `any`.
- Type object literals with an annotation (`const foo: Foo = {...}`),
  not `as Foo`, so misspelled fields are reported.

## Disallowed features

- `eval`.
- Non-standard and not-yet-standard language or web features, unless the
  project targets a runtime that has them.
- Changing built-in objects or their prototypes, and adding globals
  without a strong reason.

## Type system

- Rely on inference for simple values. Annotate empty collections and
  complex expressions whose type a reader could not guess.
- TypeScript allows `undefined` or `null` for missing values; follow the
  API around you. Do not put `| null` or `| undefined` inside a type
  alias. Prefer optional fields and parameters to `| undefined`.
- The guide prefers `interface` to a type alias for object shapes.
  Follow the project when it chose otherwise.
- Write `T[]` and `readonly T[]` for simple element types, and
  `Array<T>` for complex ones.
- Prefer `Map` and `Set` to objects with index signatures. Give index
  signature keys a meaningful name (`[userName: string]`).
- Use the simplest type that works. A little repetition beats a mapped
  or conditional type readers must evaluate in their heads.
- Avoid `any`. Use a specific type, `unknown`, or a generic, and when
  `any` is needed, suppress the lint warning with a reason.
- Use tuples for pairs only when names would not be clearer; an object
  with named fields often is.
- Avoid generics that appear only in the return type. When calling an
  API that has them, pass the type argument explicitly.

## Comments

- Use `/** */` doc comments for what users of the code read, and `//`
  comments for implementation notes.
- Write multi-line implementation comments as several `//` lines.
- Write doc comments in Markdown, with one tag per line.
- Document every export whose purpose is not obvious from its name,
  without restating the name or type.
- Start method descriptions with a verb phrase in the third person, as
  if "This method" came first.
- Do not write types in JSDoc tags in TypeScript files.
- Add `@param` and `@returns` only when they add information.
- When an argument's meaning is unclear at the call site, add a name
  comment before it, `someFunction(/* shouldRender= */ true)`, or
  better, take an options object.
- Put doc comments before decorators.

## Policies

- When the guide does not settle a question, do what the file already
  does, then what nearby files do.
- New code in an old file may follow the file's style. Make large style
  cleanups a separate change.
- Mark deprecated APIs with `@deprecated` and clear directions for
  callers.
- Generated code need not follow the guide, but names that hand-written
  code uses must.
