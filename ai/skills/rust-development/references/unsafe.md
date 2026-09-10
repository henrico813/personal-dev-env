# Unsafe code and foreign interfaces

Read this before editing unsafe blocks, unsafe functions or traits, raw-memory
operations, foreign-function interfaces (FFI), or manual `Send`/`Sync` implementations.
Do not introduce these techniques merely to satisfy this checklist.

## Contents

- [Establish the need and the boundary](#establish-the-need-and-the-boundary)
- [Write a concrete safety argument](#write-a-concrete-safety-argument)
- [Example: copy a foreign buffer](#example-copy-a-foreign-buffer)
- [Check foreign requirements explicitly](#check-foreign-requirements-explicitly)
- [Validate without overstating the result](#validate-without-overstating-the-result)

## Establish the need and the boundary

Prefer an existing safe API. State the concrete requirement that needs unsafe
code: foreign integration, a sound low-level abstraction, or measured performance
that a safe alternative does not provide. Keep the implementation and the state
it depends on small enough to review together. `unsafe` transfers particular
safety checks to the programmer; it does not remove Rust's rules.

A safe interface must remain memory-safe for every permitted safe use. Documenting
that callers should avoid an unsafe situation does not make a safe signature
sound. Enforce the condition or expose an appropriately documented unsafe API.
Do not mark ordinary destructive operations, such as deleting records, `unsafe`
unless incorrect use can violate Rust's memory-safety requirements.

References: [Microsoft: correctness](https://microsoft.github.io/rust-guidelines/agents/all.txt)
and [Rustonomicon: safe and unsafe](https://doc.rust-lang.org/nomicon/safe-unsafe-meaning.html).

## Write a concrete safety argument

For each unsafe operation, explain the applicable conditions next to the code
with a `SAFETY` comment. Check where those conditions come from and where other
code could invalidate them. Cover the relevant parts of:

- Pointer validity, alignment, allocation bounds, and initialized valid values.
- Borrow lifetimes, aliasing, mutation, and concurrent access.
- Ownership transfer, exactly-once release, allocator matching, and partially
  completed operations.
- Panic, early-return, and cancellation paths, including callbacks supplied by
  callers.

For an unsafe public function or trait, put the caller's or implementer's
obligations in `# Safety` documentation. For an unsafe call, explain how this
call satisfies the callee's requirements. Do not use vague comments such as "pointer
is safe" or rely on tests instead of an argument. Keep unsafe operations in
explicit narrow blocks, including inside unsafe functions, following the
repository's edition and lints.

References: [Rust Reference: undefined behavior](https://doc.rust-lang.org/reference/behavior-considered-undefined.html)
and [Rustonomicon](https://doc.rust-lang.org/nomicon/).

## Example: copy a foreign buffer

Suppose a foreign API supplies a pointer and length, and the application needs
an owned byte vector. A null check alone cannot establish that the memory is
live, long enough, or not changing. The function therefore has an unsafe caller
set of requirements rather than pretending it can validate an arbitrary pointer.

```rust
/// Copies a foreign buffer into a Rust-owned vector.
///
/// An empty buffer may use a null pointer. This does not free the foreign buffer.
///
/// # Safety
/// For a nonzero `len`, `data` must be non-null and point to `len` initialized
/// bytes within one live allocation. The range must be valid for reads, fit
/// within `isize::MAX` bytes, and not wrap the address space. No one may mutate
/// or free the bytes while this function reads them. The caller remains
/// responsible for any cleanup required by the foreign allocator.
pub unsafe fn copy_foreign_bytes(data: *const u8, len: usize) -> Vec<u8> {
    if len == 0 {
        return Vec::new();
    }

    // SAFETY: The caller guarantees a readable, initialized, single-allocation
    // range for this call, with no concurrent mutation. We copy it before returning
    // and do not let a reference to the foreign allocation escape.
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    bytes.to_vec()
}
```

The `# Safety` section says what the caller must establish. The `SAFETY` comment
explains how the unsafe block uses those guarantees. Empty input is handled
before creating a slice because `from_raw_parts` requires a non-null pointer
even for a zero-length slice. The owned result does not borrow the foreign buffer.

Use `input.to_vec()` instead when the caller already has a valid `&[u8]`; no
unsafe function is needed then. This example is for an existing raw-memory
boundary, not a reason to introduce raw pointers into an ordinary API. It also
does not replace reviewing an actual foreign API's allocation and cleanup rules.

Tests in [foreign.rs](../examples/src/foreign.rs) use valid buffers and the
explicitly permitted empty-null case. Do not “test” forbidden pointer uses by
executing undefined behavior.

Reference: [`slice::from_raw_parts` safety requirements](https://doc.rust-lang.org/std/slice/fn.from_raw_parts.html).

## Check foreign requirements explicitly

Verify the ABI, supported targets, parameter representation, string encoding and
termination, nullability, buffer lengths, and who owns and releases each value.
Use `repr(C)` or other representation controls where the actual foreign requirements
requires them; do not assume ordinary Rust layouts, `String`, or `Vec` are C types.
Keep callbacks alive for the period the foreign side can call them. Document any
thread-affinity or reentrancy requirements.

Do not let unwinding cross an ABI boundary without an explicitly supported,
reviewed requirements. Check both Rust panics and foreign exceptions. Use generated
bindings or established wrappers where they reduce risk, but still verify the
ownership and safety obligations of the boundary.

Reference: [Rustonomicon: FFI](https://doc.rust-lang.org/nomicon/ffi.html).

## Validate without overstating the result

Test public behavior, boundaries, failure paths, and safe callers that stress the
assumptions. Run Miri on supported tests, using the project's supported nightly
setup when it needs one. Miri detects many forms of undefined behavior on executed
paths, but has platform and foreign-call limitations and is not a proof of
soundness. A skipped foreign call or unsupported test is a validation gap, not a
passing result. Use relevant native-platform, sanitizer, or integration checks
when the project supports them.

Do not install a new production toolchain or change the MSRV merely to run a
separate validation tool. Report the actual commands, coverage, unsupported cases,
and remaining safety assumptions. Unsafe performance changes also need a relevant
benchmark; a faster microbenchmark does not excuse an unsound abstraction.

Reference: [Miri: capabilities, limitations, and setup](https://github.com/rust-lang/miri).
