# Worked examples

Start with the decision you need to understand. The full source is in
[the example package](../examples/README.md). Each module is a separate
lesson, not a part every application needs.

| Question | Explanation | Full source |
| --- | --- | --- |
| How do I check untrusted data without a library? | [Check external data at runtime](typing.md#check-external-data-at-runtime) | [Config parsing](../examples/src/config.ts), [tests](../examples/test/config.test.ts) |
| How do I keep values without an enum? | [Write erasable syntax](typing.md#write-erasable-syntax) | [Log levels](../examples/src/config.ts) |
| Where do rules end and file access begin? | [Separate decisions from effects](design-choices.md#separate-decisions-from-effects) | [`parseConfig` and `loadConfig`](../examples/src/config.ts) |
| How does an error keep its cause? | [Keep the cause](errors-resources.md#keep-the-cause) | [`loadConfig`](../examples/src/config.ts), [tests](../examples/test/config.test.ts) |
| How do a timeout and a caller's cancel work together? | [Cancel with AbortSignal](concurrency.md#cancel-with-abortsignal) | [`fetchJson`](../examples/src/fetch-json.ts), [tests](../examples/test/fetch-json.test.ts) |
| How do I test an HTTP client without mocking `fetch`? | [Prefer real parts to mocks](testing.md#prefer-real-parts-to-mocks) | [Local server tests](../examples/test/fetch-json.test.ts) |
| How do I type plain JavaScript? | [Type plain JavaScript with JSDoc](typing.md#type-plain-javascript-with-jsdoc) | [Slugs](../examples/src/slug.js), [tests](../examples/test/slug.test.ts) |
| How should `package.json` declare entry points? | [Declare entry points with exports](packages.md#declare-entry-points-with-exports) | [`package.json`](../examples/package.json), [entry module](../examples/src/index.ts), [tests](../examples/test/package.test.ts) |
| How is `tsconfig` set up for type stripping and builds? | [Write import paths the runtime accepts](packages.md#write-import-paths-the-runtime-accepts) | [`tsconfig.json`](../examples/tsconfig.json), [`tsconfig.build.json`](../examples/tsconfig.build.json) |
| How is type-checked linting set up? | [Lint with ESLint and typescript-eslint](tooling.md#lint-with-eslint-and-typescript-eslint) | [`eslint.config.js`](../examples/eslint.config.js) |

The example code and prose were written for this skill. They are not
copied from the sources in [sources](sources.md). See
[evaluation](evaluation.md) for the package checks.
