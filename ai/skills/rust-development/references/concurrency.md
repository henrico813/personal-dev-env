# Shared state, threads, and async work

Read this before adding or reviewing shared mutable state, locks, threads, async
functions, or spawned tasks. Use the runtime already chosen by the repository;
the Tokio references explain examples, not a requirement to add Tokio.

## Contents

- [Choose ownership and synchronization separately](#choose-ownership-and-synchronization-separately)
- [Respect Send, Sync, and lifetimes](#respect-send-sync-and-lifetimes)
- [Borrow with scoped work when it fits](#borrow-with-scoped-work-when-it-fits)
- [Keep locks and blocking work bounded](#keep-locks-and-blocking-work-bounded)
- [Own task completion and cancellation](#own-task-completion-and-cancellation)
- [Verify observable behavior](#verify-observable-behavior)

## Choose ownership and synchronization separately

Start with ordinary owned values and sequential code when they meet the task.
Concurrency adds questions about who can change a value, when work finishes,
and which failures the caller sees. Async lets work suspend while waiting; it
does not automatically create a thread or make CPU work faster.


`Rc<T>` shares ownership within a thread. `Arc<T>` shares ownership across threads
when `T` permits it; cloning either handle does not clone the stored value.
`Arc` does not make unsynchronized inner mutation safe. Use a lock, channel, or
another suitable safe synchronization abstraction when concurrent access needs
coordination. Prefer passing owned messages when that makes ownership clearer.

`Cell` and `RefCell` enable mutation through a shared reference without providing
thread synchronization. `RefCell` checks borrowing at runtime and can panic on a
conflict. Use them for a real local ownership need, not as automatic borrow-checker
workarounds. Do not substitute custom atomics for a clear lock without understanding
the required memory ordering and having a measured or functional reason.

References: [`std::cell`](https://doc.rust-lang.org/std/cell/index.html) and
[Tokio: shared state](https://tokio.rs/tokio/tutorial/shared-state).

## Respect Send, Sync, and lifetimes

`Send` means a value can be transferred between threads safely. `Sync` means
shared references to it can be sent between threads safely. Let the compiler
infer these traits where possible. For a cross-thread future, inspect values
retained across `.await`; a non-`Send` value there can prevent moving the future.

A spawned task may need owned captures and `'static` bounds. In that setting,
`'static` rules out retained short-lived borrows; it does not mean the task or
its values live forever. Borrow directly, use scoped threads or locally awaited
futures when that fits, or move ownership into the task. Do not add `unsafe impl
Send` or `Sync` to make a compiler error disappear.

Reference: [Tokio: spawning](https://tokio.rs/tokio/tutorial/spawning).

## Borrow with scoped work when it fits

When two jobs finish before the function returns, they may not need shared
ownership through `Arc`. Scoped threads can borrow inputs from the caller:

```rust
pub fn count_two_reports(first: &str, second: &str) -> thread::Result<usize> {
    thread::scope(|scope| {
        let first_job = scope.spawn(move || first.lines().count());
        let second_job = scope.spawn(move || second.lines().count());
        let first_count = first_job.join();
        let second_count = second_job.join();
        Ok(first_count? + second_count?)
    })
}
```

The complete file is [threaded.rs](../examples/src/threaded.rs). `move` transfers
the captured references into each closure; it does not copy the strings. The
scope ensures those borrows do not outlive the inputs. Both handles are joined
before `?` can return a failure, so neither result is left unobserved.

This deliberately small example teaches ownership and joining, not a performance
recommendation. Counting two small strings sequentially is simpler. Use threads
when the actual workload justifies them; measure before claiming an improvement.

Reference: [`thread::scope`](https://doc.rust-lang.org/std/thread/fn.scope.html).

## Keep locks and blocking work bounded

Acquire a lock only for the protected read or update. Release it before slow I/O
or `.await` whenever possible. Do not hold a blocking mutex guard across `.await`.
An async mutex can be held across an await when the operation requires it, but
it still serializes access and can participate in deadlocks. Keep lock ordering
consistent and avoid calling unknown callbacks while holding a lock. Define how
poisoned state is handled where the chosen lock can be poisoned.

A synchronous lock can be appropriate for a short, low-contention critical
section inside async code. Do not replace every lock with an async lock by habit.
Move blocking APIs or sustained CPU work off async workers using the runtime's
supported mechanism. In Tokio, `spawn_blocking` is one such mechanism; bound CPU
parallelism rather than launching unbounded blocking jobs. Started blocking jobs
cannot generally be stopped merely by aborting their handle.

References: [Tokio: shared state](https://tokio.rs/tokio/tutorial/shared-state) and
[`spawn_blocking`](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html).

### Example: copy a small snapshot before awaiting

This is a fragment inside an async function. It assumes `shared_title` is a
mutex containing a `String`, `send_title` is an existing async operation, and the
enclosing function handles the lock and send errors.

```rust
let title = {
    let guard = shared_title.lock()?;
    guard.clone()
};
send_title(title).await?;
```

The block makes the lock guard's end visible. Sending happens after the guard is
dropped. Copying a small title is a reasonable tradeoff when the operation needs
a snapshot, not a lock held over network I/O. It would not be correct for an
operation that must atomically reserve and update the same state; that requires
a different protocol. Do not move code outside a lock without checking what the
lock was protecting.

## Own task completion and cancellation

Identify who starts each task, observes its result, and shuts it down. Do not
assume dropping a task handle cancels or joins its work; check the actual API.
Await or join owned work, propagate useful failures, and put limits on queues,
in-flight requests, and retry attempts.

At each cancellation point, decide what happens to partially written output,
state updates, held resources, and remote requests. A timeout ending the wait is
not proof that external work stopped. Check cancellation safety when racing
futures, including with `select!`; losing work may be dropped before completion.
Use explicit async shutdown when completion requires it rather than expecting
an ordinary destructor to await cleanup.

References: [Tokio: spawning](https://tokio.rs/tokio/tutorial/spawning) and
[Tokio: select and cancellation](https://tokio.rs/tokio/tutorial/select).

## Verify observable behavior

Test completion, propagated errors, cancellation, shutdown, and important resource
limits. Synchronize tests with channels, barriers, or controlled clocks rather
than sleeps. Join or await work before a test ends so it cannot leak into another
test. A timeout can detect a stuck test; it should not be the synchronization
mechanism. For handwritten synchronization algorithms, use the project's supported
concurrency-testing tools and review the invariants; a stress test is not proof
that every interleaving is safe.
