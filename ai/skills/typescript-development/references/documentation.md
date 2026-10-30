# Documentation

Read this before writing or changing doc comments, implementation
comments, JSDoc types in JavaScript files, or deprecation notes. Load
and follow the `code-documentation` skill as well; it decides whether a
comment belongs at all. This page adds the JavaScript and TypeScript
mechanics.

## Use doc comments for callers

Write `/** ... */` doc comments on exports whose behavior a caller
cannot learn from the name and types: side effects, errors, units,
defaults, and ordering. Editors show them on hover. Use `//` comments
for implementation reasons that only maintainers need.

Start a doc comment with what the code does, such as "Returns ..." or
"Thrown when ...", never with the name of the thing it documents.

```ts
/**
 * Checks untrusted config data and fills defaults for missing fields.
 * Throws a `ConfigError` that lists every invalid field, not only the
 * first one.
 */
export function parseConfig(input: unknown): Config {
```

## Add tags only when they add facts

In TypeScript files the types are in the code, so do not repeat them in
`@param {string}` or `@returns {number}`. Write `@param name text` only
when the text says more than the name and type. These tags carry facts
the signature cannot:

- `@throws` for errors callers should expect, including rejections.
- `@deprecated` with what to use instead; editors strike through the
  name.
- `@example` for a call whose use is not obvious.
- `@see` for a related API or a specification.

Write doc text in Markdown. Lists and code spans render in editors;
hand-aligned plain text does not.

## Type JavaScript with JSDoc

In checked `.js` files, JSDoc tags are the types and are required where
inference falls short: `@param {T}`, `@returns {T}`, `@type`,
`@typedef`, `@template`, `@satisfies`, and `@import`.
[Typing](typing.md#type-plain-javascript-with-jsdoc) lists the JSDoc
forms TypeScript 7 no longer accepts. Keep descriptions on the same tags
short, as in [slug.js](../examples/src/slug.js).

## Explain every suppression

Give every lint or type suppression a reason on the same line or the
line above:

```ts
// eslint-disable-next-line no-console -- the CLI reports to stdout
```

ESLint reads the text after `--` as the reason. Name the rule; never
disable all rules for a line or a file. When a type error cannot be
fixed, use `@ts-expect-error` with a reason, never `@ts-ignore` or
`@ts-nocheck`. Unlike a cast, `@ts-expect-error` fails once the error it
hides goes away.
