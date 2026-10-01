# API design and value semantics

Read this before changing public names, exports, signatures, exceptions,
dataclass behavior, dunder methods, compatibility, or deprecation.

## Define the public surface

Document the names callers may rely on. A leading underscore marks a name as
internal by convention. Use `__all__` when an explicit module export list helps
readers and wildcard-import tools; it does not create runtime access control.
Treat deliberate package re-exports as public paths.

Before changing a released API, inspect names, import paths, signatures,
positional and keyword behavior, return values, exceptions, side effects, and
serialization. A source-compatible signature can still change behavior.

## Keep dunder methods consistent

| Method or option | Review |
| --- | --- |
| `__repr__` | Provide useful diagnostics without secrets or an unstable promise of executable syntax. |
| `__str__` | Use for deliberate user-facing text, not machine serialization. |
| `__eq__` | Compare the intended value or identity and return `NotImplemented` for unsupported types. |
| `__hash__` | Equal objects must hash equally; mutable equality fields normally make instances unhashable. |
| Ordering | Define a meaningful order consistent with equality; do not invent one solely for sorting convenience. |
| Dataclass `frozen` | Prevent ordinary field assignment, not deep mutation of referenced objects. |
| Dataclass `unsafe_hash` | Use only after proving the hashed identity remains stable. |

Review generated dataclass methods together. `order=True` derives tuple-like
field ordering and requires compatible equality. Field order then affects
observable comparisons. Exclude a field only when it is genuinely outside the
value's identity.

## Design for callers

Use explicit required arguments. Prefer keyword-only parameters when call sites
otherwise contain ambiguous values or options are likely to grow. Do not turn a
short, obvious call into a builder. Avoid mutable defaults; use `None` or a
factory when each call needs independent state.

Return ordinary values and iterables whose ownership and lifetime are clear.
Do not expose mutable internal collections when callers expect a snapshot.
Decide whether returned iterators are one-shot and whether errors happen during
iteration; document those facts when they matter.

## Evolve APIs deliberately

Preserve public behavior unless the task permits a break. For a deprecation:

1. provide a supported replacement;
2. emit the project's chosen warning at the caller-facing stack level;
3. document the migration and removal policy; and
4. test the replacement and warning behavior through public use.

Do not leave two implementations drifting. Route old behavior through the new
path when that preserves semantics, or keep a focused compatibility branch until
removal.

## Verify compatibility

Exercise supported import paths, positional and keyword calls, generated value
semantics, error categories, and serialized forms affected by the change. Build
and test a consumer-style example for a published library. Passing the package's
own tests does not establish compatibility for every downstream use.
