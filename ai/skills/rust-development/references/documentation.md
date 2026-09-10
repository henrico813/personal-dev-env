# Document what a caller or maintainer needs

Read this when documenting an API or explaining a Rust change. Use complete,
direct sentences. Start with what the code does, then explain the choices that
are not obvious from its types and names.

## Contents

- [Document behavior, not the signature](#document-behavior-not-the-signature)
- [Use an example that demonstrates a reason to call the API](#use-an-example-that-demonstrates-a-reason-to-call-the-api)
- [Separate usage behavior from a language explanation](#separate-usage-behavior-from-a-language-explanation)
- [Write comments that survive a refactor](#write-comments-that-survive-a-refactor)
- [Explain the decision in the handoff](#explain-the-decision-in-the-handoff)

## Document behavior, not the signature

Use `//!` for the containing crate or module and `///` for an item. Describe
public behavior and important internal guarantees. Explain units, accepted
ranges, ownership effects, I/O, and ordering when callers depend on them. Do not
repeat every parameter's type in prose.

For example, this describes more than “writes some lines”:

```rust
/// Writes lines with trailing newlines and flushes the output.
///
/// # Errors
/// Returns a write or flush error. Some output may already have been written.
/// A successful flush does not, by itself, guarantee durable storage.
```

It belongs to `write_lines` in
[buffered_output.rs](../examples/src/buffered_output.rs). The caller learns the
format, when completion is attempted, and what failure does not undo. Those
facts are more useful than a comment saying that `output` is a writer.

Add `# Errors`, `# Panics`, and `# Safety` only when applicable. An unsafe
function needs concrete caller obligations. A safe function does not become
sound because its documentation asks callers to avoid undefined behavior.
Use rustdoc links such as ``[`LoadRetryError`]`` for related items.

References: [Rust API documentation guidelines](https://rust-lang.github.io/api-guidelines/documentation.html)
and [rustdoc links](https://doc.rust-lang.org/rustdoc/write-documentation/linking-to-items-by-name.html).

## Use an example that demonstrates a reason to call the API

An example should show a realistic input and something the caller can observe.
The parser's documentation uses whitespace to demonstrate a promised behavior:

````rust
/// # Examples
///
/// ```
/// use rust_skill_examples::retry::parse_retry_count;
///
/// assert_eq!(parse_retry_count(" 3 ")?, 3);
/// # Ok::<(), std::num::ParseIntError>(())
/// ```
````

The final line gives the doctest a concrete `Result` type. Rustdoc hides lines
prefixed with `#` in displayed examples while retaining them for compilation.
Do not hide the important operation or a missing error path that way.

Prefer executable examples for public behavior. Use `no_run` when the code
should compile but must not execute during documentation tests. Use
`compile_fail` only when compiler rejection is the lesson. Do not use `ignore`
to make a broken example appear finished.

A straightforward getter does not need its own long tutorial. Link to a useful
example that already exercises it rather than repeating the same setup for every
item. The Rust API Guidelines explicitly qualify their example guidance:

> This guideline should be applied within reason.

— [Rust API Guidelines, C-EXAMPLE](https://rust-lang.github.io/api-guidelines/documentation.html#all-items-have-a-rustdoc-example-c-example)

Reference: [rustdoc documentation tests](https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html).

## Separate usage behavior from a language explanation

The production API needs lasting information: a borrowed result cannot outlive
its source, an identifier must be nonzero, or a failed write may have partial
output. Keep that information with the code.

The maintainer may also need to learn that `&self` borrows a value or that `?`
returns early on failure. Explain those concepts in the handoff or a learning
reference, near the code that uses them. Do not put a lesson on `&self` above
every method. Do not omit useful Rust features to avoid having to explain them.

A comment should not call the reader a beginner. Write the explanation so it is
useful to anyone unfamiliar with the code.

## Write comments that survive a refactor

A comment such as “create a vector” repeats the next statement. A comment such
as “keep request order because the device matches replies by position” explains
why changing the implementation could break behavior.

Keep that distinction in safety comments too. “This is safe” provides no reason.
“The live array provides all initialized bytes and remains unchanged during the
copy” identifies a checkable fact. See [unsafe code](unsafe.md) for the full example.

Delete or update a comment when its guarantee changes. Do not leave implementation
histories, self-reviews, promotional claims, or statements about following this
skill in production documentation. A short name and a clear type can remove the
need for a comment altogether.

## Explain the decision in the handoff

For a change that introduces borrowing, an explanation like this is sufficient:

> `parse_retry_count` accepts `&str` because it only reads the input. The caller
> keeps the string and can use it after the call. `Result` makes invalid input an
> error the caller can handle instead of a panic.

This is an example of the skill's own explanation style, not a source quotation.
Explain the unfamiliar concept, why it is needed here, and a relevant consequence
for maintenance. State an important alternative when the choice is not obvious.
Do not append a glossary or a list of every Rust feature used in the patch.

For a review, lead with concrete findings. Distinguish a correctness problem from
a style preference. For an implementation, report the checks that actually ran
and their results. Do not describe the code as “robust,” “elegant,” or
“production-ready” in place of explaining its behavior and limits.
