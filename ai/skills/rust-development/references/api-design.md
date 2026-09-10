# API design and type contracts

Read this before changing data models, trait implementations, public interfaces,
or re-exports. Apply the parts affected by the task, including to internal types
when their contracts matter. A small private type does not need a public-library
framework.

## Contents

- [Model meaningful distinctions](#model-meaningful-distinctions)
- [Choose standard traits deliberately](#choose-standard-traits-deliberately)
- [Conversions and borrowed views](#conversions-and-borrowed-views)
- [Design custom traits around callers](#design-custom-traits-around-callers)
- [Imports, visibility, and compatibility](#imports-visibility-and-compatibility)
- [Verify the contract](#verify-the-contract)

## Model meaningful distinctions

> Newtypes can statically distinguish between different interpretations of an underlying type.

— [Rust API Guidelines, C-NEWTYPE](https://rust-lang.github.io/api-guidelines/type-safety.html#newtypes-provide-static-distinctions-c-newtype)

A newtype is a struct wrapping another value. The point is not another name;
it is making the compiler distinguish meanings. `type UserId = u64` and
`type ProjectId = u64` are still interchangeable. Distinct structs are not.


Use an enum when exactly one alternative can apply. A variant can hold the data
needed for that alternative, instead of leaving unrelated fields as `Option`.
Use a newtype when two values have the same representation but different meanings:
`UserId` and `ProjectId` should not be interchangeable merely because both use
`u64`. A type alias does not prevent that mistake.

Use private fields and checked construction for validated values. Check every
way of creating or changing the value, including `Default`, conversions, setters,
and deserialization. Do not derive a conversion or deserializer that bypasses
validation. Prefer existing types such as `Duration`, `PathBuf`, and `NonZeroUsize`
when their meanings match. A builder helps with many independent optional settings;
a short constructor is better for a few required values.

Reference: [Rust API Guidelines: type safety](https://rust-lang.github.io/api-guidelines/type-safety.html).

### Example: a checked user identifier

Suppose this application requires nonzero user IDs. `NonZeroU64` already stores
that guarantee. The outer `UserId` adds the application meaning. The following
excerpt is from [identity.rs](../examples/src/identity.rs); the file also defines
`InvalidUserId`, its standard error implementations, and the tests.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(NonZeroU64);

impl TryFrom<u64> for UserId {
    type Error = InvalidUserId;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        let Some(value) = NonZeroU64::new(value) else {
            return Err(InvalidUserId);
        };
        Ok(Self(value))
    }
}
```

`TryFrom` makes failure part of construction. `type Error` names the error for
this implementation. The `let ... else` handles zero first; the remaining path
has a value known to be nonzero. The private field stops external callers from
constructing an unchecked value through this API. Code inside the defining
module must preserve the guarantee too.

Converting a valid `UserId` back to `u64` cannot fail, so that direction uses
`From`. No default ID is defined because the requirement does not supply one.
A `Default` implementation or deserializer must not silently invent an invalid
ID or a special “not initialized yet” value.

Do not wrap every integer. Use a newtype when confusing values or bypassing a
real validation rule would be a meaningful mistake. For an ordinary duration,
`std::time::Duration` may already provide the distinction you need.

References: [`TryFrom`](https://doc.rust-lang.org/std/convert/trait.TryFrom.html)
and [`NonZeroU64`](https://doc.rust-lang.org/std/num/type.NonZeroU64.html).

## Choose standard traits deliberately

A trait implementation is a promise to callers, not decoration. Deriving a
trait asks the compiler to generate its implementation from the fields. That is
often the simplest correct choice, but check whether those fields express the
meaning you intend.


| Trait | Contract and decision |
| --- | --- |
| `Debug` | Useful diagnostic output. Review whether deriving it exposes credentials or other secrets. Debug formatting is not a stable serialization format. |
| `Display` | A deliberate human-facing representation. Implement it rather than `ToString` directly when a textual representation makes sense. |
| `Clone` | Explicit duplication. Explain when cloning a handle shares the same resource rather than copying its contents. |
| `Copy` | Implicit duplication for suitable value types. Require correct semantics and acceptable cost; it cannot coexist with `Drop`. Do not promise it merely to avoid ownership decisions. |
| `PartialEq`, `Eq`, `Hash` | Equality must describe the intended identity. Equal keys must produce equal hashes. Derive together when fields define identity; coordinate manual implementations. |
| `PartialOrd`, `Ord` | Ordering must agree with equality and with each other. Claim a total order only when the type has one. |
| `Default` | A meaningful valid default, not a placeholder that requires later repair. |

References: [interoperability](https://rust-lang.github.io/api-guidelines/interoperability.html),
[`Hash`](https://doc.rust-lang.org/std/hash/trait.Hash.html), and
[`Drop`](https://doc.rust-lang.org/std/ops/trait.Drop.html).

### Example: equality and hashing must use the same identity

Suppose two account values are considered equal when their IDs match, even if
their display names differ. A handwritten equality implementation that compares
only the ID cannot be combined with a derived hash that includes the display
name. Equal values could then produce different hashes.

Either let all relevant fields define both equality and hashing, or implement
both consistently using the intended identity. Apply the same reasoning to
ordering. Test the relationships when implementations are handwritten; do not
assert a particular hash number, which is not the contract.

Reference: [`Hash` and equality](https://doc.rust-lang.org/std/hash/trait.Hash.html#hash-and-eq).

## Conversions and borrowed views

Implement `From` for an obvious, infallible value conversion; use `TryFrom` when
conversion can fail. Implementing them provides the corresponding `Into` or
`TryInto` support, so normally do not implement those counterparts separately.
Use `TryFrom` for a narrowing numeric conversion when an out-of-range value must
be rejected rather than truncated.

Use `AsRef` for a cheap borrowed view when generic callers benefit. It is not
necessary to make every parameter generic: `&str`, `&[T]`, and `&Path` often say
exactly what is needed. `Borrow` additionally promises compatible equality,
hashing, and ordering with the borrowed form. Do not use it as interchangeable
spelling for `AsRef`. Use `Deref` for genuine smart-pointer-like behavior, not
class inheritance or automatic exposure of a wrapped object's entire API.

References: [`From`](https://doc.rust-lang.org/std/convert/trait.From.html),
[`AsRef`](https://doc.rust-lang.org/std/convert/trait.AsRef.html), and
[Effective Rust: conversions](https://effective-rust.com/casts.html).

## Design custom traits around callers

Read [design choices](design-choices.md#add-a-trait-when-a-caller-needs-shared-behavior)
for the report-to-summary example and [ownership](ownership.md#choose-a-callback-bound)
for callbacks. Use the following checks when the abstraction is justified.

> When designing your public types and primary API surface, avoid exposing nested or complex parametrized types to your users.

— [Microsoft, M-SIMPLE-ABSTRACTIONS](https://microsoft.github.io/rust-guidelines/agents/all.txt)

This does not prohibit containers such as `Vec<T>` or useful generic functions.
The question is which choices a caller must understand. Keep backend and storage
parameters private when the caller does not select them. Expose a parameter when
substituting that type is part of the API's purpose.


State what each operation promises and which type supplies it. An associated
type ties a type to an implementation, such as an iterator's item type. A generic
trait parameter permits implementations parameterized by different types.

Choose a concrete type, generic parameter, or trait object based on actual callers.
Use `FnOnce` when calling a callback once is sufficient, `FnMut` when repeated
calls may mutate captured state, and `Fn` when calls only need shared access to
the captures. Avoid stronger bounds than required. For trait objects, check dyn
compatibility with the project's compiler. Return-position `impl Trait` represents
one hidden concrete type; use an enum or compatible trait object for genuinely
different runtime alternatives. Do not build custom iteration APIs when standard
`Iterator` or `IntoIterator` fits.

References: [Effective Rust: generics and trait objects](https://effective-rust.com/generics.html)
and [The Rust Book: closures](https://doc.rust-lang.org/book/ch13-01-closures.html).

## Imports, visibility, and compatibility

Prefer explicit imports and readable module-qualified names when names would
otherwise collide. Follow the repository's grouping; do not reformat unrelated
imports. Keep important types and entry points easy to find before private detail.
Use `pub(crate)` when only this crate needs access. A private implementation
module plus a deliberate `pub use` can expose a stable API without exposing the
file layout. Document that API where consumers find it.

Before changing a released interface, check names and paths, field visibility,
function signatures, trait bounds, enum variants, associated items, implemented
traits, features, and observable behavior. Adding a required trait method or an
enum variant callers match exhaustively can break consumers. Consider
`#[non_exhaustive]` at the initial design stage when extension is expected; adding
it later can itself break callers. Preserve old paths or provide a migration when
compatibility requires it. A cleaner module layout is not permission to break it.

References: [rust-analyzer: style](https://rust-analyzer.github.io/book/contributing/style.html)
and [Cargo: SemVer compatibility](https://doc.rust-lang.org/cargo/reference/semver.html).

## Verify the contract

Exercise construction and validation through the supported API. Check handwritten
trait relationships and failure cases, not compiler-generated boilerplate. Build
examples as a consumer would, and check docs and supported features. For released
libraries, use the project's API compatibility checks when available; do not
claim compatibility solely because the library itself compiles.
