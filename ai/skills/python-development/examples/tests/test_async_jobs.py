import asyncio
from collections.abc import Iterator

import pytest

from python_skill_examples.async_jobs import process_jobs


def test_work_and_tasks_stay_bounded() -> None:
    async def scenario() -> None:
        release = asyncio.Event()
        producer_blocked = asyncio.Event()
        pulled = 0
        active = 0
        most_active = 0
        worker_tasks: set[int] = set()

        def job_ids() -> Iterator[str]:
            nonlocal pulled
            for index in range(10):
                pulled += 1
                if pulled == 5:
                    producer_blocked.set()
                yield f"job-{index}"

        async def handle(job_id: str) -> None:
            nonlocal active, most_active
            task = asyncio.current_task()
            assert task is not None
            worker_tasks.add(id(task))
            active += 1
            most_active = max(most_active, active)
            try:
                await release.wait()
            finally:
                active -= 1

        processing = asyncio.create_task(
            process_jobs(
                job_ids(), worker_count=2, queue_size=2, handle=handle
            )
        )
        await producer_blocked.wait()

        assert pulled == 5
        assert active == 2
        assert len(worker_tasks) == 2

        release.set()
        await processing
        assert most_active == 2
        assert pulled == 10

    asyncio.run(scenario())


def test_worker_failure_cancels_siblings() -> None:
    async def scenario() -> None:
        waiting_started = asyncio.Event()
        waiting_cancelled = asyncio.Event()

        async def handle(job_id: str) -> None:
            if job_id == "waiting":
                waiting_started.set()
                try:
                    await asyncio.Event().wait()
                except asyncio.CancelledError:
                    waiting_cancelled.set()
                    raise
            await waiting_started.wait()
            raise RuntimeError("job failed")

        with pytest.raises(ExceptionGroup) as error:
            await process_jobs(
                ["waiting", "bad"],
                worker_count=2,
                queue_size=2,
                handle=handle,
            )

        assert any(
            isinstance(cause, RuntimeError) for cause in error.value.exceptions
        )
        assert waiting_cancelled.is_set()

    asyncio.run(scenario())
