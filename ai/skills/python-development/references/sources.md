# Sources and how to use them

This skill is an independent synthesis for maintainable Python work. Links
support specific decisions; they are not instructions to browse every source on
every task.

## Contents

- [Source hierarchy](#source-hierarchy)
- [Primary style and documentation sources](#primary-style-and-documentation-sources)
- [Design and runtime sources](#design-and-runtime-sources)
- [Typing, testing, and packaging sources](#typing-testing-and-packaging-sources)
- [Quotations and review status](#quotations-and-review-status)
- [Skill-authoring references](#skill-authoring-references)

## Source hierarchy

PEP 8 is the primary style guide. Official Python and pytest documentation
settle language, standard-library, typing, mock, and pytest behavior. Repository
instructions, supported versions, configured tools, and surrounding conventions
settle style and workflow choices.

When a source describes its own project, keep that context. Django and Google
guidance can inform a decision without becoming a universal repository rule.
Current official documentation wins for behavior that changed across Python
versions.

## Primary style and documentation sources

[PEP 8](https://peps.python.org/pep-0008/) supplies the complete condensed style
reference and supports exception chaining and resource-cleanup guidance.
[PEP 257](https://peps.python.org/pep-0257/) supplies the condensed docstring
mechanics. Each PEP states, “This document has been placed in the public
domain.”

[Google's Python Style Guide](https://google.github.io/styleguide/pyguide.html)
supports focused guidance on exceptions, mutable globals, decorators,
annotations, documentation, resources, and power features. It is a supporting
source, not the primary style guide. The guide is CC BY 3.0.

[Django coding style](https://docs.djangoproject.com/en/dev/internals/contributing/writing-code/coding-style/)
supports following surrounding style and avoiding unrelated refactors. Django
documentation is BSD-3-Clause.

## Design and runtime sources

[Django design philosophies](https://docs.djangoproject.com/en/dev/misc/design-philosophies/#loose-coupling)
support loose coupling, cohesion, and explicit behavior.
[Cosmic Python Ch. 3](https://www.cosmicpython.com/book/chapter_03_abstractions.html#implementing-our-chosen-abstraction)
supports separating decisions from stateful effects without requiring classes.
Its CC BY-NC-ND 4.0 terms permit the short unchanged quotation used here, not an
adaptation of the chapter.

[Brandon Rhodes on composition](https://python-patterns.guide/gang-of-four/composition-over-inheritance/)
supports composition where behaviors vary independently; the source repository
uses the MIT license. The
[functional programming HOWTO](https://docs.python.org/3/howto/functional.html)
supports functional decomposition and incremental iterator processing. The
technique-selection table remains this skill's policy.

Official Python documentation supplies behavior for
[tasks](https://docs.python.org/3/library/asyncio-task.html),
[bounded queues](https://docs.python.org/3/library/asyncio-queue.html), and
[executors](https://docs.python.org/3/library/concurrent.futures.html). Python
documentation uses the PSF License v2; code examples are additionally available
under Zero-Clause BSD terms.

## Typing, testing, and packaging sources

[Typing protocol documentation](https://typing.python.org/en/latest/reference/protocols.html)
supports structural subtyping and qualifies runtime protocol checks. It uses the
PSF License v2, with code additionally under Zero-Clause BSD terms.

[pytest parameterization](https://docs.pytest.org/en/stable/how-to/parametrize.html),
[fixtures](https://docs.pytest.org/en/stable/how-to/fixtures.html#handling-errors-for-yield-fixture),
[unittest interoperability](https://docs.pytest.org/en/stable/how-to/unittest.html),
and [plugin testing](https://docs.pytest.org/en/stable/how-to/writing_plugins.html#testing-plugins)
supply pytest mechanics. pytest documentation uses the MIT license.

[Python mock documentation](https://docs.python.org/3/library/unittest.mock.html#where-to-patch)
settles lookup-site patching. [PythonSpeed's verified-fakes article](https://pythonspeed.com/articles/verified-fakes/#verified-fakes-testing-both-the-real-and-fake-implementation)
supports running one shared behavior suite against real and fake implementations;
no explicit article-text license was located, so its quotation stays short and
attributed.

[PyPA's layout discussion](https://packaging.python.org/en/latest/discussions/src-layout-vs-flat-layout/)
supports the limited claim that a source layout helps prevent accidental use of
the development copy. PyPA documentation uses the PSF License v2; examples are
additionally Zero-Clause BSD. [Sphinx doctest documentation](https://www.sphinx-doc.org/en/master/usage/extensions/doctest.html)
supports executable marked snippets and uses BSD-2-Clause terms.

## Quotations and review status

Short exact quotations appear beside the decisions they support. Preserve their
wording, attribution, and links. Surrounding summaries and selection rules are
this skill's policy. Cosmic Python's NoDerivatives license requires its excerpt
to remain unchanged; no source code was copied from it.

The supplied starting draft was generated material, not a verified authority.
Every retained attributed claim and quotation was checked against inspectable
source text on September 30, 2026. Unverified commercial-book chapter claims are
not used. The test-comment rule in [documentation](documentation.md) and the
technique table in [design choices](design-choices.md) remain this skill's
policy.

## Skill-authoring references

[Anthropic's Complete Guide to Building Skills](https://resources.anthropic.com/hubfs/The-Complete-Guide-to-Building-Skill-for-Claude.pdf)
informs folder structure, frontmatter, composability, and activation and
execution evaluation. [Anthropic's skill-authoring guidance](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices)
informs concise main instructions, conditional references, direct links,
examples, and comparison with a no-skill baseline. [OpenAI Academy's skills
guidance](https://openai.com/academy/skills/) supports defining inputs, steps,
output format, guardrails, and final checks. No open content license was located
for these materials, so this skill summarizes them independently and does not
reproduce substantial prose.
