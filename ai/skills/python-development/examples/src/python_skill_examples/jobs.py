"""Explicit job states and checked transitions."""

from dataclasses import dataclass, replace
from enum import Enum


class JobState(Enum):
    QUEUED = "queued"
    RUNNING = "running"
    SUCCEEDED = "succeeded"
    FAILED = "failed"


_ALLOWED_TRANSITIONS = {
    JobState.QUEUED: frozenset({JobState.RUNNING}),
    JobState.RUNNING: frozenset({JobState.SUCCEEDED, JobState.FAILED}),
    JobState.SUCCEEDED: frozenset(),
    JobState.FAILED: frozenset(),
}


@dataclass(frozen=True)
class Job:
    identifier: str
    state: JobState = JobState.QUEUED

    def transition_to(self, state: JobState) -> "Job":
        """Return the next job, rejecting unsupported transitions."""
        if state not in _ALLOWED_TRANSITIONS[self.state]:
            raise ValueError(
                f"cannot transition from {self.state.value} to {state.value}"
            )
        return replace(self, state=state)
