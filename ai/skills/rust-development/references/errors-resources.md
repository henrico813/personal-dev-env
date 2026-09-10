# Errors and resource cleanup

Read this before changing failure handling, resource ownership, or cleanup.

## Contents

- [Give callers usable errors](#give-callers-usable-errors)
- [Example: keep the underlying error](#example-keep-the-underlying-error)
- [Distinguish bugs from expected failures](#distinguish-bugs-from-expected-failures)
- [Make ownership perform cleanup](#make-ownership-perform-cleanup)
- [Make fallible completion explicit](#make-fallible-completion-explicit)
- [Verify the behavior](#verify-the-behavior)

## Give callers usable errors

> If all of the different errors that a function encounters are already of the same type, it can just return that type.

— [David Drysdale, Effective Rust, Item 4](https://effective-rust.com/errors.html)

Do not add an error wrapper merely because a function exists. The retry parser
returns `ParseIntError` because that already describes its failures. Reading the
count from another source introduces a second kind of failure, so the caller may
need a type that distinguishes reading from parsing.


Use `Option` for expected absence and `Result` for recoverable failure. Select
error types by what callers need to do. An enum suits failures callers must
match; a public struct with private representation can preserve flexibility.
Application error wrappers suit failures mainly enriched with context and reported.
Keep the project's established approach instead of adding another error crate
for one function.

Implement `Debug`, `Display`, and `Error` for custom errors. Store an underlying
error and expose it through `source()` when it caused this error. Do not replace
it with `to_string()` merely to simplify the type. `From` or an established derive
can support propagation with `?`; use a contextual conversion when the operation
or resource must also be recorded.

Describe the failed operation and useful safe context, such as an allowed path or
record identifier. Do not include passwords, tokens, or sensitive payloads. Report
errors at a clear boundary rather than logging the same propagated error at every
layer. Do not discard a fallible return value without an explicit reason.

References: [`Error`](https://doc.rust-lang.org/std/error/trait.Error.html) and
[Effective Rust: error types](https://effective-rust.com/errors.html).

## Example: keep the underlying error

The following excerpts are from [retry.rs](../examples/src/retry.rs). The file
includes imports, documentation, and `Display` as well.

```rust
#[derive(Debug)]
pub enum LoadRetryError {
    Read(io::Error),
    Invalid(ParseIntError),
}

impl Error for LoadRetryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Invalid(error) => Some(error),
        }
    }
}
```

The enum preserves the two kinds of failure. `source()` exposes the original
error instead of a string copied from it. Its returned reference borrows from
`self`; the `'static` bound on the trait object does not make that reference
live forever. `Display` supplies a useful outer description, while a reporter
can walk the cause chain for the underlying details.

```rust
pub fn read_retry_count(input: &mut impl Read) -> Result<u32, LoadRetryError> {
    let mut text = String::new();
    input.read_to_string(&mut text).map_err(LoadRetryError::Read)?;
    parse_retry_count(&text).map_err(LoadRetryError::Invalid)
}
```

`map_err` changes the error into the matching enum variant. `?` returns early
on a read failure and otherwise lets parsing continue. The final expression
returns the parsing result. This function reads a small trusted input until EOF;
a network or untrusted-input API would also need explicit size and time limits.

This code spells out the standard traits so the reader can see what happens.
In a project already using `thiserror`, deriving the same behavior may be clearer
than repeating the implementation. An application that only reports failures
can use its existing application error wrapper instead. Preserve typed errors
where callers need to recover differently; do not impose the same representation
on every layer.

References: [`Error`](https://doc.rust-lang.org/std/error/trait.Error.html),
[`Result::map_err`](https://doc.rust-lang.org/std/result/enum.Result.html#method.map_err),
and [`thiserror`](https://docs.rs/thiserror/latest/thiserror/).

## Distinguish bugs from expected failures

Bad user input, missing files, and network failures are not ordinarily reasons
to panic. Use `expect` only where the condition is already guaranteed, and explain
that guarantee in the message. Tests can use it for setup failures. Do not catch
panics as a substitute for normal error handling or assume callers will recover
from your panic. Document intentional panic conditions and preserve valid state
on early returns and partial failure.

Reference: [Microsoft's correctness guidance](https://microsoft.github.io/rust-guidelines/agents/all.txt).

## Make ownership perform cleanup

RAII stands for resource acquisition is initialization. In practical terms, a
value owns a resource, and ending that value's lifetime releases what it owns.
You do not need to write RAII infrastructure to use it: `File`, `Vec`, and lock
guards already manage their resources. A report containing strings does not
need a custom destructor to free those strings.


Prefer types that own their resources and existing guards that release access
when dropped. This is RAII: tying a resource's lifetime to a value's lifetime.
A file object owns its handle; a lock guard owns the current permission to access
locked state. Keep the guard's scope as short as the protected operation needs.
An inner block or `drop(guard)` can end that access deliberately. Do not call
`Drop::drop` directly.

A struct's fields are dropped automatically. Add a custom `Drop` implementation
only when additional cleanup is required, such as releasing a foreign handle.
Check exactly-once release, partially constructed state, and early returns.
Destructors should not panic or require asynchronous progress. Do not rely on
`Drop` executing after an abort, forced termination, or deliberate leak, and never
make memory safety depend on a destructor being impossible to skip.

References: [Effective Rust: RAII](https://effective-rust.com/raii.html),
[`Drop`](https://doc.rust-lang.org/std/ops/trait.Drop.html), and
[Rustonomicon: leaking](https://doc.rust-lang.org/nomicon/leaking.html).

## Make fallible completion explicit

`Drop` cannot return a `Result`. Provide or call `flush`, `finish`, `close`, or an
async `shutdown` operation when callers need confirmation that completion
succeeded. Define whether the operation consumes the owner, can be retried, or
leaves it usable after failure. Keep fallback cleanup best-effort without silently
claiming the operation succeeded.

For example, explicitly call `BufWriter::flush()` when flush errors matter;
errors while flushing during drop are ignored. Flushing a buffer is not the same
as guaranteeing durable storage. State the guarantee the application requires
and use the appropriate persistence operation when necessary.

Reference: [`BufWriter`](https://doc.rust-lang.org/std/io/struct.BufWriter.html).

### Example: return a flush failure

```rust
pub fn write_lines(output: impl Write, lines: &[&str]) -> io::Result<()> {
    let mut output = BufWriter::new(output);
    for line in lines {
        writeln!(output, "{line}")?;
    }
    output.flush()
}
```

The last expression returns the result of flushing. Replacing it with `Ok(())`
and relying only on `BufWriter`'s destructor would hide a possible completion
error. The buffer already owns its cleanup, so adding another `Drop`
implementation would not solve the reporting problem.

The [failure-path test](testing.md#control-a-dependency-with-a-trait)
uses a writer that rejects flushing. The test checks the caller-visible error,
not whether a particular internal buffer method was called.

## Verify the behavior

Test meaningful failure categories, retained causes, partial writes or updates,
and cleanup after an early return when these are part of the contract. Assert
structured error information where available instead of matching incidental
message punctuation. Control the failing dependency rather than relying on disk
exhaustion, network timing, or another global environmental accident. Keep mocks
or failure injectors confined to the boundary that needs them.
