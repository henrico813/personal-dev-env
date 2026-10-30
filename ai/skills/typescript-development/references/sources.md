# Sources and how to use them

This skill is an independent summary for maintainable JavaScript and
TypeScript work. The links support specific decisions; they are not a
reading list for every task.

## Contents

- [Which source wins](#which-source-wins)
- [Language and compiler](#language-and-compiler)
- [Runtime, modules, and packages](#runtime-modules-and-packages)
- [Style, linting, and formatting](#style-linting-and-formatting)
- [Testing](#testing)
- [Projects studied](#projects-studied)
- [Other style guides](#other-style-guides)
- [Sources not used as defaults](#sources-not-used-as-defaults)
- [Skill-authoring references](#skill-authoring-references)

## Which source wins

The project's own config, conventions, and framework docs come first.
Official TypeScript, Node, and MDN documentation settle how the
language, compiler, and runtime behave. The condensed Google guide
settles style questions that nothing else does. The rest support single
decisions.

## Language and compiler

The [TypeScript
Handbook](https://www.typescriptlang.org/docs/handbook/intro.html),
[TSConfig reference](https://www.typescriptlang.org/tsconfig/), and
[release
notes](https://www.typescriptlang.org/docs/handbook/release-notes/overview.html)
support the typing and `tsconfig` guidance. The TypeScript website's
documentation is CC BY 4.0 and its code is MIT; this skill summarizes it
and copies no text.

The [TypeScript
6.0](https://devblogs.microsoft.com/typescript/announcing-typescript-6-0/)
and [TypeScript
7.0](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/)
announcements support the notes on new defaults, removed options, JSDoc
changes, and running TypeScript 6 beside 7. No open license was found
for the blog, so the skill only summarizes it.

The [MDN JavaScript
Guide](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide),
including
[promises](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide/Using_promises),
[modules](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide/Modules),
and
[AbortController](https://developer.mozilla.org/en-US/docs/Web/API/AbortController),
supports the concurrency and module guidance. MDN prose is CC BY-SA 2.5,
so the skill summarizes it in its own words.

## Runtime, modules, and packages

Node's documentation on
[TypeScript](https://nodejs.org/api/typescript.html), [ES
modules](https://nodejs.org/api/esm.html),
[packages](https://nodejs.org/api/packages.html), and the [test
runner](https://nodejs.org/api/test.html) supports type stripping,
import paths, `exports`, subpath imports, and `node:test`. The
`tsconfig` block in [Packages](packages.md) lists the settings Node's
TypeScript page recommends.

[publint](https://publint.dev/) (MIT) and [Are the Types
Wrong](https://arethetypeswrong.github.io/) by Andrew Branch (MIT)
support the package checks. npm's documentation on [trusted
publishing](https://docs.npmjs.com/trusted-publishers) and
[provenance](https://docs.npmjs.com/generating-provenance-statements)
supports the publishing guidance. The [OWASP npm
Security](https://cheatsheetseries.owasp.org/cheatsheets/NPM_Security_Cheat_Sheet.html)
and [Node.js
Security](https://cheatsheetseries.owasp.org/cheatsheets/Nodejs_Security_Cheat_Sheet.html)
cheat sheets, CC BY-SA 4.0, support the dependency guidance and are
summarized, not copied.

## Style, linting, and formatting

[references/google-ts-guide.md](google-ts-guide.md) is a condensed,
modified adaptation of the [Google TypeScript Style
Guide](https://google.github.io/styleguide/tsguide.html), CC BY 3.0.
[CC-BY-3.0.txt](../CC-BY-3.0.txt) holds the license text. The page lists
where this skill differs from the guide.

The [ESLint configuration
docs](https://eslint.org/docs/latest/use/configure/) (MIT),
typescript-eslint's [typed
linting](https://typescript-eslint.io/getting-started/typed-linting) and
[What About
Formatting?](https://typescript-eslint.io/users/what-about-formatting)
(MIT), and [Prettier's rationale](https://prettier.io/docs/rationale)
(MIT) support the tooling guidance. [Biome](https://biomejs.dev/) and
[Oxc](https://oxc.rs/) document the alternatives.

## Testing

Testing Library's [guiding
principles](https://testing-library.com/docs/guiding-principles) (MIT)
support the user-interface advice in [Testing](testing.md), which
restates them in its own words. The
[Vitest](https://vitest.dev/) (MIT),
[Playwright](https://playwright.dev/) (Apache-2.0), and `node:test` docs
supply runner mechanics. Yoni Goldberg's [JavaScript testing best
practices](https://github.com/goldbergyoni/javascript-testing-best-practices)
(MIT) supports testing through public behavior and preferring real
collaborators; it is summarized, not copied.

## Projects studied

These open-source projects were read for shared practices, and no code
was copied from them: Vite, Vitest, typescript-eslint, Prettier, Zod,
Svelte, Ky, and TypeScript. They share deliberate public entry points,
strict checking with each exception explained, checks at data
boundaries, one error model that keeps context, tests through public
APIs, comments for reasons only, and a reason on every lint suppression.
They differ on interfaces against type aliases, test placement, error
model, and module format, which is why the skill leaves those to the
project.

Ky shows one async error channel: it keeps any thrown value and rejects
instead of throwing synchronously, and it gives its one lint suppression
a reason. See its
[`source/index.ts`](https://github.com/sindresorhus/ky/blob/main/source/index.ts).

## Other style guides

The [Google JavaScript Style
Guide](https://google.github.io/styleguide/jsguide.html) (CC BY 3.0) is
history only. Its page says it is no longer updated and recommends
migrating to TypeScript, and `eslint-config-google` is archived. Its
Closure rules, such as `goog.module`, Closure JSDoc types, clang-format,
and the 80-column limit, are not used.

The [Airbnb JavaScript Style
Guide](https://github.com/airbnb/javascript) (MIT) is used in part. Its
rules on default parameters and on returning objects instead of arrays
are summarized in [API design](api-design.md) with links, and no text is
copied. Its lint configs, `eslint-config-airbnb` 19.0.4 and
`eslint-config-airbnb-base` 15.0.0, use legacy config and need ESLint 7
or 8. It has no official TypeScript guide, and
`eslint-config-airbnb-typescript` is archived. Its advice against import
extensions breaks native ES modules, and its advice against generators
and iterators served ES5 output.

[Standard](https://standardjs.com/) (MIT) is not a default. `standard`
17.1.2 runs ESLint 8, and `ts-standard` 12.0.2 was last published in
2023. [neostandard](https://github.com/neostandard/neostandard) (MIT)
continues Standard style with flat config; version 0.14.1 supports
ESLint `^9.22.0 || ^10.0.0` and TypeScript through a `ts` option.
Leaving out semicolons is Standard's preference, not a correctness rule.

[Design
choices](design-choices.md#follow-the-project-where-style-guides-disagree)
lists where these guides disagree.

## Sources not used as defaults

The Airbnb and Standard lint configs, `eslint-config-google`, and
TSLint, which is deprecated, are not defaults.
Advice built on `baseUrl`, `moduleResolution: node`, or ES5 output no
longer applies to TypeScript 7. *Effective TypeScript* and *Total
TypeScript* are useful but copyrighted; read them directly rather than
expecting their text here.

## Skill-authoring references

[Anthropic's skill-authoring best
practices](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices)
inform the short entry point, directly linked references, contents
lists, and evaluations with a no-skill baseline. [OpenAI Academy's
skills guidance](https://openai.com/academy/skills/) informs the inputs,
steps, output, guardrails, and final checks in `SKILL.md`. No open
license was found for either, so the skill follows them without copying
their text.
