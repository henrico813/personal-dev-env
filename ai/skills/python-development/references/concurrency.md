# Async, threads, and processes

Read this before adding or reviewing async functions, tasks, queues, threads,
processes, locks, or shared mutable state.

## Start with workload and ownership

State what can grow: requests, jobs, data, memory, latency, queued work, or active
operations. Measure the suspected bottleneck when practical. Prefer better
algorithms, fewer I/O calls, batching, or streaming before concurrency.

Every background task, thread, and process needs an owner that observes results
and defines completion, cancellation, timeout, failure propagation, shutdown,
and cleanup. Fire-and-forget work is still owned by the process and can still
fail.

> Save a reference to the result of this function, to avoid a task disappearing mid-execution.

— [Python documentation, Creating tasks](https://docs.python.org/3/library/asyncio-task.html#creating-tasks)

## Keep event loops nonblocking

Do not call blocking I/O directly from a coroutine. Use the async API supplied
by the project's libraries or the runtime's supported thread handoff. Sustained
CPU work may need a process pool, native code, or a different architecture.
Moving work off the loop does not make cancellation undo an external effect.

## Bound accepted and active work

`TaskGroup` owns and awaits its children and propagates grouped failures. It does
not bound how many tasks a producer creates. A semaphore around one HTTP call
limits that region; tasks waiting to acquire the semaphore still exist and use
memory.

Use incremental input and a fixed worker set when accepted work can grow. A
bounded queue applies backpressure:

> If it is an integer greater than `0`, then `await put()` blocks when the queue reaches `maxsize` until an item is removed by `get()`.

— [Python documentation, asyncio.Queue](https://docs.python.org/3/library/asyncio-queue.html#asyncio.Queue)

The [async example](../examples/src/python_skill_examples/async_jobs.py) uses one
producer, a bounded queue, fixed workers, and a task group. Its tests synchronize
with events rather than sleeps.

## Handle cancellation and partial effects

Use `try/finally` around cleanup that must survive cancellation. Propagate
`CancelledError` after cleanup. Define whether a timeout stops only the wait or
also the underlying operation. Account for partially written output, remote
requests, state updates, and queue accounting.

Keep task references until results are observed. When one child failure should
cancel siblings, use the runtime's structured mechanism and test that behavior
deterministically.

## Choose threads or processes from facts

> The asynchronous execution can be performed with threads ... or separate processes.

— [Python documentation, concurrent.futures](https://docs.python.org/3/library/concurrent.futures.html#module-concurrent.futures)

Threads often fit blocking I/O. Processes can bypass the ordinary CPython GIL
for CPU work but require picklable inputs and results and impose startup and IPC
cost. Extensions may release the GIL, alternate interpreters differ, and
free-threaded CPython builds are not the default. Do not say Python threads can
never execute in parallel.

Protect shared mutable state with a clear lock or message boundary. Keep lock
scope short, avoid blocking calls while holding it, and use a consistent order
when several locks are required. Prefer isolated process state when sharing is
not needed.

## Verify observable behavior

Test result propagation, sibling cancellation, shutdown, queue and worker bounds,
and cleanup. Use events, barriers, channels, or controlled clocks rather than
arbitrary sleeps. Await or join work before a test ends. Stress runs can reveal
races but do not prove every interleaving correct.
