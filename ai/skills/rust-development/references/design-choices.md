# Choose a design the caller can use

Read this when choosing between functions, structs, composition, enums, and
traits. The examples start with one requirement and add another only when the
first design no longer fits. They are alternatives, not steps every program
must go through.

## Contents

- [Start with a function](#start-with-a-function)
- [Use a struct for data that belongs together](#use-a-struct-for-data-that-belongs-together)
- [Compose types without hiding the call](#compose-types-without-hiding-the-call)
- [Put alternatives in an enum](#put-alternatives-in-an-enum)
- [Add a trait when a caller needs shared behavior](#add-a-trait-when-a-caller-needs-shared-behavior)
- [Choose how the caller supplies an implementation](#choose-how-the-caller-supplies-an-implementation)
- [Use a configuration struct or builder when it earns its place](#use-a-configuration-struct-or-builder-when-it-earns-its-place)

## Start with a function

Suppose a command accepts a retry count. Parsing needs text, not an object that
stores text until someone calls `parse`.

```rust
pub fn parse_retry_count(input: &str) -> Result<u32, ParseIntError> {
    input.trim().parse()
}
```

This is an excerpt from [retry.rs](../examples/src/retry.rs), which imports
`ParseIntError` and contains the tests. `&str` borrows the input. `trim()` ignores
surrounding whitespace. `parse()` returns the number or an error; the return type
tells the compiler which number type is required. The expression has no trailing
semicolon because it is the function's return value.

The caller can write `parse_retry_count(input)`. A `RetryParser::new(input).run()`
interface would add construction without providing useful state or another
operation. Prefer the function for this requirement.

> Avoid creating “doer” objects. That is, objects which are created only to execute a single action.

— [rust-analyzer, Functions Over Objects](https://rust-analyzer.github.io/book/contributing/style.html#functions-over-objects)

A parser object can make sense when it keeps a useful resource or state: for
example, a streaming parser that receives several chunks. A private context
struct can also organize a large implementation without becoming part of the
caller's API. Do not confuse a small public interface with a ban on internal
structs.

Language references: [functions and expressions](https://doc.rust-lang.org/book/ch03-03-how-functions-work.html)
and [string parsing](https://doc.rust-lang.org/std/primitive.str.html#method.parse).

## Use a struct for data that belongs together

Now consider a report. Its title and author stay together and are used by several
operations. A struct makes that relationship explicit.

```rust
pub struct Author {
    pub name: String,
}

pub struct Report {
    title: String,
    author: Author,
}

impl Report {
    pub fn new(title: String, author: Author) -> Self {
        Self { title, author }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn heading(&self) -> String {
        format!("{} - {}", self.title, self.author.name)
    }
}
```

This shortened excerpt omits the documentation and other methods in
[report.rs](../examples/src/report.rs). `impl Report` groups behavior associated
with `Report`. `Self` means `Report` inside that block. `new` is an associated
function, called as `Report::new(...)`; it has no `self` parameter. `heading` is a
method, called on a report value.

`new` takes the title and author because the report keeps them. `title` gives the
caller a borrowed view of existing text. `heading` produces different text, so it
returns a new `String`. These different signatures describe different ownership
needs; they are not inconsistent style.

The example accepts any title. Do not invent a rule that titles must be nonempty
unless the application needs it. The fields are private so the report can change
its representation without changing callers. `Author::name` is deliberately
exposed data. A released library may choose private fields there too to preserve
future flexibility.

Reference: [methods and associated functions](https://doc.rust-lang.org/book/ch05-03-method-syntax.html).

## Compose types without hiding the call

A `Report` contains an `Author`. That is composition: one value owns another
value and uses it as part of its work. It is not inheritance.

Suppose the author later gains a `display_name` method. The report can call
`self.author.display_name()`. Containing an author does not automatically give
`Report` all of `Author`'s methods or trait implementations. Add a forwarding
method only when it belongs in the report's interface.

Likewise, implementing a trait for the inner type does not implement it for the
outer type. Write an explicit implementation for the outer type when callers
need one. Do not use `Deref` to imitate a base class or to expose every inner
method. It has a different role in smart-pointer-like types.

The supplied Stack Overflow discussion is a useful model for explaining a
change step by step. Its Go embedding behavior is not a Rust rule. Use the Rust
Book to settle what Rust actually does.

References: [Rust's object-oriented features](https://doc.rust-lang.org/book/ch18-01-what-is-oo.html)
and [`Deref`](https://doc.rust-lang.org/std/ops/trait.Deref.html).

## Put alternatives in an enum

Suppose a device is disconnected, connected to a port, or has failed. Several
booleans would allow contradictory combinations: connected and failed, or
connected with no port. An enum names the alternatives and stores their data.

```rust
pub enum Connection {
    Disconnected,
    Connected { port: PathBuf },
    Failed { reason: String },
}

impl Connection {
    pub fn port(&self) -> Option<&Path> {
        match self {
            Self::Connected { port } => Some(port.as_path()),
            Self::Disconnected | Self::Failed { .. } => None,
        }
    }
}
```

The imports and tests are in [device.rs](../examples/src/device.rs). A connected
value must have a port. A failed value must have a reason. `match` handles the
alternatives, and the compiler checks that they are covered. The returned path
is borrowed from the connection; it is not copied.

Do not replace independent settings with an enum. “Include timestamps” and
“use color” may both be true. Two booleans in a named configuration struct can
express that honestly. An enum fits mutually exclusive alternatives, not every
pair of booleans.

References: [enums](https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html)
and [type safety](https://rust-lang.github.io/api-guidelines/type-safety.html).

## Add a trait when a caller needs shared behavior

Writing only report headings does not require a custom trait. The concrete
`write_report` function already does that job. Now suppose a summary screen must
show both reports and build results. The shared requirement is a short summary.

```rust
pub trait Summary {
    fn summary(&self) -> String;
}

impl Summary for Report {
    fn summary(&self) -> String {
        self.heading()
    }
}

impl Summary for BuildResult {
    fn summary(&self) -> String {
        let status = if self.passed { "passed" } else { "failed" };
        format!("{}: {status}", self.target)
    }
}
```

`BuildResult` and the complete implementations are in
[report.rs](../examples/src/report.rs). The trait names the operation. Each
implementing type supplies it. The fields still belong to the structs; the trait
does not become a container for their state.

Here, summaries are a specific application capability. Use `Display` instead
when the requirement is simply the type's ordinary human-readable representation.
Prefer an existing standard trait when its contract fits; do not introduce a
near-duplicate only to rename its methods.

A trait does not require two implementations already in the repository. An
extension point for downstream users or a real external dependency may justify
one implementation today. State that need. “We might need flexibility” alone is
not a reason to add another interface.

References: [traits](https://doc.rust-lang.org/book/ch10-02-traits.html)
and [Effective Rust, generics and trait objects](https://effective-rust.com/generics.html).

## Choose how the caller supplies an implementation

A trait answers what behavior is needed. The function signature answers how an
implementation is supplied.

```rust
pub fn write_summary(item: &impl Summary, output: &mut impl Write) -> io::Result<()> {
    writeln!(output, "{}", item.summary())
}
```

This function accepts one borrowed item of a concrete type implementing
`Summary`. The compiler selects the implementation using that type. `Write` is
a standard trait: output can be a file or the byte buffer used by the tests.

If the caller has a collection of different kinds of summary items, use trait
objects at that boundary:

```rust
pub fn write_summaries(items: &[&dyn Summary], output: &mut impl Write) -> io::Result<()> {
    for item in items {
        writeln!(output, "{}", item.summary())?;
    }
    Ok(())
}
```

`&dyn Summary` is a borrowed reference carrying enough information to call the
right implementation at runtime. The slice can contain both a report and a
build result. Neither the slice of references nor `&dyn Summary` requires boxing
the items. Use `Box<dyn Summary>` only when the program needs to own items through
that interface.

A named generic parameter, such as `T: Summary`, is useful when the same type
must appear in several arguments or the return type. Return-position
`impl Summary` hides one concrete return type; it does not permit unrelated
concrete types in different return branches. For a fixed set of alternatives,
an enum can be clearer than either trait-object storage or more generic layers.

Only dyn-compatible traits can be used as trait objects. Consult the compiler
and the trait's contract instead of adding bounds until errors disappear.
Do not choose dispatch primarily on an unmeasured speed claim.

References: [Effective Rust, Item 12](https://effective-rust.com/generics.html)
and [dyn compatibility](https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility).

## Use a configuration struct or builder when it earns its place

Start with a short constructor when the required values are obvious. When
several arguments have the same type or many switches appear at the call site,
a named configuration struct can make the choices visible. A builder is useful
when callers choose among many optional settings in different combinations.

Do not introduce a builder merely because a type is public. It adds another
type and another set of methods to understand and test. Conversely, do not keep
an unreadable eight-argument constructor just to avoid using one.

Keep required values required. A builder can accept them up front and use
methods for optional settings; it need not start in a half-valid state. Use
`Default` only when it produces a usable, valid value. Do not fabricate an empty
identifier or disconnected resource and require callers to repair it later.

A method that consumes and returns `self` supports chained calls; a method
borrowing `&mut self` supports repeated updates to an existing builder. Choose
based on callers, then keep that choice consistent within the API.

References: [Effective Rust, builders](https://effective-rust.com/builders.html)
and [Rust API Guidelines, complex construction](https://rust-lang.github.io/api-guidelines/type-safety.html).
