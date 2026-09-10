# Worked examples

Start with the decision you need to understand. The full source lives in
[the example package](../examples/README.md), with no third-party dependencies.
The topics are independent lessons, not modules every application must contain.

| Question | Explanation | Full source |
| --- | --- | --- |
| Does this operation need an object? | [Start with a function](design-choices.md#start-with-a-function) | [Retry parser](../examples/src/retry.rs) |
| Where do data and methods belong? | [Structs and composition](design-choices.md#use-a-struct-for-data-that-belongs-together) | [Reports](../examples/src/report.rs) |
| Should a function borrow, own, or clone? | [Ownership](ownership.md) | [Report methods and clone test](../examples/src/report.rs) |
| How do I prevent contradictory states? | [Enum alternatives](design-choices.md#put-alternatives-in-an-enum) | [Device connections](../examples/src/device.rs) |
| When does a custom trait help? | [Shared behavior](design-choices.md#add-a-trait-when-a-caller-needs-shared-behavior) | [Summary implementations](../examples/src/report.rs) |
| Should I use generics or a trait object? | [Implementation selection](design-choices.md#choose-how-the-caller-supplies-an-implementation) | [Single and mixed summaries](../examples/src/report.rs) |
| Why use a newtype rather than an alias? | [Checked identifiers](api-design.md#example-a-checked-user-identifier) | [User IDs](../examples/src/identity.rs) |
| How do callers distinguish failures? | [Errors and causes](errors-resources.md#example-keep-the-underlying-error) | [Reading a retry count](../examples/src/retry.rs) |
| Why is dropping a writer not enough? | [Fallible completion](errors-resources.md#example-return-a-flush-failure) | [Buffered output](../examples/src/buffered_output.rs) |
| What belongs in API docs? | [Behavior and usage examples](documentation.md) | [Parser doctest](../examples/src/retry.rs) |
| What should tests assert? | [Behavior, tables, and failure substitutes](testing.md) | [Local tests](../examples/src/report.rs), [CLI tests](../examples/tests/cli.rs) |
| Can threads borrow without an Arc? | [Scoped work](concurrency.md#borrow-with-scoped-work-when-it-fits) | [Joined threads](../examples/src/threaded.rs) |
| What makes an unsafe boundary reviewable? | [A foreign byte buffer](unsafe.md#example-copy-a-foreign-buffer) | [Safety requirements and tests](../examples/src/foreign.rs) |

Some explanatory snippets omit surrounding imports, types, or documentation to
focus on one choice. They are labeled as excerpts or fragments and link to their
context. `first_line` in the ownership guide is a complete standalone function.
The async lock example is a fragment, not a complete async application.

Quoted prose in the guides is attributed to its source. The Rust examples were
written for this skill; they are not presented as code copied from those guides.
See [sources](sources.md) for the reference hierarchy and
[evaluation](evaluation.md) for package checks and proposed agent tasks.
