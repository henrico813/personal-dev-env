# Tooling

Read this before running, adding, or changing the type checker, linter,
or formatter, or before adding a lint or type suppression.

## Contents

- [Run the project's tools](#run-the-projects-tools)
- [Type check with tsc](#type-check-with-tsc)
- [Lint with ESLint and
  typescript-eslint](#lint-with-eslint-and-typescript-eslint)
- [Use one formatter](#use-one-formatter)
- [Keep checks to the change](#keep-checks-to-the-change)

## Run the project's tools

Use the project's scripts first, such as `npm run lint`, `npm run
typecheck`, and `npm test`, because they carry the project's flags. Read
`scripts` in `package.json` and the CI workflow to find them.

To run a tool the project installs but has no script for, use `npx --no
-- <tool>`. Without `--no`, `npx` downloads a missing package from the
registry, and some command names belong to unrelated packages; `npx tsc`
without TypeScript installed fetches a package that is not the
TypeScript compiler. Write the `--`, because `npx` reads the word after
`--no` as its value. Without it, npm also takes the tool's flags, such
as `--check`, as its own, so `npx --no prettier --check file.ts` runs
Prettier without `--check`, prints the formatted file, and exits 0.
Never install tools globally without the user's permission.

## Type check with tsc

Run `tsc --noEmit -p <tsconfig>` for each `tsconfig` the project checks,
or `tsc -b` for a project that uses project references. Type stripping,
bundlers, and Vitest do not check types, so passing tests do not mean
the types are right.

Code that compiles cleanly with TypeScript 6, with `stableTypeOrdering`
on and no `ignoreDeprecations`, should compile the same way under
TypeScript 7. JavaScript checked through JSDoc is the exception, because
TypeScript 7 checks JSDoc more strictly. TypeScript 7 also infers
template literal types by Unicode code point, so an upgrade can show new
errors.

TypeScript 7.0 has no programmatic API. Tools that import the
`typescript` package, including typescript-eslint, therefore need
TypeScript 6 installed under that name. Check the `typescript` range in
typescript-eslint's `peerDependencies`. When the project's TypeScript is
outside that range, install the two side by side the way the TypeScript
team describes, so `tsc` is the new compiler and tools still import
TypeScript 6:

```json
{
  "devDependencies": {
    "@typescript/native": "npm:typescript@^7.0.2",
    "typescript": "npm:@typescript/typescript6@^6.0.2"
  }
}
```

Scripts can then call `tsc` for TypeScript 7 and `tsc6` for the older
compiler. The `typecheck` script in [the example
package](../examples/package.json) runs both. When typescript-eslint's
range includes the project's TypeScript, go back to one plain
`typescript` entry.

## Lint with ESLint and typescript-eslint

ESLint reads only flat config files, such as `eslint.config.js`; ESLint
10 removed support for `.eslintrc` files. For TypeScript, use
typescript-eslint's `recommendedTypeChecked` config with
`projectService: true`. The type-checked rules catch bugs plain rules
cannot, such as unhandled promises and unsafe use of `any`, at the cost
of a slower run:

```js
// @ts-check
import js from "@eslint/js";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";

export default defineConfig(
  { ignores: ["dist/"] },
  {
    files: ["**/*.{js,ts}"],
    extends: [js.configs.recommended, tseslint.configs.recommendedTypeChecked],
    languageOptions: { parserOptions: { projectService: true } },
  },
);
```

A project that uses `node:test` needs this `no-floating-promises` entry
for `describe` and `test`. [Testing][runner] explains why:

```js
"@typescript-eslint/no-floating-promises": [
  "error",
  {
    allowForKnownSafeCalls: [
      {
        from: "package",
        name: ["describe", "test"],
        package: "node:test",
      },
    ],
  },
],
```

A file that no `tsconfig` includes, such as a config file, needs
`projectService.allowDefaultProject` or an entry in a `tsconfig`. Use
`strictTypeChecked` only when the project chooses it.

[runner]: testing.md#pick-the-runner

Biome lints and formats in one tool. Oxlint lints and can run beside
ESLint, with `eslint-plugin-oxlint` turning off the ESLint rules Oxlint
already covers. Keep either one only in a project that already uses it.
Do not add one to a new project, and do not add a second linter to a
project that has one.

For a project that follows Standard style, use
[`neostandard`](https://github.com/neostandard/neostandard). Version
0.14 ships flat config, supports ESLint `^9.22.0 || ^10.0.0`, and checks
TypeScript through its `ts` option.

Do not add these to a new project:

- `eslint-config-airbnb` 19 and `eslint-config-airbnb-base` 15, which
  use legacy config and need ESLint 7 or 8.
- `standard` 17, which runs ESLint 8, and `ts-standard`, which builds on
  it.
- `eslint-config-google`, which follows a JavaScript guide Google no
  longer updates.

A project that already uses one keeps it until it chooses to migrate. Do
not migrate it inside an unrelated task.

## Use one formatter

Let one formatter own whitespace, semicolons, quotes, and line breaks.
Prettier is the default. Keep Biome or Oxfmt only in a project that
already uses one. Two formatters on the same files undo each other's
output.

Do not use lint rules for formatting. ESLint and typescript-eslint turn
no formatting rules on in their recommended configs. If another shared
config turns some on, add `eslint-config-prettier` last to turn them
off. Do not run Prettier inside ESLint with `eslint-plugin-prettier` in
new setups; it makes linting slower.

When you set up linting for a new package with no formatter, add
Prettier with `format` and `format:check` scripts. When an existing
project has no formatter, match the surrounding code by hand and do not
add one inside an unrelated task.

## Keep checks to the change

In a project with existing lint errors, lint and format the files you
changed and fix only problems your change added. Run `prettier --check
<files>` before `--write`, and write only files the task touches. Report
existing failures separately from new ones.

Order the checks from fastest to slowest so early failures are cheap:
format, lint, type check, then tests.
