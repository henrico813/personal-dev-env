# TypeScript and JavaScript examples

This package holds the worked examples the skill links to. It runs on
Node 24 or later and has no runtime dependencies. Keep using your own
project's runtime, tools, and layout; this package does not prescribe
them, and it is not a starter template.

## Read an example

Start with the [example index](../references/examples.md), which links
each question to its explanation and source. The modules are separate
lessons:

- `src/config.ts` checks untrusted data without a library and keeps
  error causes.
- `src/fetch-json.ts` combines a caller's `AbortSignal` with a timeout.
- `src/slug.js` is plain JavaScript checked through JSDoc types.
- `src/index.ts` is the one public entry point that `package.json`
  `exports`.

## Check the code

From this directory, run:

```sh
npm ci
npm run format:check
npm run typecheck
npm run lint
npm test
npm run check:package
```

`npm ci` installs exactly what `package-lock.json` records; skip it when
`node_modules` is already installed. `npm run typecheck` runs TypeScript
7 (`tsc`) and then TypeScript 6 (`tsc6`), which typescript-eslint uses;
[Tooling](../references/tooling.md#type-check-with-tsc) explains the
two. `npm test` builds `dist/` first, because one test imports the
package by name through its `exports`, then runs `node --test`, which
runs the `.ts` tests through Node's type stripping. `npm run
check:package` runs publint on the packed package.

## Supported Node versions

`engines` asks for Node 24 or later. Check on the oldest supported
version and on the newest LTS release. To check another version without
installing it, download its Linux binary from nodejs.org, put its `bin`
directory first on `PATH`, and run all the commands above again.

## Validation status

These commands passed on Node 24.16.0 and Node 26.13.0 with the locked
dependencies. Agent loading and the cases in
[evaluation](../references/evaluation.md) have not been run.
