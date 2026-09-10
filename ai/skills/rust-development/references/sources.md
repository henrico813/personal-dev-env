# Sources and how to use them

This skill selects and adapts guidance for maintainable Rust, with plain-English
explanations for a maintainer who is learning the language. Accessibility changes
how the guidance is explained, not whether standard Rust topics are covered.
It does not reproduce every rule or treat different projects' preferences as
universal requirements. Links are supporting references, not instructions to
fetch every guide on every task.

## Contents

- [Primary Rust guidance](#primary-rust-guidance)
- [Testing guidance from other languages](#testing-guidance-from-other-languages)
- [Rust language, conventions, and tools](#rust-language-conventions-and-tools)
- [Sources for the required topic checklists](#sources-for-the-required-topic-checklists)
- [Excerpts and writing references](#excerpts-and-writing-references)
- [Skill-authoring references](#skill-authoring-references)

## Primary Rust guidance

[Microsoft's Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/agents/all.txt)
are the primary reference. Consult the relevant rule for API design,
documentation, errors, safety, or testing. The linked file contains the combined
guide and its rule identifiers.

For this skill, Microsoft's specific choices about error structs, mock
controllers, shared service handles, and application allocators are options to
evaluate, not mandatory scaffolding. Choose them when they solve the task's
actual problem. This qualification is this skill's policy, not Microsoft's.

[rust-analyzer's style guide](https://rust-analyzer.github.io/book/contributing/style.html)
is the first supplement for implementation readability. Apply its preference
for clear control flow and functions instead of unnecessary action objects.
Keep its project context in mind: its advice about public fields, setters,
`Default`, and testing is not automatically suitable for a public library.

[How to Test, by matklad](https://matklad.github.io/2021/05/31/how-to-test.html)
is the first supplement for testing decisions. Use observable behavior as the
target and keep computation separate from I/O where practical. Small helpers
can reduce test-writing effort, but should not hide the scenario being tested.

[Effective Rust](https://effective-rust.com/) explains harder tradeoffs.
Consult [Item 4](https://effective-rust.com/errors.html) for error design,
[Item 11](https://effective-rust.com/raii.html) for cleanup and resource ownership,
and [Item 12](https://effective-rust.com/generics.html) for generics versus
trait objects, [Item 20](https://effective-rust.com/optimize.html) for optimization,
[Item 22](https://effective-rust.com/visibility.html) for visibility, and
[Item 30](https://effective-rust.com/testing.html) for complementary kinds of tests.

## Testing guidance from other languages

[Software Engineering at Google, Chapter 12](https://abseil.io/resources/swe-book/html/ch12.html)
supports behavior-focused tests, using the API consumers use, asserting outcomes
rather than incidental interactions, and keeping relevant setup visible.
DAMP means Descriptive And Meaningful Phrases: favor clarity over eliminating
all repetition. It is not a mandated test-name template or word limit.

[Uber Go Style Guide: Test Tables](https://github.com/uber-go/guide/blob/master/style.md#test-tables)
supports grouping comparable input/output cases and splitting tables whose
setup or assertions require branching. Adapt these ideas to Rust. Do not copy
Go naming conventions, `testing.T`, subtests, or language-specific workarounds.

The preference for test names of seven words or fewer, the minimal-change
workflow, and the emphasis on brief explanations for a junior maintainer are
skill-specific choices. A test's clarity and defect-detection value matter more
than a lower test count or an arbitrary coverage percentage.

## Rust language, conventions, and tools

Use official Rust documentation to resolve language and tool behavior:

| Topic | Reference |
| --- | --- |
| Naming and conversions | [Rust API Guidelines: Naming](https://rust-lang.github.io/api-guidelines/naming.html) |
| API docs and examples | [Rust API Guidelines: Documentation](https://rust-lang.github.io/api-guidelines/documentation.html) |
| Methods and ownership of `self` | [The Rust Book: Methods](https://doc.rust-lang.org/book/ch05-03-method-syntax.html) |
| Borrowing | [The Rust Book: References and Borrowing](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html) |
| Traits | [The Rust Book: Traits](https://doc.rust-lang.org/book/ch10-02-traits.html) |
| Expressions and return values | [The Rust Book: Functions](https://doc.rust-lang.org/book/ch03-03-how-functions-work.html) |
| Recoverable errors and `?` | [The Rust Book: Result](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html) |
| Test locations and visibility | [The Rust Book: Test Organization](https://doc.rust-lang.org/book/ch11-03-test-organization.html) |
| File layout | [The Cargo Book: Package Layout](https://doc.rust-lang.org/cargo/guide/project-layout.html) |
| Formatting | [The Rust Style Guide](https://doc.rust-lang.org/style-guide/) |
| Lints and exceptions | [Clippy: Usage](https://doc.rust-lang.org/clippy/usage.html) |
| Test selection and doctests | [Cargo: cargo test](https://doc.rust-lang.org/cargo/commands/cargo-test.html) |

## Sources for the required topic checklists

`SKILL.md` states when each local checklist must be read. Those checklists contain
working guidance and topic-specific source links; do not replace reading them
with a vague instruction to follow best practices.

| Checklist | Main additional authorities |
| --- | --- |
| API design | [API interoperability](https://rust-lang.github.io/api-guidelines/interoperability.html), [type safety](https://rust-lang.github.io/api-guidelines/type-safety.html), [standard conversions](https://doc.rust-lang.org/std/convert/index.html), and [Cargo compatibility](https://doc.rust-lang.org/cargo/reference/semver.html) |
| Errors and resources | [`Error`](https://doc.rust-lang.org/std/error/trait.Error.html), [`Drop`](https://doc.rust-lang.org/std/ops/trait.Drop.html), and [`BufWriter`](https://doc.rust-lang.org/std/io/struct.BufWriter.html) |
| Cargo | [Features](https://doc.rust-lang.org/cargo/reference/features.html), [workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html), [MSRV](https://doc.rust-lang.org/cargo/reference/rust-version.html), and [lockfiles](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html) |
| Concurrency | [Tokio shared state](https://tokio.rs/tokio/tutorial/shared-state), [task ownership](https://tokio.rs/tokio/tutorial/spawning), [cancellation](https://tokio.rs/tokio/tutorial/select), and [`std::cell`](https://doc.rust-lang.org/std/cell/index.html) |
| Unsafe and FFI | [Rustonomicon](https://doc.rust-lang.org/nomicon/), [FFI](https://doc.rust-lang.org/nomicon/ffi.html), and [Miri](https://github.com/rust-lang/miri) |

Tokio is a reference for async behavior, not a required dependency. Consult the
actual runtime's documentation when the project uses something else. These
checklists do not justify introducing concurrency, unsafe code, or dependencies
into a task that does not need them.

Check documentation against the repository's supported toolchain when syntax,
features, or command flags might differ. A newer guide is not permission to
raise the project's minimum Rust version.

## Excerpts and writing references

Short quotations appear next to the decisions they support, with attribution
and a link to the original page. The surrounding explanation and Rust examples
were written for this skill. They are not presented as the source author's code
or as universal rules followed by every Rust project.

The quotations come from Microsoft's guidance on public types, rust-analyzer's
advice about action-only objects, matklad's testing article, Effective Rust's
error discussion, and the Rust API Guidelines on newtypes and examples. Their
wording was checked against the linked sources for this revision.

The prose uses the supplied [Uber guide](https://github.com/uber-go/guide/blob/master/style.md#test-tables)
and [Google testing chapter](https://abseil.io/resources/swe-book/html/ch12.html)
as writing models: give a recommendation, show the relevant code, explain why
it works, and state when another choice fits. The supplied
[Stack Overflow discussion](https://stackoverflow.com/questions/40091142/is-there-an-established-pattern-name-for-golang-code-which-seems-similar-to-a-mi)
is another model for explaining a design through changing requirements. Its
Go embedding and method-promotion behavior is not a Rust rule.

The educational references are [design choices](design-choices.md),
[ownership](ownership.md), [testing](testing.md), and
[documentation](documentation.md). The existing API, error, Cargo, concurrency,
and unsafe references also include explanations and examples. Read the relevant
section rather than loading them all for an unrelated task.

Some long-lived books describe older compiler behavior. Use current official
documentation for facts about the target toolchain, while keeping the project's
supported versions unchanged unless the task explicitly calls for an upgrade.

## Skill-authoring references

[Anthropic's Complete Guide to Building Skills](https://resources.anthropic.com/hubfs/The-Complete-Guide-to-Building-Skill-for-Claude.pdf)
informs the folder structure, YAML metadata, task triggers, and evaluation cases.

[Anthropic's skill-authoring best practices](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices)
informs the navigable main instructions, directly linked supporting files, and
loading detailed material only when needed.

[OpenAI Academy: Using skills](https://openai.com/academy/skills/)
informs the repeatable workflow: identify inputs, perform the task, check the
result, and provide a useful handoff.

These references guide the skill's packaging and behavior. They are not Rust
language authorities and need not be consulted during ordinary Rust changes.
