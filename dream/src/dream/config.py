"""Read the machine's JSON config and build its adapters.

This is the only core file that imports adapters, so a new place needs
one adapter file and one name here.
"""

from __future__ import annotations

import datetime
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from dream.adapters.openai_model import OpenAIModel
from dream.adapters.opencode_sessions import OpenCodeSessions
from dream.adapters.pde_cards import PdeCards
from dream.forms import CARDS_PER_NIGHT, SESSIONS_PER_NIGHT
from dream.ports import Model, Sessions, WorkItems

DEFAULT_PATH = Path("~/.config/dream/config.json")
TOP_KEYS = {"state_dir", "stop_starting_at", "work_items", "sessions", "model"}
WORK_KEYS = {"adapter", "per_night"}
SESSION_KEYS = {
    "adapter",
    "per_night",
    "min_cloud_tokens",
    "db",
    "stats_script",
    "local_providers",
}
MODEL_KEYS = {"adapter", "base_url", "key_file", "name"}


class ConfigError(Exception):
    """A config ``dream`` cannot use; the message names the file and key."""


@dataclass(frozen=True)
class Config:
    """One machine's adapters, staging folder, deadline and caps.

    Attributes:
        work_items: None when this machine runs no form A.
        sessions: None when this machine runs no form B.
        stop_starting_at: No item starts after this time of day.
    """

    state_dir: Path
    stop_starting_at: datetime.time
    work_items: WorkItems | None
    sessions: Sessions | None
    model: Model
    model_name: str
    items_per_night: int
    sessions_per_night: int
    min_cloud_tokens: int


def load_config(path: Path) -> Config:
    """Read the machine's JSON config and build its adapters.

    Raises:
        ConfigError: The file is missing, is not JSON, has an unknown
            key, or names an unknown adapter. ``dream`` exits 2 with the
            message.
    """
    try:
        raw = json.loads(path.expanduser().read_text())
    except OSError as error:
        raise ConfigError(f"{path}: {error.strerror}") from error
    except ValueError as error:
        raise ConfigError(f"{path}: not JSON: {error}") from error
    if not isinstance(raw, dict):
        raise ConfigError(f"{path}: not a JSON object")
    check_keys(path, "", raw, TOP_KEYS)
    try:
        return build_config(path, raw)
    except (KeyError, TypeError, ValueError) as error:
        raise ConfigError(f"{path}: bad or missing value: {error}") from error


def build_config(path: Path, raw: dict[str, Any]) -> Config:
    """Build the adapters a checked config names."""
    work = raw.get("work_items")
    sessions = raw.get("sessions")
    model = raw["model"]
    work_items = None
    items_per_night = 0
    if work is not None:
        check_keys(path, "work_items.", work, WORK_KEYS)
        if work["adapter"] != "pde-cards":
            raise unknown_adapter(path, "work_items", work["adapter"])
        work_items = PdeCards()
        items_per_night = capped(path, "work_items", work, CARDS_PER_NIGHT)
    session_port = None
    sessions_per_night = 0
    if sessions is not None:
        check_keys(path, "sessions.", sessions, SESSION_KEYS)
        if sessions["adapter"] != "opencode":
            raise unknown_adapter(path, "sessions", sessions["adapter"])
        session_port = OpenCodeSessions(
            Path(sessions["db"]).expanduser(),
            Path(sessions["stats_script"]).expanduser(),
            tuple(sessions.get("local_providers", [])),
        )
        sessions_per_night = capped(
            path, "sessions", sessions, SESSIONS_PER_NIGHT
        )
    check_keys(path, "model.", model, MODEL_KEYS)
    if model["adapter"] != "openai-http":
        raise unknown_adapter(path, "model", model["adapter"])
    key_file = model.get("key_file")
    return Config(
        state_dir=Path(
            raw.get("state_dir", "~/.local/state/dream")
        ).expanduser(),
        stop_starting_at=datetime.time.fromisoformat(
            raw.get("stop_starting_at", "06:00")
        ),
        work_items=work_items,
        sessions=session_port,
        model=OpenAIModel(
            model["base_url"],
            Path(key_file).expanduser() if key_file else None,
            model["name"],
        ),
        model_name=model["name"],
        items_per_night=items_per_night,
        sessions_per_night=sessions_per_night,
        min_cloud_tokens=int(
            (sessions or {}).get("min_cloud_tokens", 1_000_000)
        ),
    )


def check_keys(
    path: Path, prefix: str, section: Any, allowed: set[str]
) -> None:
    """Refuse keys the config does not list, so a typo never hides."""
    if not isinstance(section, dict):
        raise ConfigError(f"{path}: {prefix.rstrip('.')} is not an object")
    extra = sorted(set(section) - allowed)
    if extra:
        raise ConfigError(f"{path}: unknown key {prefix}{extra[0]}")


def capped(path: Path, form: str, section: dict, default: int) -> int:
    """Return ``per_night``, which may lower the default but never raise it."""
    per_night = int(section.get("per_night", default))
    if not 0 < per_night <= default:
        raise ConfigError(f"{path}: {form}.per_night must be 1 to {default}")
    return per_night


def unknown_adapter(path: Path, port: str, name: object) -> ConfigError:
    """Return the error for an adapter name no file provides."""
    return ConfigError(f"{path}: unknown adapter {port}.adapter = {name}")
