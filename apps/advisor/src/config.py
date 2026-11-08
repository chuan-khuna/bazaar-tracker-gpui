"""The Advisor's settings, from the environment and `apps/advisor/.env`.

Real environment variables win over `.env` (see `.env.example`). Changing ``base_url`` is all a
local OpenAI-compatible model needs: the key is then optional.

The key is ``OPENROUTER_API_KEY`` and nothing else (``ADVISOR_API_KEY`` was dropped: a stale value
in it once won over the real key, and OpenRouter answered 401 "Missing Authentication header").
Settings that look wrong are kept as ``notes`` for the CLI and the service to warn about; they never
include the key itself.
"""

from __future__ import annotations

import os
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path

from dotenv import load_dotenv

APP_DIR = Path(__file__).resolve().parent.parent
REPO_DIR = APP_DIR.parent.parent

OPENROUTER_BASE_URL = "https://openrouter.ai/api/v1"
DEFAULT_MODEL = "nvidia/nemotron-3.5-super-vl:free"
DEFAULT_PORT = 8420

# The key's variable.
API_KEY_VAR = "OPENROUTER_API_KEY"
# The old name for the key, no longer read.
OLD_API_KEY_VAR = "ADVISOR_API_KEY"
# Every OpenRouter key starts so; OpenRouter answers anything else with 401 "Missing Authentication header".
OPENROUTER_KEY_PREFIX = "sk-or-"
# Characters a copied key can carry that are never part of one: a byte-order mark and zero-width spaces.
_INVISIBLE = "﻿​‌‍⁠"
_QUOTES = ("'", '"')


@dataclass(frozen=True, slots=True)
class Config:
    base_url: str
    # None when no key is set: fine for a local model, an error for OpenRouter.
    api_key: str | None
    model: str
    port: int
    # The tracker's data folder: history.db, raw_data/.
    data_dir: Path
    # Warnings about settings that look wrong (never the key's value).
    notes: tuple[str, ...] = ()

    def __repr__(self) -> str:
        key = "set" if self.api_key else "None"
        return f"Config(base_url={self.base_url!r}, api_key={key}, model={self.model!r}, port={self.port})"

    @property
    def history_db(self) -> Path:
        return self.data_dir / "history.db"

    @property
    def is_openrouter(self) -> bool:
        return is_openrouter(self.base_url)

    @classmethod
    def from_env(cls, env: Mapping[str, str]) -> Config:
        """The settings in ``env``. An empty value counts as unset."""

        def get(key: str) -> str | None:
            value = env.get(key, "").strip()
            return value or None

        port_text = get("ADVISOR_PORT")
        try:
            port = int(port_text) if port_text else DEFAULT_PORT
        except ValueError:
            raise ValueError(f"ADVISOR_PORT must be a number, not {port_text!r}.") from None
        data_dir = get("BAZAAR_TRACKER_DATA_DIR")
        base_url = get("ADVISOR_BASE_URL") or OPENROUTER_BASE_URL
        api_key = clean_key(env.get(API_KEY_VAR, ""))
        notes = []
        if get(OLD_API_KEY_VAR):
            notes.append(
                f"{OLD_API_KEY_VAR} is no longer read: the key goes in {API_KEY_VAR}. "
                f"Remove {OLD_API_KEY_VAR} from apps/advisor/.env and the environment."
            )
        if is_openrouter(base_url) and api_key and not api_key.startswith(OPENROUTER_KEY_PREFIX):
            notes.append(
                f"{API_KEY_VAR} doesn't look like an OpenRouter key (they start with {OPENROUTER_KEY_PREFIX!r}); "
                'OpenRouter will answer 401 "Missing Authentication header". '
                "Copy the key again from https://openrouter.ai/settings/keys."
            )
        return cls(
            base_url=base_url,
            api_key=api_key,
            model=get("ADVISOR_MODEL") or DEFAULT_MODEL,
            port=port,
            data_dir=Path(data_dir) if data_dir else REPO_DIR / ".bazaar-tracker",
            notes=tuple(notes),
        )


def is_openrouter(base_url: str) -> bool:
    return base_url.rstrip("/") == OPENROUTER_BASE_URL


def clean_key(raw: str) -> str | None:
    """The key in ``raw`` without what copying can add around it: spaces, matching quotes (a real
    environment variable keeps them; `.env` drops them itself) and invisible characters. None when
    nothing is left."""
    key = "".join(ch for ch in raw if ch not in _INVISIBLE).strip()
    while len(key) >= 2 and key[0] == key[-1] and key[0] in _QUOTES:
        key = key[1:-1].strip()
    return key or None


def load() -> Config:
    """The settings: `apps/advisor/.env` loaded under the real environment, then read."""
    load_dotenv(APP_DIR / ".env")
    return Config.from_env(os.environ)
