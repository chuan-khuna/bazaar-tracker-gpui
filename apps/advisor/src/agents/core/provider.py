"""The model provider: any OpenAI-compatible endpoint through tau_ai's adapter (copied from tau-app).

``base_url`` comes from the config: OpenRouter by default, or a local model such as Ollama or
LM Studio. OpenRouter needs a key; another endpoint may have none, and then no Authorization
header is sent.
"""

from __future__ import annotations

from tau_ai import OpenAICompatibleConfig, OpenAICompatibleProvider

from config import Config


def provider_from(config: Config) -> OpenAICompatibleProvider:
    """A provider for the endpoint in ``config``."""
    return OpenAICompatibleProvider(provider_config(config))


def provider_config(config: Config) -> OpenAICompatibleConfig:
    """The adapter's settings for ``config``'s endpoint."""
    if config.is_openrouter:
        if not config.api_key:
            raise RuntimeError(
                "No API key for OpenRouter. Copy apps/advisor/.env.example to apps/advisor/.env and set "
                "ADVISOR_API_KEY, or set ADVISOR_BASE_URL to a local OpenAI-compatible model."
            )
        return OpenAICompatibleConfig(
            api_key=config.api_key,
            base_url=config.base_url,
            provider_name="OpenRouter",
            thinking_format="openrouter",
            # Same compat flags tau_coding derives for OpenRouter: no `store`
            # field, usage in the stream, and `max_tokens` for the limit.
            compat={
                "supportsStore": False,
                "supportsUsageInStreaming": True,
                "maxTokensField": "max_tokens",
            },
            # Free models can be slow to start streaming.
            timeout_seconds=120.0,
        )
    return OpenAICompatibleConfig(
        api_key=config.api_key or "",
        base_url=config.base_url,
        provider_name="OpenAI-compatible",
        omit_authorization_header=not config.api_key,
        # A local model can take a while to load before it streams.
        timeout_seconds=300.0,
    )
