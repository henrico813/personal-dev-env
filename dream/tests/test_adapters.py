"""Tests for the adapters, against stand-ins for their real sources."""

import json
import os
import sqlite3
import threading
import time
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from typing import ClassVar

import pytest

from dream.adapters.openai_model import OpenAIModel
from dream.adapters.opencode_sessions import OpenCodeSessions, parse_stats
from dream.adapters.pde_cards import PdeCards
from dream.ports import AdapterError, Cost

SAMPLES = Path(__file__).parent / "samples"


class TestParseStats:
    """``parse_stats`` reads ``session_stats.py``'s text."""

    def test_parse_stats_reads_each_block(self) -> None:
        """Each of the four blocks becomes costs with exact numbers.

        The OpenCode adapter depends on the script's text; a format
        change must fail here, not at night.
        """
        model, costs = parse_stats((SAMPLES / "stats-demo.txt").read_text())

        assert model == "cloud/big-model"
        assert Cost("tool", "shell", 52993, 3908032, 0, 116) in costs
        assert (
            Cost("output", "shell: git diff -M30%", 6637, 384946, 58, 1)
            in costs
        )
        assert Cost("skill", "demo-skill", 3149, 195238, 62, 1) in costs
        assert (
            Cost(
                "subagent",
                "Review the export fix on cloud/small-model",
                300000,
                0,
                0,
                1,
            )
            in costs
        )
        assert Cost("repeat", "make test", 0, 0, 0, 9) in costs

    def test_parse_stats_raises_on_unknown_line(self) -> None:
        """A block line of an unknown shape raises instead of guessing.

        A guessed parse would hand the model wrong numbers that the
        check then holds it to.
        """
        text = "carry by tool\n  shell calls many tokens 5 carry 9\n"

        with pytest.raises(AdapterError, match="session_stats line"):
            parse_stats(text)


def test_list_large_counts_subagent_cloud_tokens(tmp_path: Path) -> None:
    """A local-model root's cloud subagent counts; its local steps do not.

    Ranking by all tokens would spend the night on free sessions.
    """
    db = tmp_path / "opencode.db"
    con = sqlite3.connect(db)
    con.execute("create table session_v2 (id, parent_id, title)")
    con.execute(
        "create table session_message (session_id, type, time_created, data)"
    )
    con.executemany(
        "insert into session_v2 values (?, ?, ?)",
        [("root", None, "demo"), ("child", "root", "sub")],
    )
    old = int(time.time() * 1000) - 2 * 3600 * 1000

    def step(sid: str, provider: str, tokens: int) -> tuple:
        """One assistant step on ``provider`` that read ``tokens``."""
        data = {
            "model": {"providerID": provider},
            "tokens": {"input": tokens, "cache": {"read": 0, "write": 0}},
        }
        return (sid, "assistant", old, json.dumps(data))

    con.executemany(
        "insert into session_message values (?, ?, ?, ?)",
        [step("root", "goog", 900), step("child", "cloud", 300)],
    )
    con.commit()
    con.close()
    sessions = OpenCodeSessions(db, Path("unused"), ("goog",))

    [summary] = sessions.list_large(24, 100)

    assert (summary.id, summary.cloud_tokens, summary.tokens_read) == (
        "root",
        300,
        1200,
    )
    assert summary.idle_minutes >= 119


@pytest.fixture
def open_stdin():
    """File descriptor 0 as an open pipe for the length of one test."""
    # pytest points fd 0 at /dev/null, which would hide a child that
    # inherits stdin.
    read_end, write_end = os.pipe()
    saved = os.dup(0)
    os.dup2(read_end, 0)
    yield
    os.dup2(saved, 0)
    for fd in (saved, read_end, write_end):
        os.close(fd)


@pytest.mark.usefixtures("open_stdin")
def test_list_drafts_reads_card_without_stdin(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Drafts come from ``card`` output, with criteria and dates read.

    The fake ``card`` fails when stdin is open, because ``opencode
    run`` once read a script from stdin into its prompt.
    """
    card = tmp_path / "card"
    card.write_text(
        "#!/bin/sh\n"
        '[ "$(readlink /proc/$$/fd/0)" = /dev/null ] ||'
        " { echo stdin open >&2; exit 3; }\n"
        'case "$*" in\n'
        '  "list --stage draft --tabs")\n'
        '    printf "DEMO-1\\tdraft\\tbuild\\tDemo\\tFix export\\n" ;;\n'
        '  *Acceptance*) echo "- [ ] works" ;;\n'
        '  *) printf "date_created: 2026-10-01\\nexport crashes\\n" ;;\n'
        "esac\n"
    )
    card.chmod(0o755)
    monkeypatch.setenv("PATH", f"{tmp_path}:/usr/bin:/bin")

    [item] = PdeCards().list_drafts()

    assert (item.key, item.title, str(item.created), item.has_criteria) == (
        "DEMO-1",
        "Fix export",
        "2026-10-01",
        True,
    )
    assert "export crashes" in item.text


class StubServer(BaseHTTPRequestHandler):
    """A model server that records requests and gives a set reply."""

    reply: tuple[int, dict] = (200, {})
    bodies: ClassVar[list[dict]] = []

    def do_POST(self) -> None:
        """Record the request body and send the set reply."""
        length = int(self.headers["Content-Length"])
        StubServer.bodies.append(json.loads(self.rfile.read(length)))
        status, body = StubServer.reply
        if status == 0:
            time.sleep(2)
            return
        self.send_response(status)
        self.end_headers()
        self.wfile.write(json.dumps(body).encode())

    def log_message(self, *args: object) -> None:
        """Keep test output quiet."""


@pytest.fixture
def stub_url():
    """The URL of a running stub model server."""
    server = HTTPServer(("127.0.0.1", 0), StubServer)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    StubServer.bodies = []
    yield f"http://127.0.0.1:{server.server_port}/v1"
    server.shutdown()


def test_openai_model_sends_no_tools(stub_url: str, tmp_path: Path) -> None:
    """The request holds no tools, and prompt tokens become tokens read.

    A ``tools`` field would hand the model tools again with no visible
    change anywhere else.
    """
    key = tmp_path / "key"
    key.write_text("k\n")
    StubServer.reply = (
        200,
        {
            "choices": [{"message": {"content": "OK"}}],
            "usage": {"prompt_tokens": 54},
        },
    )

    answer = OpenAIModel(stub_url, key, "local").ask("reply OK", timeout=5)

    assert (answer.text, answer.tokens_read, answer.failure) == ("OK", 54, "")
    assert set(StubServer.bodies[0]) == {"model", "messages", "max_tokens"}


@pytest.mark.parametrize(
    ("reply", "failure"),
    [
        pytest.param((500, {}), "http 500", id="server-error"),
        pytest.param((502, {}), "http 502", id="gateway-error"),
        pytest.param(
            (200, {"choices": [{"message": {"content": ""}}]}),
            "empty answer",
            id="empty",
        ),
        pytest.param((0, {}), "timed out", id="timeout"),
    ],
)
def test_openai_model_turns_errors_into_failures(
    stub_url: str, reply: tuple, failure: str
) -> None:
    """Every server failure gives an Answer with ``failure`` set.

    An exception would end the night instead of one item.
    """
    StubServer.reply = reply

    answer = OpenAIModel(stub_url, None, "local").ask("x", timeout=1)

    assert failure in answer.failure
