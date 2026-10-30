# Packages and modules

Read this before changing `package.json`, module format, import paths,
`tsconfig` module settings, builds, dependencies, lockfiles, or
publishing.

## Contents

- [Use ES modules](#use-es-modules)
- [Pick module resolution by
  runtime](#pick-module-resolution-by-runtime)
- [Write import paths the runtime
  accepts](#write-import-paths-the-runtime-accepts)
- [Declare entry points with
  exports](#declare-entry-points-with-exports)
- [Set supported runtimes](#set-supported-runtimes)
- [Install from the lockfile](#install-from-the-lockfile)
- [Add dependencies with care](#add-dependencies-with-care)
- [Check a package before
  publishing](#check-a-package-before-publishing)

## Use ES modules

Write new code as ES modules with `import` and `export`. Set `"type":
"module"` in `package.json` so Node treats `.js` files as ES modules;
`.mjs` files are always ES modules and `.cjs` files are always CommonJS,
whatever `type` says. Set `type` even when it is `"commonjs"`, so
readers and tools do not guess.

Publish ES modules only, unless some callers use `require()` on a
runtime that cannot load an ES module that way. Node 20.19, Node 22.12,
and later versions can `require()` an ES module, as long as neither that
module nor anything it imports uses top-level `await`. A package that
ships both formats can be loaded twice in one program, once in each
format, and then keeps two copies of its state.

Do not convert a CommonJS project to ES modules inside an unrelated
task.

## Pick module resolution by runtime

`moduleResolution` tells TypeScript how the runtime will find imports.
Choose it from what runs the code:

- Code that Node runs directly, and any library that others install:
  `"module": "nodenext"`, which sets `moduleResolution` to match.
  TypeScript then checks file extensions and `exports` the same way Node
  does. Code that passes this check also works in bundlers.
- An application that a bundler builds, such as Vite, esbuild, or
  webpack: `"module": "preserve"` with `"moduleResolution": "bundler"`.
  It allows imports without extensions because the bundler resolves
  them.

TypeScript 7 rejects `moduleResolution` `node`, `node10`, and `classic`,
`baseUrl`, `target: es5`, and AMD, UMD, and SystemJS output. Do not add
them; when an older config has them, move to the settings above as its
own change.

## Write import paths the runtime accepts

Node ES modules need the file extension in relative imports. Under
`nodenext`, either write the extension of the emitted file
(`./config.js` for `config.ts`), or, when Node runs the `.ts` source by
type stripping, write `./config.ts` and set
`rewriteRelativeImportExtensions` so `tsc` rewrites it in its output.

For code that Node runs from `.ts` files, the Node documentation
recommends these settings:

```json
{
  "compilerOptions": {
    "module": "nodenext",
    "target": "esnext",
    "rewriteRelativeImportExtensions": true,
    "erasableSyntaxOnly": true,
    "verbatimModuleSyntax": true,
    "noEmit": true
  }
}
```

Leave out `noEmit` when the package also builds `.js` files, and set
`target` to what the oldest supported runtime runs. Node ignores
`tsconfig.json`, so `paths` aliases work in `tsc` but fail when Node
runs the code. Use `package.json` subpath imports instead, which start
with `#` and work in Node, bundlers, and TypeScript:

```json
{
  "imports": {
    "#internal/*": "./src/internal/*"
  }
}
```

## Declare entry points with exports

List every public entry point in `exports`. Put the `types` condition
first and `default` last, because tools use the first condition that
matches:

```json
{
  "type": "module",
  "exports": {
    ".": {
      "types": "./dist/index.d.ts",
      "default": "./dist/index.js"
    },
    "./package.json": "./package.json"
  },
  "files": ["dist"]
}
```

`files` lists what `npm pack` and `npm publish` include, which keeps
tests and secrets out of the package. Publish compiled JavaScript with
declaration files. Node refuses to strip types from `.ts` files under
`node_modules`, so a package that ships only `.ts` source does not run
in Node. [The example package](../examples/package.json) uses this shape
and tests it with an import by package name.

## Set supported runtimes

Set `engines.node` to the oldest Node version the project supports. Test
on that version and on the newest LTS release, so code does not use APIs
the oldest runtime lacks and does not break on the newest. Browser code
follows the project's browser targets, often a `browserslist` entry.

## Install from the lockfile

Commit the lockfile and use the package manager it belongs to:
`package-lock.json` for npm, `pnpm-lock.yaml` for pnpm, `yarn.lock` for
Yarn, and `bun.lock` for Bun. Do not switch package managers inside a
task.

In CI, and locally when `node_modules` is missing, install with `npm
ci`, `pnpm install --frozen-lockfile`, `yarn install --immutable`, or
`bun install --frozen-lockfile`. These install exactly what the lockfile
records and fail when it disagrees with `package.json`, so every machine
tests the same dependencies. `npm ci` also deletes `node_modules` first,
runs install scripts, and removes linked packages, so do not run it
again before every check. `npm install` may rewrite the lockfile; use it
only to change dependencies on purpose. Review lockfile diffs and never
edit a lockfile by hand.

## Add dependencies with care

Add a dependency only when the task needs it and the platform does not
already cover it. Node and browsers include `fetch`, `AbortController`,
`structuredClone`, `URL`, `crypto.randomUUID()`, and Node includes
`node:test` and `util.parseArgs`. Put build and test tools in
`devDependencies`.

Before adding a package, check its maintenance, license, size, and
whether it runs install scripts. Install scripts run with your user's
permissions on every install. Watch for names that imitate popular
packages. Treat `npm audit` output as a list to triage, not a reason to
upgrade everything; report findings that affect the change.

## Check a package before publishing

Run `npm pack --dry-run` to see what would ship. Run `publint` to
check `package.json` fields against the files, and `attw --pack` from
Are the Types Wrong to check that types resolve under each module
setting a caller might use. For an ES-module-only package, run `attw
--pack --profile esm-only`; the default profile also checks CommonJS
`node10` resolution, which an ES-module-only package fails on purpose.
Do not add a CommonJS build only to pass that check.
Delete the `.tgz` and any scratch folders after a real `npm pack`,
such as one for `attw --pack`, so they do not end up in the diff.

Publish only when the user asks. Prefer npm trusted publishing from CI,
which uses short-lived credentials instead of a stored token and adds a
provenance statement linking the package to its source and build. Never
print or commit npm tokens.
