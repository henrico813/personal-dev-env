# Rust test organization

Load and follow the `behavior-focused-testing` skill for test purpose, scope,
naming, observable assertions, doubles, determinism, and regression coverage.
This reference covers Rust-specific test organization and mechanics.

## Use tables for comparable cases

A table fits when each case uses the same setup, operation, and assertion:

```rust
#[test]
fn parses_retry_counts() {
    let cases = [
        ("zero", "0", 0),
        ("ordinary count", "3", 3),
        ("surrounding whitespace", " 3 ", 3),
        ("maximum count", "4294967295", u32::MAX),
    ];

    for (case_name, input, expected) in cases {
        assert_eq!(parse_retry_count(input), Ok(expected), "{case_name}");
    }
}
```

These rows are not individually registered Rust tests. A failed assertion stops
the function, so later rows do not run. Use separate functions when cases need
independent filtering, isolation, or failure reports. Split a table when rows
need different setup or assertion branches.

## Control a dependency with a trait

Use real, fast components first. When a failure is otherwise difficult to
produce, implement the same production trait with a focused test type:

```rust
struct RejectFlush;

impl Write for RejectFlush {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "output disconnected",
        ))
    }
}
```

The full example is in
[buffered_output.rs](../examples/src/buffered_output.rs). Do not add a custom
trait around every type only to make substitution possible.

## Put tests in the right place

| Location | Use it for |
| --- | --- |
| `#[cfg(test)] mod tests` | Focused local behavior, including private details when useful. |
| `tests/*.rs` | Behavior exposed to another crate through the public API. |
| Rustdoc examples | Small executable demonstrations of public API use. |
| Binary tests | Arguments, exit status, stdout, and stderr. |

`#[cfg(test)]` includes a local module in the test build; it is not a Cargo
feature. Integration tests link the library as a dependency, so local test-only
helpers are not exported. Shared integration helpers can live in
`tests/common/mod.rs` and be imported with `mod common;`.

For a package with a binary and library, import the library from the binary
instead of declaring modules twice. Process tests should use Cargo's binary path
rather than assuming `target/debug`; see [cli.rs](../examples/tests/cli.rs).

Test handwritten equality, ordering, validation, and error behavior when they
matter. Do not add tests solely to prove that a standard derive works. Property
tests and fuzzing can supplement clear examples when a few cases are
insufficient.

References: [Rust test organization](https://doc.rust-lang.org/book/ch11-03-test-organization.html),
[Cargo integration tests](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#integration-tests),
and [Rust test functions](https://doc.rust-lang.org/book/ch11-01-writing-tests.html).
