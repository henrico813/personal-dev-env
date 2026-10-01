# Errors and resource cleanup

Read this before changing exception handling, translation, resource ownership,
partial setup, or fallible completion.

## Give callers usable failures

Catch an exception only where the code can recover, add useful context, or
translate an implementation detail into the error vocabulary callers use. Catch
specific exceptions. Keep the `try` suite small so the handler does not absorb a
failure from an unrelated operation.

Preserve structured information. Do not flatten exceptions into strings when a
caller needs the type or cause. Add safe context such as an operation, path, or
identifier; do not include secrets or sensitive payloads.

> Use exception chaining appropriately. `raise X from Y` should be used to indicate explicit replacement without losing the original traceback.

— [PEP 8, Programming Recommendations](https://peps.python.org/pep-0008/#programming-recommendations)

Use `raise NewError(...) from error` when translation should retain the cause.
Use `from None` only when deliberately suppressing irrelevant implementation
detail and the replacement carries what the caller needs.

The [count example](../examples/src/python_skill_examples/counts.py) distinguishes
read failures from invalid content while retaining each lower-level cause.

## Keep bugs distinct from expected failures

Do not catch `BaseException` for ordinary failures; it includes process-control
exceptions. Avoid blanket `except Exception` unless the boundary genuinely owns
reporting or containment, and re-raise unexpected failures when recovery is not
defined. Assertions state programmer invariants; they are not validation for
untrusted input and can be disabled.

## Own resources locally

Use `with` for files, locks, transactions, temporary resources, and other
context-managed lifetimes. Use `try/finally` when no suitable context manager
exists. Acquire close to cleanup and make ownership clear.

For a dynamic number of resources, `contextlib.ExitStack` registers each cleanup
as acquisition succeeds. If a later acquisition fails, the stack still unwinds
the earlier ones. This does not make an external operation all-or-nothing.

The [resource example](../examples/src/python_skill_examples/resources.py) uses
`ExitStack` to protect partial setup.

## Design context managers carefully

`__enter__` should return the value used inside the block. `__exit__` receives
exception information and suppresses the exception only by returning a true
value. Do not suppress failures by accident. A generator-based context manager
must yield exactly once; code after `yield` belongs in a `finally` path when
cleanup must run.

When setup performs several state-changing operations, register cleanup after
each success rather than waiting until every operation completes. Define how
cleanup failures are reported, especially while another exception is already
active.

## Make completion failure visible

Destructors and garbage collection are not suitable places to report a required
flush, close, commit, or shutdown result. Provide or call an explicit operation
when callers need to know completion succeeded. State whether failure leaves
partial output or a reusable object.

The [write example](../examples/src/python_skill_examples/resources.py) calls
`flush()` directly so an `OSError` reaches the caller. A successful flush does
not by itself promise durable storage.

## Verify failure paths

Check distinct caller-visible error categories, retained causes, partial effects,
cleanup after failed setup, and fallible completion where relevant. Control the
failing boundary rather than depending on disk exhaustion, network timing, or
object destruction.
