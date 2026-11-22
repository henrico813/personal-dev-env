# Rust function names

Read this before you name a Rust function or method, or keep a Rust
name that does not start with a verb.

In Rust, start every function and method name with a verb. These
names need no verb:

- a test, marked by an attribute that ends in `test`, such as
  `#[test]` or `#[tokio::test]`
- `main`
- a method in an `impl Trait for Type` block, such as `fmt` or
  `next`, since the trait chose its name
- a constructor named `new`, or one whose name starts with `new_` or
  `with_`, since Rust code names constructors this way
- a getter

The conversion words `as`, `to`, `into` and `from` count as verbs, so
names such as `to_json` and `into_inner` follow Rust's conventions
and pass.

A method counts as a getter only when its code shows it, since a name
with no verb tells the caller that the method only reads a value. A
getter takes only `&self`, and its body is one expression that reads
a field of `self`:

- the field itself, such as `self.width`
- a reference to it, such as `&self.path`, or a copy, such as
  `*self.count`
- the field followed by `.clone()`, `.copied()`, `.cloned()` or an
  `as_` call with no arguments, such as `self.name.as_str()`

Any other method needs a verb, even one that takes only `&self`.

This method needs no verb, since its body only borrows a field:

```rust
fn path(&self) -> &Path {
    &self.path
}
```

This one needs a verb, such as `build_path`, since it builds a new
value:

```rust
fn path(&self) -> PathBuf {
    self.root.join(&self.name)
}
```
