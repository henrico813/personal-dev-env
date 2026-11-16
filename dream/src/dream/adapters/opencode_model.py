"""A cloud model through ``opencode run``, for models only OpenCode reaches.

OpenCode's agent has tools that ``dream`` cannot turn off, so the prompt
asks for an answer only and an answer that used a tool fails.
"""

from __future__ import annotations

import json
import subprocess
import tempfile
import time

from dream.ports import Answer


class OpenCodeModel:
    """One model that ``opencode run`` reaches, such as a Claude model."""

    name = "opencode-run"

    def __init__(self, model: str, agent: str | None):
        """Keep the model name and the OpenCode agent, if the config sets one.

        Args:
            model: The model as ``opencode models`` lists it.
            agent: An OpenCode agent, such as one whose tools are all
                denied; None runs the default agent.
        """
        self.model = model
        self.agent = agent

    def ask(self, prompt: str, timeout: int) -> Answer:
        """Run ``opencode run`` once and read the text parts of its JSON.

        It runs with stdin closed, in a new empty folder, so the agent
        reads no project rules and finds no files. ``tokens_read`` is 0,
        because ``opencode run`` reports tokens for some steps only. A
        tool call, an error event, a non-zero exit, a timeout or an
        empty answer gives an Answer with ``failure`` set.
        """
        command = ["opencode", "run", "--standalone", "--format", "json"]
        command += ["-m", self.model]
        if self.agent:
            command += ["--agent", self.agent]
        start = time.monotonic()
        try:
            with tempfile.TemporaryDirectory() as folder:
                done = subprocess.run(
                    [*command, prompt],
                    stdin=subprocess.DEVNULL,
                    capture_output=True,
                    text=True,
                    timeout=timeout,
                    cwd=folder,
                    check=False,
                )
        except subprocess.TimeoutExpired:
            return failed("timed out", start)
        except OSError as error:
            return failed(f"{type(error).__name__}: {error}"[:200], start)
        if done.returncode != 0:
            lines = done.stderr.strip().splitlines() or [""]
            return failed(f"exit {done.returncode}: {lines[-1]}"[:200], start)
        text, failure = read_events(done.stdout)
        if not failure and not text.strip():
            failure = "empty answer"
        seconds = round(time.monotonic() - start, 1)
        return Answer(text, 0, seconds, failure)


def read_events(output: str) -> tuple[str, str]:
    """Return the joined text parts and the first failure in the events."""
    texts = []
    for line in output.splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if not isinstance(event, dict):
            continue
        part = event.get("part") or {}
        if event.get("type") == "tool_use":
            return "", f"used a tool: {part.get('tool')}"
        if event.get("type") == "error":
            return "", f"error: {event.get('error')}"[:200]
        if event.get("type") == "text":
            texts.append(part.get("text") or "")
    return "".join(texts), ""


def failed(reason: str, start: float) -> Answer:
    """Return an Answer for a call that gave no usable reply."""
    return Answer("", 0, round(time.monotonic() - start, 1), reason)
