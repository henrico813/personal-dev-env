"""A local model behind an OpenAI-style HTTP server, called with no tools."""

from __future__ import annotations

import json
import time
import urllib.error
import urllib.request
from pathlib import Path

from dream.ports import Answer

MAX_TOKENS = 16000


class OpenAIModel:
    """One model on an OpenAI-style ``/chat/completions`` server."""

    name = "openai-http"

    def __init__(self, base_url: str, key_file: Path | None, model: str):
        """Keep the server, the key file and the model name, not the key."""
        self.base_url = base_url.rstrip("/")
        self.key_file = key_file
        self.model = model

    def ask(self, prompt: str, timeout: int) -> Answer:
        """POST one user message to ``/chat/completions``, with no tools.

        The request holds only ``model``, the message and ``max_tokens``.
        With no ``tools`` field the server has nothing to call, so the
        model cannot run anything. ``tokens_read`` is the reply's
        ``usage.prompt_tokens``. A network error, a non-200 status or an
        empty ``content`` gives an Answer with ``failure`` set.
        """
        body = {
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "messages": [{"role": "user", "content": prompt}],
        }
        headers = {"Content-Type": "application/json"}
        start = time.monotonic()
        try:
            if self.key_file is not None:
                key = self.key_file.read_text().strip()
                headers["Authorization"] = f"Bearer {key}"
            request = urllib.request.Request(
                f"{self.base_url}/chat/completions",
                json.dumps(body).encode(),
                headers,
            )
            with urllib.request.urlopen(request, timeout=timeout) as response:
                reply = json.load(response)
        except urllib.error.HTTPError as error:
            return failed(f"http {error.code}", start)
        except (OSError, ValueError) as error:
            return failed(f"{type(error).__name__}: {error}"[:200], start)
        seconds = round(time.monotonic() - start, 1)
        try:
            text = reply["choices"][0]["message"].get("content") or ""
        except (KeyError, IndexError, TypeError, AttributeError):
            text = ""
        usage = reply.get("usage") if isinstance(reply, dict) else None
        tokens = (usage or {}).get("prompt_tokens") or 0
        failure = "" if text.strip() else "empty answer"
        return Answer(text, tokens, seconds, failure)


def failed(reason: str, start: float) -> Answer:
    """Return an Answer for a call that got no reply."""
    return Answer("", 0, round(time.monotonic() - start, 1), reason)
