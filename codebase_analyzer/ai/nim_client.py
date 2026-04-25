"""
ai/nim_client.py
Phase 6: NVIDIA NIM API client.

Uses the OpenAI-compatible client pointed at NVIDIA's inference endpoint.
Supports streaming responses, automatic model fallback, rate-limit retries,
and a connection health check.

Public API:
    client = NIMClient(api_key)
    async for chunk in client.stream(messages): yield chunk
    response = await client.complete(messages)  -> str
    ok, model = await client.health_check()     -> (bool, str)
"""

import asyncio
import time
from typing import AsyncIterator

from utils.logger import get_logger

log = get_logger(__name__)

# ─── Config ───────────────────────────────────────────────────────────────────

NIM_BASE_URL   = "https://integrate.api.nvidia.com/v1"

# Primary model — large, high quality
MODEL_PRIMARY  = "meta/llama-3.1-70b-instruct"
# Fallback — faster, lighter
MODEL_FALLBACK = "microsoft/phi-3.5-mini-instruct"

MAX_TOKENS     = 1024
TEMPERATURE    = 0.3          # low = more factual, less hallucination
MAX_RETRIES    = 3
RETRY_DELAYS   = [1.0, 2.0, 4.0]   # exponential-ish back-off
REQUEST_TIMEOUT = 30           # seconds per streaming chunk window


# ─── Exceptions ───────────────────────────────────────────────────────────────

class NIMError(Exception):
    """Base error for NIM client failures."""

class NIMConnectionError(NIMError):
    """Cannot reach the NVIDIA NIM endpoint."""

class NIMAuthError(NIMError):
    """Invalid or missing API key."""

class NIMRateLimitError(NIMError):
    """Rate limit hit — caller should back off."""

class NIMModelError(NIMError):
    """Model unavailable or returned an error."""


# ─── Client ───────────────────────────────────────────────────────────────────

