#!/usr/bin/env python3
"""Validate LLM API key with a minimal request."""

from __future__ import annotations

import argparse
import logging
import sys

import requests

from shared import configure_logging, log_event, request_with_retry

LOGGER = logging.getLogger("landfall.healthcheck")
DEFAULT_API_URL = "https://openrouter.ai/api/v1/chat/completions"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate LLM API key connectivity.")
    parser.add_argument("--api-key", required=True, help="API key to validate.")
    parser.add_argument(
        "--model",
        default="anthropic/claude-sonnet-4",
        help="Model ID for the probe request.",
    )
    parser.add_argument(
        "--api-url",
        default=DEFAULT_API_URL,
        help="OpenAI-compatible chat completions endpoint.",
    )
    parser.add_argument("--timeout", type=int, default=30, help="HTTP timeout in seconds.")
    parser.add_argument(
        "--warn-only",
        action="store_true",
        help="Emit ::warning:: and exit 0 on failure instead of ::error:: and exit 1. "
        "Use when synthesis is optional (synthesis-required: false).",
    )
    parser.add_argument(
        "--log-level",
        default="INFO",
        choices=("DEBUG", "INFO", "WARNING", "ERROR"),
    )
    return parser.parse_args()


def probe_api(api_url: str, api_key: str, model: str, timeout: int) -> None:
    """Send a minimal completion request to verify the API key works."""
    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
        "HTTP-Referer": "https://github.com/misty-step/landfall",
        "X-Title": "Landfall Health Check",
    }
    payload = {
        "model": model,
        "max_tokens": 5,
        "temperature": 0,
        "messages": [{"role": "user", "content": "Say OK"}],
    }

    session = requests.Session()
    try:
        response = request_with_retry(
            LOGGER,
            session,
            "POST",
            api_url,
            headers=headers,
            json=payload,
            timeout=timeout,
            retries=1,
            retry_backoff=2.0,
        )
        body = response.json()
        content = body.get("choices", [{}])[0].get("message", {}).get("content", "")
        if not content.strip():
            raise RuntimeError("API returned empty response")
    finally:
        session.close()


def _fail(message: str, warn_only: bool) -> int:
    """Emit a GitHub Actions annotation and return the appropriate exit code."""
    if warn_only:
        print(f"::warning::{message}", file=sys.stderr)
        return 0
    print(f"::error::{message}", file=sys.stderr)
    return 1


def main() -> int:
    args = parse_args()
    configure_logging(args.log_level)

    if not args.api_key or not args.api_key.strip():
        log_event(LOGGER, logging.ERROR, "healthcheck_skipped", reason="api-key is empty")
        return 1

    try:
        probe_api(args.api_url, args.api_key, args.model, args.timeout)
    except requests.HTTPError as exc:
        status = exc.response.status_code if exc.response is not None else "unknown"
        if status == 401:
            if args.api_url.startswith("https://openrouter.ai/"):
                message = (
                    "LLM auth failed (HTTP 401). "
                    "Default provider is OpenRouter — ensure llm-api-key is an OpenRouter key "
                    "(format: sk-or-...). Get a key at https://openrouter.ai/keys"
                )
            else:
                message = (
                    f"LLM auth failed (HTTP 401) for {args.api_url}. "
                    "Verify your llm-api-key secret is valid for the configured provider."
                )
        elif status == 403:
            message = (
                "API key lacks permissions (HTTP 403). "
                "Check provider account for billing or model access restrictions."
            )
        else:
            message = f"API request failed (HTTP {status}). Check provider status and key validity."
        log_event(LOGGER, logging.ERROR, "healthcheck_failed", status_code=status, message=message)
        return _fail(message, args.warn_only)
    except (requests.RequestException, RuntimeError) as exc:
        message = f"Health check failed: {exc}"
        log_event(LOGGER, logging.ERROR, "healthcheck_failed", error=str(exc))
        return _fail(message, args.warn_only)

    log_event(LOGGER, logging.INFO, "healthcheck_passed", model=args.model)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
