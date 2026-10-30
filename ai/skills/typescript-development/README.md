# TypeScript Development Skill

This bundle guides planning, writing, reviewing, refactoring, and
testing TypeScript and JavaScript code. `SKILL.md` says when and how
agents use it. One skill covers both languages, because plain JavaScript
checked through JSDoc uses the same compiler and the same rules.

## Version notes

Check `engines`, the CI matrix, the TypeScript version in the lockfile,
and the runtime the code ships to before using a feature from the notes
below. A newer local tool does not let a project raise its minimum
versions.

### Node 20.19 and 22.12

These versions can `require()` an ES module, as long as neither it nor
anything it imports uses top-level `await`. See [Node's modules
documentation](https://nodejs.org/api/modules.html#loading-ecmascript-modules-using-require).

### Node 22.18

Node runs `.ts` files without flags by stripping types, and rejects
syntax that needs code generation. See [Node's TypeScript
documentation](https://nodejs.org/api/typescript.html).

### Node 24

Node 24 runs `using` and `await using` declarations, which release
resources at the end of a block. Keep `try...finally` in code that must
run on older versions.

### TypeScript 6.0

TypeScript 6.0 turned on `strict` by default, set `types` to `[]` and
`rootDir` to the config's folder by default, and deprecated `baseUrl`,
`moduleResolution` `node10` and `classic`, `target: es5`, and AMD, UMD,
and SystemJS output. See the [TypeScript 6.0
announcement](https://devblogs.microsoft.com/typescript/announcing-typescript-6-0/).

### TypeScript 7.0

TypeScript 7.0 is a native compiler. Code that compiles cleanly with
TypeScript 6.0, with `stableTypeOrdering` on and no
`ignoreDeprecations`, should compile the same way, except JavaScript
checked through JSDoc. TypeScript 7.0 turns 6.0's deprecations into
errors, checks JSDoc in JavaScript files more like TypeScript files,
infers template literal types by Unicode code point, and ships no
programmatic API. Tools that import `typescript`, such as
typescript-eslint, therefore need TypeScript 6 installed beside it;
[Tooling](references/tooling.md#type-check-with-tsc) shows how. See the
[TypeScript 7.0
announcement](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/).

### ESLint 10

ESLint 10 removed `.eslintrc` files; only flat config files such as
`eslint.config.js` work. Do not migrate an older ESLint setup inside an
unrelated task. See the [ESLint 10 migration
guide](https://eslint.org/docs/latest/use/migrate-to-10.0.0).

## Sources and licensing

Sources were checked on October 28, 2026. Official TypeScript, Node, and
MDN documentation settle behavior; the project's own conventions settle
style. See [sources](references/sources.md) for each source's role and
license.

[references/google-ts-guide.md](references/google-ts-guide.md) is a
condensed and modified adaptation of the [Google TypeScript Style
Guide](https://google.github.io/styleguide/tsguide.html) by Google,
licensed under CC BY 3.0. The license text is in
[CC-BY-3.0.txt](CC-BY-3.0.txt). The adaptation lists where this skill
differs from the guide, and Google does not endorse it.

The rest of the prose and all example code were written for this skill.
No other source text is quoted.

## Maintenance

Update the condensed Google guide section by section and keep its
attribution and modification notice. Recheck the version notes, the
TypeScript 6 and 7 side-by-side setup, and typescript-eslint's supported
TypeScript range when a new TypeScript or ESLint major version ships.
Keep `SKILL.md` short and move details into references that `SKILL.md`
links directly. After a material change, run the example checks and the
cases in [evaluation](references/evaluation.md).
