# Make ownership clear

Read this when choosing parameter types, changing method receivers, fixing
borrow errors, or explaining a move, borrow, clone, or lifetime. Start by asking
who needs to keep the value after the operation finishes.

## Contents

- [Borrow when inspecting a value](#borrow-when-inspecting-a-value)
- [Take ownership when retaining a value](#take-ownership-when-retaining-a-value)
- [Choose the method receiver](#choose-the-method-receiver)
- [Clone for a reason](#clone-for-a-reason)
- [Keep lifetime annotations tied to a real relationship](#keep-lifetime-annotations-tied-to-a-real-relationship)
- [Use control flow the reader can follow](#use-control-flow-the-reader-can-follow)
- [Choose a callback bound](#choose-a-callback-bound)

## Borrow when inspecting a value

The retry parser only reads text:

```rust
pub fn parse_retry_count(input: &str) -> Result<u32, ParseIntError> {
    input.trim().parse()
}
```

The complete file is [retry.rs](../examples/src/retry.rs). The caller keeps its
text and can use it after the call. Accepting a `String` would ask the caller to
transfer ownership for an operation that does not need it. Accepting `&String`
would unnecessarily require a particular owned string representation.

For the same reason, use `&[T]` when reading a sequence and `&Path` when reading a
path. Use `&mut [T]` to change existing elements without resizing. Use `&mut Vec<T>`
when the operation really needs to grow or shrink the vector. A borrowed view is
a good default, not a rule to erase capabilities an operation requires.

References: [borrowing](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html)
and [slices](https://doc.rust-lang.org/book/ch04-03-slices.html).

## Take ownership when retaining a value

A report stores its title, so its constructor can take a `String`:

```rust
let title = String::from("Build summary");
let author = Author { name: String::from("Avery") };
let report = Report::new(title, author);
```

This is a call-site fragment using the types in
[report.rs](../examples/src/report.rs). After the call, `title` and `author` have
moved into the report. The caller uses `report` instead of those moved variables.
Moving the `String` transfers its ownership; it does not duplicate the text.

Storing `&str` instead is possible, but then the report borrows from an owner
outside itself. That owner must remain valid whenever the borrowed title is used.
Use that design when the relationship is useful, such as a short-lived parsed
view into a retained input buffer. Do not add lifetime parameters to long-lived
application objects merely to avoid a small allocation.

For a broadly used constructor, `impl Into<String>` may make different caller
inputs convenient. It also hides a conversion and possible allocation behind the
call. Use it when that convenience helps actual callers; a concrete `String`
parameter makes ownership particularly clear to a beginner.

References: [ownership and moves](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html)
and [Effective Rust, optimization tradeoffs](https://effective-rust.com/optimize.html).

## Choose the method receiver

The three forms below do different jobs. They come from the report example.

```rust
pub fn title(&self) -> &str {
    &self.title
}

pub fn rename(&mut self, title: String) {
    self.title = title;
}

pub fn into_title(self) -> String {
    self.title
}
```

These methods belong inside `impl Report`. `&self` borrows a report for reading.
`&mut self` borrows it exclusively so the method can update it. `self` consumes
the report and transfers its title to the caller. For this non-`Copy` type, the
caller cannot keep using the consumed report.

The getter returns a view into the report, so that view cannot be used after its
owner is gone. A mutable borrow also prevents conflicting access while it is in
use. These restrictions describe which access is safe; they are not requests to
add a lock.

Reference: [method receivers](https://doc.rust-lang.org/book/ch05-03-method-syntax.html).

## Clone for a reason

A draft that must be edited independently of an original is a reason to clone:

```rust
let mut draft = original.clone();
draft.rename(String::from("Revised summary"));
```

This call-site fragment uses `Report`, whose derived `Clone` clones its owned
strings. The test `renaming_clone_leaves_original_unchanged` checks the intended
behavior. It is not a reason to clone before every method call.

`Clone` does not always mean a fully independent resource. Cloning an `Arc<T>`
creates another owner of the same allocation. Cloning a custom service handle
may keep talking to the same service. Check the type's contract before promising
independent state.

When the borrow checker rejects a change, first identify the overlapping uses
or the owner that ends too soon. Often a smaller borrow scope, a moved value, or
a borrowed function parameter solves the actual problem. A clone is appropriate
when an independent owned value is what the design needs and its cost is acceptable.

References: [`Clone`](https://doc.rust-lang.org/std/clone/trait.Clone.html)
and [`Arc`](https://doc.rust-lang.org/std/sync/struct.Arc.html).

## Keep lifetime annotations tied to a real relationship

This complete function borrows a line from the input instead of allocating it:

```rust
fn first_line(input: &str) -> Option<&str> {
    input.lines().next()
}
```

Rust infers the relationship here: the returned text comes from `input`. Writing
`'a` on both types could name that same relationship, but would not extend the
input's life. Add an explicit lifetime when the compiler needs the relationship
stated, not as a ritual on every borrowed argument.

Never return a reference into a local `String` that will be dropped when the
function returns. Return an owned `String`, borrow from a caller-owned input,
or change who owns the storage. Adding `'static` is not a way to keep a local
variable alive.

When a thread or task requires `T: 'static`, it generally means `T` cannot retain
shorter-lived borrowed data. An owned `String` can meet that bound and still be
dropped normally. See [concurrency](concurrency.md) before changing task captures.

References: [lifetimes](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html)
and [Tokio, spawning](https://tokio.rs/tokio/tutorial/spawning).

## Use control flow the reader can follow

Use a short iterator chain when each step describes the transformation. The
`first_line` function above does not become clearer when expanded into a manual
loop. Use a loop when there is substantial branching, early termination, or
state to update. Do not replace either form solely to reduce line count.

Use `match` when the alternatives deserve separate handling. `if let` suits one
interesting pattern. `let ... else` is useful for rejecting an invalid case and
keeping the successful path unindented, as in the checked identifier example.
Use named intermediate values when a nested expression hides the steps.

Keep bindings immutable unless they must change. Shadowing is reasonable when a
name keeps the same meaning while its representation changes, such as raw text
becoming a parsed count. Use a different name when the distinction matters later.
Do not add a custom macro for ordinary control flow; macros make both compiler
errors and call behavior harder to follow when a function would do.

References: [control flow](https://doc.rust-lang.org/book/ch03-05-control-flow.html)
and [Effective Rust, iterators](https://effective-rust.com/iterators.html).

## Choose a callback bound

A closure is a small function value that can use values from its surrounding
scope. Prefer the standard callback traits to a custom trait with one method
when the caller only needs to provide one operation.

| Requirement | Bound to consider |
| --- | --- |
| Call once, possibly consuming captured values | `FnOnce` |
| Call repeatedly, allowing captured state to change | `FnMut` |
| Call through shared access to the closure | `Fn` |

`Fn` does not promise purity: a closure can still write a file or use interior
mutability. The bound describes access to the closure's captures. A `move`
closure takes ownership of captures but is not automatically `FnOnce`-only;
what its body does determines which call traits it implements.

Use an associated type when one implementation chooses a related output type;
use a generic parameter when implementations need to vary with that parameter.
For example, `Iterator::Item` is chosen by an iterator implementation. Do not
invent an elaborate callback interface until the caller needs more than the
standard traits provide.

Reference: [closures and their call traits](https://doc.rust-lang.org/book/ch13-01-closures.html).
