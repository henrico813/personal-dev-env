"""Bounded asynchronous job processing."""

import asyncio
from collections.abc import Awaitable, Callable, Iterable

JobHandler = Callable[[str], Awaitable[None]]
_STOP: None = None


async def process_jobs(
    job_ids: Iterable[str],
    *,
    worker_count: int,
    queue_size: int,
    handle: JobHandler,
) -> None:
    """Process jobs with fixed workers and bounded pending work.

    A worker failure cancels the producer and sibling workers through the task
    group. ``worker_count`` and ``queue_size`` must both be positive.
    """
    if worker_count < 1:
        raise ValueError("worker_count must be positive")
    if queue_size < 1:
        raise ValueError("queue_size must be positive")

    queue: asyncio.Queue[str | None] = asyncio.Queue(maxsize=queue_size)

    async def produce() -> None:
        for job_id in job_ids:
            await queue.put(job_id)
        for _ in range(worker_count):
            await queue.put(_STOP)

    async def consume() -> None:
        while True:
            item = await queue.get()
            try:
                if item is _STOP:
                    return
                await handle(item)
            finally:
                queue.task_done()

    async with asyncio.TaskGroup() as group:
        group.create_task(produce(), name="job-producer")
        for index in range(worker_count):
            group.create_task(consume(), name=f"job-worker-{index}")
