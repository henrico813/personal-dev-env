# Errors and resources

Read this before changing what code throws or rejects with, how it
catches errors, or how it cleans up files, connections, timers, and
listeners.

## Contents

- [Follow one error model](#follow-one-error-model)
- [Throw Error objects](#throw-error-objects)
- [Keep the cause](#keep-the-cause)
- [Catch narrowly and on purpose](#catch-narrowly-and-on-purpose)
- [Never leave a rejection
  unhandled](#never-leave-a-rejection-unhandled)
- [Clean up what you open](#clean-up-what-you-open)

## Follow one error model

Use the project's way of reporting failure. The common models are thrown
errors, error subclasses that callers check, and result objects such as
`{ ok: true, value } | { ok: false, error }`. A result object fits
expected, frequent failures such as form validation, because the type
forces callers to handle both cases. Thrown errors fit failures the
immediate caller usually cannot fix. Do not report the same failure both
ways in one module.

An `async` function turns every `throw` into a rejection, so callers
have one place to handle failure. A function that returns a promise
without `async` must not also throw before returning it.

## Throw Error objects

Create errors with `new Error()` or a subclass, so each one has a stack
trace. Make a subclass when callers need to tell a failure apart, and
give it a stable `name` and any fields callers need:

```ts
export class HttpError extends Error {
  override name = "HttpError";
  readonly status: number;

  constructor(url: string, status: number) {
    super(`GET ${url} failed with status ${status}`);
    this.status = status;
  }
}
```

An error class meant for subclassing declares `name` without `readonly`.
A subclass that sets its own `name` would otherwise fail to compile.

`instanceof` fails when an error comes from another copy of the package
or another realm, such as a worker or an iframe. In that code, check
`name` or a `code` property instead. Node's own errors carry a `code`
such as `ENOENT`.

## Keep the cause

When translating a low-level error into one callers understand, pass the
original as `cause`. The caller gets your message and can still inspect
what went wrong underneath:

```ts
try {
  data = JSON.parse(text);
} catch (error) {
  throw new ConfigError(`Config ${path} is not valid JSON`, {
    cause: error,
  });
}
```

[config.ts](../examples/src/config.ts) does this for read and parse
failures, and its test checks the cause. Add context such as a file path
or an ID to the message; never add secrets or full request bodies.

## Catch narrowly and on purpose

Catch an error only where code can recover, add context, or translate
it. Keep the `try` block to the call that can fail, so the handler does
not absorb an unrelated bug. Do not catch, log, and rethrow at every
layer; handle each failure once.

`catch` variables are `unknown` in strict mode. Narrow before reading
properties, for example with `error instanceof Error`. Do not write
defensive handling for thrown strings unless an API is known to throw
them, and say which one in a comment.

## Never leave a rejection unhandled

Every promise needs an owner: `await` it, `return` it, or pass it to
code that handles its rejection. Node stops the process on an unhandled
rejection by default. Use typescript-eslint's `no-floating-promises` and
`no-misused-promises` rules to find promises nobody owns, including
`async` callbacks passed where a plain function is expected.

Use `void promise` only when failure is handled elsewhere, and write
where.

## Clean up what you open

Use `try...finally` to release a resource on every path. Where the
runtime supports explicit resource management, as Node 24 and later do,
`using` and `await using` release an object with a `Symbol.dispose` or
`Symbol.asyncDispose` method at the end of the block, in reverse order:

```ts
await using file = await open(path, "r");
const text = await file.readFile("utf8");
```

Use `pipeline` from `node:stream/promises` to connect streams, so an
error in any stream closes the others. Read or cancel every `fetch`
response body; an unread body keeps its connection busy. Remove event
listeners you add, for example with a `signal` option that aborts when
the work ends; `{ once: true }` removes a listener only after its event
fires. Clear timers you start.

Libraries must not call `process.exit()`. Command-line programs should
set `process.exitCode` and let pending work finish.
