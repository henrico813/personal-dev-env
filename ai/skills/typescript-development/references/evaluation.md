# Evaluate this skill

Use these cases when installing or revising the skill, not during
ordinary JavaScript or TypeScript work. Run each case in the intended
agent and inspect whether the skill loaded, which references it read,
the actual diff, and the checks it ran. The cases are proposals until
someone runs them.

## Run each case twice

Set the model once: `EVAL_MODEL="${EVAL_MODEL:-goog/qwen3.8}"`. Run
every case once with the skill installed and once without it, the
no-skill baseline, in a fresh session each time. Record the harness,
exact model and version, whether the skill loaded, the references read,
the result, and how it differs from the baseline. Record an unavailable
model as not run; do not swap in another.

## Everyday behavior

| Request | Expected behavior |
| --- | --- |
| Add a `retries` option to this TypeScript HTTP client. | Reads the existing API and tests, adds an options field with a default, accepts no `any`, and keeps the caller's `AbortSignal` working. |
| Parse this webhook body in our Node service. | Types the body as `unknown`, checks it at the boundary with the project's schema library or a hand-written check, and returns a typed value. |
| Fix this `tsc` error in `user.ts`. | Finds the real type problem and fixes it without `as any`, `!`, or `@ts-ignore`. |
| Add tests for this `fetch` wrapper. | Uses the project's test runner, groups tests in a `describe` block, tests against a local server or the project's fake, covers error status and timeout, and does not test a pass-through. |
| Add a status field with values draft, live, and archived. | Uses a union of literals or an `as const` object, not an `enum`, unless the project already uses enums for this. |
| Review this JavaScript module that uses JSDoc types. | Treats it as in scope, checks JSDoc types against TypeScript syntax, and separates defects from preferences. |
| Set up linting for this new TypeScript package. | Adds ESLint flat config with `recommendedTypeChecked` and Prettier, checks typescript-eslint's TypeScript range, and adds no second formatter. |

## Reference-loading checks

| Request | Must read | Expected behavior |
| --- | --- | --- |
| Publish this library as ESM with types. | Packages, API design | Declares `exports` with `types` first, sets `files`, runs `npm pack --dry-run` and publint, and does not publish. |
| This loop fetches 20,000 URLs with `Promise.all`. Review it. | Concurrency | Flags the unbounded start, proposes a fixed worker count, and keeps cancellation. |
| Wrap this library's errors for our callers. | Errors and resources, API design | Keeps each original error as `cause`, gives callers a stable `name` or `code`, and does not catch at every layer. |
| Convert this file's `enum` to something Node can run directly. | Typing | Replaces it with an `as const` object and a derived union, and keeps runtime values the same. |
| Why does `npx eslint` report floating promises on every `describe`? | Tooling, testing | Explains that `node:test` reports each failure itself and its `describe()` and `test()` promises never reject, and configures `allowForKnownSafeCalls` rather than turning the rule off. |

## Negative and constraint cases

| Request or condition | Expected behavior |
| --- | --- |
| Explain what a JavaScript closure is. | Does not load the skill for a basic language question. |
| Fix a typo in this README; change no code. | Does not load the skill because the repository has JavaScript. |
| Review this Go HTTP server. | Does not apply this skill. |
| The project uses Jest and `.eslintrc.cjs` with ESLint 8. | Keeps Jest and the existing config; does not migrate the test runner or the config inside the task. |
| The project uses Biome. | Runs Biome and does not add Prettier or ESLint. |
| `node_modules` is missing and there is no network. | Reports which checks could not run and does not claim they passed. |

For each positive case, check that the matching references were read
before the decision, that `behavior-focused-testing` and
`code-documentation` loaded when tests or comments changed, and that no
unrelated change appeared.

## Validate the example package

| Check | Expected result |
| --- | --- |
| Parse the `SKILL.md` frontmatter. | `name` and `description` load; the description is under 1,024 characters. |
| Check every relative link and heading fragment. | Every link resolves. |
| Run the commands in [the example README](../examples/README.md). | Formatting, both compilers, linting, tests, and publint pass. |
| Run the tests on the oldest and newest supported Node versions. | Both pass. |

Passing these checks shows the example package works. It does not show
that agents load or follow the skill; only the cases above do.
