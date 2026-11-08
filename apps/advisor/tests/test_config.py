"""The config, and the provider settings it gives: OpenRouter by default, a local model by base_url.

The key comes from OPENROUTER_API_KEY only. The fake keys here are made up; none is a real key."""

from __future__ import annotations

import asyncio
import os
from pathlib import Path

import httpx
import pytest
from tau_ai import OpenAICompatibleProvider

import config
from agents.core.provider import provider_config
from config import DEFAULT_MODEL, OPENROUTER_BASE_URL, REPO_DIR, Config, clean_key

FAKE_KEY = "sk-or-v1-fake"


def test_defaults() -> None:
    config = Config.from_env({})
    assert config.base_url == OPENROUTER_BASE_URL
    assert config.api_key is None
    assert config.model == DEFAULT_MODEL
    assert config.port == 8420
    assert config.data_dir == REPO_DIR / ".bazaar-tracker"
    assert config.history_db == REPO_DIR / ".bazaar-tracker" / "history.db"
    assert config.notes == ()
    assert (REPO_DIR / "apps" / "advisor").is_dir()


def test_reads_the_environment_and_treats_empty_as_unset(tmp_path: Path) -> None:
    config = Config.from_env(
        {
            "ADVISOR_BASE_URL": "http://localhost:11434/v1",
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
    assert Config.from_env({"OPENROUTER_API_KEY": "  "}).api_key is None


def test_the_key_comes_from_openrouter_api_key_only() -> None:
    assert Config.from_env({"OPENROUTER_API_KEY": FAKE_KEY}).api_key == FAKE_KEY
    # The old name is not read, even when it is the only one set; a note says so.
    old_only = Config.from_env({"ADVISOR_API_KEY": FAKE_KEY})
    assert old_only.api_key is None
    assert any("ADVISOR_API_KEY is no longer read" in note for note in old_only.notes)
    # The bug: a placeholder in ADVISOR_API_KEY won over the real key and OpenRouter answered 401.
    both = Config.from_env({"ADVISOR_API_KEY": "your-key-here", "OPENROUTER_API_KEY": FAKE_KEY})
    assert both.api_key == FAKE_KEY


def test_a_copied_key_loses_spaces_quotes_and_invisible_characters() -> None:
    assert clean_key(f"  {FAKE_KEY}\n") == FAKE_KEY
    assert clean_key(f'"{FAKE_KEY}"') == FAKE_KEY
    assert clean_key(f"' {FAKE_KEY} '") == FAKE_KEY
    assert clean_key(f"﻿{FAKE_KEY}​") == FAKE_KEY
    assert clean_key('""') is None
    assert clean_key("") is None
    assert Config.from_env({"OPENROUTER_API_KEY": f'"{FAKE_KEY}"'}).api_key == FAKE_KEY


def test_a_key_that_is_not_an_openrouter_key_is_noted_without_its_value() -> None:
    odd = Config.from_env({"OPENROUTER_API_KEY": "sk-ant-not-openrouter"})
    assert odd.api_key == "sk-ant-not-openrouter"
    assert len(odd.notes) == 1 and "doesn't look like an OpenRouter key" in odd.notes[0]
    assert "sk-ant" not in odd.notes[0]
    # A local model's key can look like anything.
    assert Config.from_env({"ADVISOR_BASE_URL": "http://localhost:1234/v1", "OPENROUTER_API_KEY": "x"}).notes == ()


def test_the_key_never_shows_in_the_config_repr() -> None:
    assert FAKE_KEY not in repr(Config.from_env({"OPENROUTER_API_KEY": FAKE_KEY}))


def test_a_bad_port_is_an_error() -> None:
    with pytest.raises(ValueError, match="ADVISOR_PORT"):
        Config.from_env({"ADVISOR_PORT": "eighty"})


def test_openrouter_needs_a_key() -> None:
    with pytest.raises(RuntimeError, match="OPENROUTER_API_KEY"):
        provider_config(Config.from_env({}))

    settings = provider_config(Config.from_env({"OPENROUTER_API_KEY": "k"}))
    assert (settings.base_url, settings.api_key, settings.thinking_format) == (OPENROUTER_BASE_URL, "k", "openrouter")
    assert not settings.omit_authorization_header


def test_changing_base_url_is_enough_for_a_local_model() -> None:
    settings = provider_config(Config.from_env({"ADVISOR_BASE_URL": "http://localhost:1234/v1"}))
    assert settings.base_url == "http://localhost:1234/v1"
    assert settings.omit_authorization_header
    assert settings.thinking_format == "openai"


def write_env(folder: Path, text: str) -> None:
    (folder / ".env").write_text(text, encoding="utf-8")


def test_load_reads_the_env_file_under_the_real_environment(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(config, "APP_DIR", tmp_path)
    write_env(tmp_path, f'ADVISOR_API_KEY=placeholder\nOPENROUTER_API_KEY="{FAKE_KEY}"\nADVISOR_MODEL=from-file\n')
    monkeypatch.setattr(os, "environ", {"ADVISOR_MODEL": "from-env"})
    settings = config.load()
    assert settings.api_key == FAKE_KEY
    assert settings.model == "from-env"


def sent_authorization(settings: Config) -> str | None:
    """The Authorization header the provider sends for ``settings``, caught before it leaves."""
    seen: list[str | None] = []

    def answer(request: httpx.Request) -> httpx.Response:
        seen.append(request.headers.get("authorization"))
        return httpx.Response(401, json={"error": {"message": "test"}})

    async def call() -> None:
        client = httpx.AsyncClient(transport=httpx.MockTransport(answer))
        provider = OpenAICompatibleProvider(provider_config(settings), client=client)
        async for _ in provider.stream_response(model="m", system="s", messages=[], tools=[]):
            pass
        await client.aclose()

    asyncio.run(call())
    assert len(seen) == 1
    return seen[0]


def test_the_provider_sends_the_key_as_a_bearer_header(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(config, "APP_DIR", tmp_path)
    write_env(tmp_path, f"ADVISOR_API_KEY=placeholder\nOPENROUTER_API_KEY= {FAKE_KEY} \n")
    monkeypatch.setattr(os, "environ", {})
    assert sent_authorization(config.load()) == f"Bearer {FAKE_KEY}"
    # A local model without a key gets no header at all.
    assert sent_authorization(Config.from_env({"ADVISOR_BASE_URL": "http://localhost:1234/v1"})) is None
