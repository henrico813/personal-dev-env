import pytest

from python_skill_examples.jobs import Job, JobState


@pytest.mark.parametrize(
    ("start", "destination"),
    [
        pytest.param(JobState.QUEUED, JobState.RUNNING, id="start"),
        pytest.param(JobState.RUNNING, JobState.SUCCEEDED, id="succeed"),
        pytest.param(JobState.RUNNING, JobState.FAILED, id="fail"),
    ],
)
def test_allows_supported_transitions(
    start: JobState, destination: JobState
) -> None:
    original = Job("build-7", start)

    changed = original.transition_to(destination)

    assert changed == Job("build-7", destination)
    assert original == Job("build-7", start)


@pytest.mark.parametrize(
    ("start", "destination"),
    [
        pytest.param(JobState.QUEUED, JobState.SUCCEEDED, id="skip-running"),
        pytest.param(
            JobState.SUCCEEDED, JobState.RUNNING, id="restart-finished"
        ),
        pytest.param(JobState.FAILED, JobState.FAILED, id="repeat-failure"),
    ],
)
def test_rejects_unsupported_transitions(
    start: JobState, destination: JobState
) -> None:
    with pytest.raises(ValueError, match="cannot transition"):
        Job("build-7", start).transition_to(destination)
