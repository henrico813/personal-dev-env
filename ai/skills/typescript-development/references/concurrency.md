# Concurrency

Read this before changing promises, `async` code, timeouts,
cancellation, limits on parallel work, or CPU-heavy work.

## Contents

- [Know what async buys](#know-what-async-buys)
- [Choose sequential or parallel on
  purpose](#choose-sequential-or-parallel-on-purpose)
- [Limit work over large inputs](#limit-work-over-large-inputs)
- [Cancel with AbortSignal](#cancel-with-abortsignal)
- [Watch state across await](#watch-state-across-await)

## Know what async buys

One event loop runs JavaScript one piece at a time. `async` code waits
for many I/O operations at once, but CPU work still blocks everything
else on that loop, including timers and incoming requests. Move long CPU
work to `worker_threads` in Node or a Web Worker in browsers, or split
it into smaller chunks. Measure before claiming a speedup.

Use promise-based APIs, such as `node:fs/promises` and
`node:timers/promises`, instead of callbacks. Wrap an old callback API
once with `util.promisify`; do not mix callbacks and promises in one
function.

## Choose sequential or parallel on purpose

`await` inside a loop runs one item at a time. That is right when order
matters or the other side needs a gentle load. Use `Promise.all` for
independent work; it rejects on the first failure. Use
`Promise.allSettled` when every result matters, even after a failure.

`Promise.all` does not stop the other operations when one fails. Pass
them a shared `AbortSignal` and abort it on the first failure when the
rest are wasted work.

## Limit work over large inputs

`Promise.all(items.map(fetchItem))` starts every request at once. Over
thousands of items that can exhaust sockets, memory, or a rate limit.
Run a fixed number of workers that pull from one iterator:

```ts
async function mapWithLimit<T, R>(
  items: Iterable<T>,
  limit: number,
  work: (item: T) => Promise<R>,
): Promise<R[]> {
  if (!Number.isInteger(limit) || limit < 1) {
    throw new RangeError("limit must be a whole number of at least 1");
  }
  const results: R[] = [];
  const entries = Array.from(items).entries();
  async function runWorker(): Promise<void> {
    for (const [index, item] of entries) {
      results[index] = await work(item);
    }
  }
  await Promise.all(Array.from({ length: limit }, () => runWorker()));
  return results;
}
```

All workers share one iterator, so each item runs once. Use the
project's existing helper, such as `p-limit`, when it has one.

## Cancel with AbortSignal

Accept an `AbortSignal` in any API that does long or remote work, and
pass it to every call that accepts one: `fetch`, `node:timers/promises`
`setTimeout`, `fs.readFile`, `events.once`, and child processes. Between
steps of your own long work, call `signal.throwIfAborted()`.

- `AbortSignal.timeout(ms)` aborts with a `TimeoutError` after a delay.
- `AbortSignal.any([a, b])` aborts when any of its signals aborts, with
  that signal's reason. Use it to combine the caller's signal with a
  timeout, so neither is lost.
- `controller.abort()` without a reason aborts with an `AbortError`.

[fetch-json.ts](../examples/src/fetch-json.ts) combines a caller's
signal with a timeout, and its tests check both.

Remove abort listeners when the work finishes, so a long-lived signal
does not keep every past request in memory.

## Watch state across await

Other code runs while a function waits at `await`. A value read before
the `await` may be stale after it. Read shared state again after
waiting, or keep the check and the update in the same synchronous step.
When two calls must not overlap, keep one in-flight promise and return
it to later callers instead of starting another.