class NIMClient:
    """
    Async wrapper around the OpenAI-compatible NVIDIA NIM API.

    Usage:
        client = NIMClient(api_key="nvapi-xxxx")

        # Streaming (for chat UI)
        async for chunk in client.stream(messages):
            print(chunk, end="", flush=True)

        # Non-streaming (for programmatic use)
        response = await client.complete(messages)

        # Check connection before showing AI panel
        ok, model_name = await client.health_check()
    """

    def __init__(self, api_key: str, preferred_model: str = MODEL_PRIMARY):
        if not api_key or not api_key.strip():
            raise NIMAuthError("API key must not be empty")

        self._api_key = api_key.strip()
        self._preferred_model = preferred_model
        self._active_model    = preferred_model
        self._client          = None   # lazy-init in _get_client()

        log.info("NIMClient initialised, model=%s", preferred_model)

    # ── Lazy client init ──────────────────────────────────────────────────────

    def _get_client(self):
        """Lazy-initialise the OpenAI client (avoids import errors at startup)."""
        if self._client is None:
            try:
                from openai import AsyncOpenAI
            except ImportError:
                raise NIMConnectionError(
                    "openai package not installed. Run: pip install openai"
                )
            self._client = AsyncOpenAI(
                api_key=self._api_key,
                base_url=NIM_BASE_URL,
                timeout=REQUEST_TIMEOUT,
            )
        return self._client

    # ── Public: streaming ────────────────────────────────────────────────────

    async def stream(
        self,
        messages: list[dict],
        system_prompt: str | None = None,
    ) -> AsyncIterator[str]:
        """
        Yield response text chunks as they arrive from the API.
        Automatically falls back to MODEL_FALLBACK if primary fails.

        Args:
            messages:      list of {"role": "user"/"assistant", "content": "..."}
            system_prompt: optional system message prepended to messages

        Yields:
            str — each text delta as it streams in
        """
        full_messages = self._build_messages(messages, system_prompt)

        for attempt in range(MAX_RETRIES):
            try:
                async for chunk in self._stream_once(full_messages):
                    yield chunk
                return  # success — stop retrying

            except NIMRateLimitError:
                if attempt < MAX_RETRIES - 1:
                    delay = RETRY_DELAYS[attempt]
                    log.warning("Rate limit hit, retrying in %.1fs (attempt %d)", delay, attempt + 1)
                    await asyncio.sleep(delay)
                else:
                    raise

            except NIMModelError as e:
                if self._active_model == MODEL_PRIMARY:
                    log.warning("Primary model failed (%s), switching to fallback %s", e, MODEL_FALLBACK)
                    self._active_model = MODEL_FALLBACK
                    # retry immediately with fallback
                else:
                    raise

            except NIMAuthError:
                raise   # never retry auth errors

            except NIMConnectionError:
                if attempt < MAX_RETRIES - 1:
                    delay = RETRY_DELAYS[attempt]
                    log.warning("Connection error, retrying in %.1fs", delay)
                    await asyncio.sleep(delay)
                else:
                    raise

    async def _stream_once(self, messages: list[dict]) -> AsyncIterator[str]:
        client = self._get_client()
        try:
            stream = await client.chat.completions.create(
                model=self._active_model,
                messages=messages,
                max_tokens=MAX_TOKENS,
                temperature=TEMPERATURE,
                stream=True,
            )
            async for chunk in stream:
                delta = chunk.choices[0].delta.content if chunk.choices else None
                if delta:
                    yield delta

        except Exception as e:
            self._classify_and_raise(e)

    # ── Public: non-streaming ────────────────────────────────────────────────

    async def complete(
        self,
        messages: list[dict],
        system_prompt: str | None = None,
    ) -> str:
        """
        Collect full streaming response into a single string.
        Use this for programmatic calls that don't need incremental display.
        """
        parts: list[str] = []
        async for chunk in self.stream(messages, system_prompt):
            parts.append(chunk)
        return "".join(parts)

    # ── Public: health check ─────────────────────────────────────────────────

    async def health_check(self) -> tuple[bool, str]:
        """
        Send a tiny test message to confirm the API key and model work.
        Returns (success: bool, model_name: str).
        """
        try:
            client = self._get_client()
            resp = await client.chat.completions.create(
                model=self._active_model,
                messages=[{"role": "user", "content": "Reply with the single word: ready"}],
                max_tokens=10,
                temperature=0.0,
                stream=False,
            )
            reply = resp.choices[0].message.content or ""
            log.info("Health check passed: model=%s reply=%r", self._active_model, reply)
            return True, self._active_model

        except NIMAuthError:
            log.error("Health check failed: auth error")
            return False, "auth_error"

        except NIMModelError:
            # Try fallback
            if self._active_model == MODEL_PRIMARY:
                log.warning("Primary model health check failed, trying fallback")
                self._active_model = MODEL_FALLBACK
                return await self.health_check()
            return False, "model_error"

        except Exception as e:
            log.error("Health check failed: %s", e)
            return False, str(e)

    # ── Properties ───────────────────────────────────────────────────────────

    @property
    def active_model(self) -> str:
        return self._active_model

    @property
    def is_using_fallback(self) -> bool:
        return self._active_model == MODEL_FALLBACK

    # ── Helpers ───────────────────────────────────────────────────────────────

    @staticmethod
    def _build_messages(
        messages: list[dict],
        system_prompt: str | None,
    ) -> list[dict]:
        full = []
        if system_prompt:
            full.append({"role": "system", "content": system_prompt})
        full.extend(messages)
        return full

    @staticmethod
    def _classify_and_raise(exc: Exception) -> None:
        """Translate openai exceptions into NIM-specific ones."""
        name = type(exc).__name__
        msg  = str(exc)

        if "AuthenticationError" in name or "401" in msg:
            raise NIMAuthError(f"Invalid API key: {msg}") from exc
        if "RateLimitError" in name or "429" in msg:
            raise NIMRateLimitError(f"Rate limit: {msg}") from exc
        if "NotFoundError" in name or "404" in msg or "model" in msg.lower():
            raise NIMModelError(f"Model unavailable: {msg}") from exc
        if any(x in name for x in ("ConnectionError", "Timeout", "APIError")):
            raise NIMConnectionError(f"Connection failed: {msg}") from exc

        # Unknown — wrap generically
        raise NIMError(f"API error ({name}): {msg}") from exc
