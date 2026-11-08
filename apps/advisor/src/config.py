"""The Advisor's settings, from the environment and `apps/advisor/.env`.

Real environment variables win over `.env` (see `.env.example`). Changing ``base_url`` is all a
local OpenAI-compatible model needs: the key is then optional.
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


@dataclass(frozen=True, slots=True)
class Config:
    base_url: str
    # None when no key is set: fine for a local model, an error for OpenRouter.
    api_key: str | None
    model: str
    port: int
    # The tracker's data folder: history.db, raw_data/.
    data_dir: Path

    @property
    def history_db(self) -> Path:
        return self.data_dir / "history.db"

    @property
    def is_openrouter(self) -> bool:
        return self.base_url.rstrip("/") == OPENROUTER_BASE_URL

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
        return cls(
            base_url=get("ADVISOR_BASE_URL") or OPENROUTER_BASE_URL,
            api_key=get("ADVISOR_API_KEY") or get("OPENROUTER_API_KEY"),
            model=get("ADVISOR_MODEL") or DEFAULT_MODEL,
            port=port,
            data_dir=Path(data_dir) if data_dir else REPO_DIR / ".bazaar-tracker",
        )


def load() -> Config:
    """The settings: `apps/advisor/.env` loaded under the real environment, then read."""
    load_dotenv(APP_DIR / ".env")
    return Config.from_env(os.environ)
