"""The config, and the provider settings it gives: OpenRouter by default, a local model by base_url."""

from __future__ import annotations

from pathlib import Path

import pytest

from agents.core.provider import provider_config
from config import DEFAULT_MODEL, OPENROUTER_BASE_URL, REPO_DIR, Config


def test_defaults() -> None:
    config = Config.from_env({})
    assert config.base_url == OPENROUTER_BASE_URL
    assert config.api_key is None
    assert config.model == DEFAULT_MODEL
    assert config.port == 8420
    assert config.data_dir == REPO_DIR / ".bazaar-tracker"
    assert config.history_db == REPO_DIR / ".bazaar-tracker" / "history.db"
    assert (REPO_DIR / "apps" / "advisor").is_dir()


def test_reads_the_environment_and_treats_empty_as_unset(tmp_path: Path) -> None:
    config = Config.from_env(
        {
            "ADVISOR_BASE_URL": "http://localhost:11434/v1",
            "ADVISOR_API_KEY": "",
            "OPENROUTER_API_KEY": "or-key",
            "ADVISOR_MODEL": "qwen3",
            "ADVISOR_PORT": "9000",
            "BAZAAR_TRACKER_DATA_DIR": str(tmp_path),
        }
    )
    assert (config.base_url, config.api_key, config.model, config.port) == (
        "http://localhost:11434/v1",
        "or-key",
        "qwen3",
        9000,
    )
    assert config.data_dir == tmp_path
    assert Config.from_env({"BAZAAR_TRACKER_DATA_DIR": "  "}).data_dir == REPO_DIR / ".bazaar-tracker"


def test_a_bad_port_is_an_error() -> None:
    with pytest.raises(ValueError, match="ADVISOR_PORT"):
        Config.from_env({"ADVISOR_PORT": "eighty"})


def test_openrouter_needs_a_key() -> None:
    with pytest.raises(RuntimeError, match="ADVISOR_API_KEY"):
        provider_config(Config.from_env({}))

    settings = provider_config(Config.from_env({"ADVISOR_API_KEY": "k"}))
    assert (settings.base_url, settings.api_key, settings.thinking_format) == (OPENROUTER_BASE_URL, "k", "openrouter")
    assert not settings.omit_authorization_header


def test_changing_base_url_is_enough_for_a_local_model() -> None:
    settings = provider_config(Config.from_env({"ADVISOR_BASE_URL": "http://localhost:1234/v1"}))
    assert settings.base_url == "http://localhost:1234/v1"
    assert settings.omit_authorization_header
    assert settings.thinking_format == "openai"
