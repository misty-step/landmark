#!/usr/bin/env python3
"""Generate user-facing release notes from technical changelog content."""

from __future__ import annotations

import argparse
import json
import logging
import os
import re
import sys
import time
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

import requests


DEFAULT_API_URL = "https://openrouter.ai/api/v1/chat/completions"
SECTION_HEADING_RE = re.compile(r"^##\s+.+$", re.MULTILINE)
RETRYABLE_STATUS_CODES = {429, 500, 502, 503, 504}
REQUIRED_TEMPLATE_TOKENS = (
    "{{PRODUCT_NAME}}",
    "{{VERSION}}",
    "{{TECHNICAL_CHANGELOG}}",
)
LOGGER = logging.getLogger("landfall.synthesize")


def configure_logging(level_name: str) -> None:
    level = getattr(logging, level_name.upper(), logging.INFO)
    logging.basicConfig(level=level, format="%(message)s", stream=sys.stderr)


def log_event(level: int, event: str, **fields: Any) -> None:
    payload = {"event": event, **fields}
    LOGGER.log(level, json.dumps(payload, sort_keys=True, default=str))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Synthesize user-facing release notes with an OpenAI-compatible LLM provider."
    )
    parser.add_argument("--api-key", required=True, help="API key for the LLM provider.")
    parser.add_argument(
        "--model",
        default="anthropic/claude-sonnet-4",
        help="Primary model ID (default: anthropic/claude-sonnet-4).",
    )
    parser.add_argument(
        "--fallback-models",
        default="",
        help="Comma-separated fallback model IDs tried after the primary model.",
    )
    parser.add_argument(
        "--api-url",
        default=DEFAULT_API_URL,
        help="OpenAI-compatible chat completions endpoint URL.",
    )
    parser.add_argument(
        "--prompt-template",
        required=True,
        help="Path to prompt template markdown file.",
    )
    parser.add_argument(
        "--changelog-file",
        default="CHANGELOG.md",
        help="Path to markdown changelog.",
    )
    parser.add_argument(
        "--technical-changelog-file",
        help="Optional path to raw technical changelog text.",
    )
    parser.add_argument(
        "--product-name",
        help="Product name injected into the prompt template.",
    )
    parser.add_argument(
        "--version",
        help="Version or tag used to locate a changelog section.",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=60,
        help="HTTP timeout in seconds.",
    )
    parser.add_argument(
        "--retries",
        type=int,
        default=2,
        help="Number of retries for retryable HTTP failures (default: 2).",
    )
    parser.add_argument(
        "--retry-backoff",
        type=float,
        default=1.0,
        help="Base backoff seconds between retries (default: 1.0).",
    )
    parser.add_argument(
        "--log-level",
        default="INFO",
        choices=("DEBUG", "INFO", "WARNING", "ERROR"),
        help="Structured log verbosity written to stderr.",
    )
    return parser.parse_args()


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8").strip()


def validate_args(args: argparse.Namespace) -> None:
    if not args.api_key or not args.api_key.strip():
        raise ValueError("api-key must be non-empty")
    if not args.model or not args.model.strip():
        raise ValueError("model must be non-empty")
    if args.timeout <= 0:
        raise ValueError("timeout must be greater than zero")
    if args.retries < 0:
        raise ValueError("retries cannot be negative")
    if args.retry_backoff < 0:
        raise ValueError("retry-backoff cannot be negative")
    if not args.api_url.startswith(("http://", "https://")):
        raise ValueError("api-url must start with http:// or https://")
    parsed_url = urlparse(args.api_url)
    if (
        parsed_url.scheme == "http"
        and parsed_url.hostname
        and parsed_url.hostname not in ("localhost", "127.0.0.1")
    ):
        log_event(logging.WARNING, "insecure_api_url", url=args.api_url)
    if args.version is not None and not args.version.strip():
        raise ValueError("version cannot be blank when provided")


def normalize_version(version: str) -> str:
    return version.strip().lstrip("v")


def extract_release_section(changelog_text: str, version: str | None) -> str:
    headings = list(SECTION_HEADING_RE.finditer(changelog_text))
    if not headings:
        return changelog_text.strip()

    target_index = 0
    if version:
        normalized = normalize_version(version).lower()
        for index, match in enumerate(headings):
            heading = match.group(0).lower()
            if normalized in heading or f"v{normalized}" in heading:
                target_index = index
                break
        else:
            log_event(
                logging.WARNING,
                "version_not_found",
                version=version,
                fallback="latest_section",
            )

    start = headings[target_index].start()
    if target_index + 1 < len(headings):
        end = headings[target_index + 1].start()
    else:
        end = len(changelog_text)
    return changelog_text[start:end].strip()


def render_prompt(template_text: str, product_name: str, version: str, technical: str) -> str:
    return (
        template_text.replace("{{PRODUCT_NAME}}", product_name)
        .replace("{{VERSION}}", version)
        .replace("{{TECHNICAL_CHANGELOG}}", technical)
    )


def validate_template_tokens(template_text: str) -> None:
    missing = [token for token in REQUIRED_TEMPLATE_TOKENS if token not in template_text]
    if missing:
        raise ValueError(f"prompt template missing required token(s): {', '.join(missing)}")


def infer_product_name(explicit_name: str | None) -> str:
    if explicit_name:
        return explicit_name
    repository = os.getenv("GITHUB_REPOSITORY", "")
    if "/" in repository:
        return repository.split("/", 1)[1]
    return "this product"


def request_with_retry(
    session: requests.Session,
    method: str,
    url: str,
    *,
    timeout: int,
    retries: int,
    retry_backoff: float,
    **kwargs: Any,
) -> requests.Response:
    total_attempts = retries + 1
    for attempt in range(1, total_attempts + 1):
        try:
            response = session.request(method=method.upper(), url=url, timeout=timeout, **kwargs)
        except (requests.Timeout, requests.ConnectionError) as exc:
            if attempt >= total_attempts:
                raise
            delay = retry_backoff * (2 ** (attempt - 1))
            log_event(
                logging.WARNING,
                "http_retry_exception",
                attempt=attempt,
                max_attempts=total_attempts,
                method=method.upper(),
                url=url,
                wait_seconds=delay,
                error_type=type(exc).__name__,
            )
            time.sleep(delay)
            continue

        if response.status_code in RETRYABLE_STATUS_CODES and attempt < total_attempts:
            delay = retry_backoff * (2 ** (attempt - 1))
            log_event(
                logging.WARNING,
                "http_retry_status",
                attempt=attempt,
                max_attempts=total_attempts,
                method=method.upper(),
                url=url,
                status_code=response.status_code,
                wait_seconds=delay,
            )
            time.sleep(delay)
            continue

        response.raise_for_status()
        return response

    raise RuntimeError("failed to receive HTTP response")


def synthesize_notes(
    api_url: str,
    api_key: str,
    model: str,
    prompt: str,
    timeout: int,
    retries: int,
    retry_backoff: float,
    session: requests.Session | None = None,
) -> str:
    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
        "HTTP-Referer": "https://github.com/misty-step/landfall",
        "X-Title": "Landfall Release Pipeline",
    }
    payload = {
        "model": model,
        "temperature": 0.2,
        "messages": [
            {
                "role": "system",
                "content": "You rewrite technical release notes into user-facing product notes.",
            },
            {"role": "user", "content": prompt},
        ],
    }

    created_session = session is None
    http = session or requests.Session()

    try:
        response = request_with_retry(
            http,
            "POST",
            api_url,
            headers=headers,
            json=payload,
            timeout=timeout,
            retries=retries,
            retry_backoff=retry_backoff,
        )
        body = response.json()
    finally:
        if created_session:
            http.close()

    try:
        content = body["choices"][0]["message"]["content"]
    except (KeyError, IndexError, TypeError) as exc:
        raise RuntimeError(
            "LLM provider response did not include choices[0].message.content"
        ) from exc

    notes = content.strip()
    if not notes:
        raise RuntimeError("LLM provider returned empty synthesized notes")
    return notes


def main() -> int:
    args = parse_args()
    configure_logging(args.log_level)

    try:
        validate_args(args)
    except ValueError as exc:
        log_event(logging.ERROR, "invalid_input", error=str(exc))
        return 1

    template_path = Path(args.prompt_template)

    try:
        template_text = read_text(template_path)
    except OSError as exc:
        log_event(
            logging.ERROR,
            "prompt_template_read_failed",
            path=str(template_path),
            error=str(exc),
        )
        return 1

    try:
        validate_template_tokens(template_text)
    except ValueError as exc:
        log_event(logging.ERROR, "invalid_prompt_template", error=str(exc))
        return 1

    try:
        if args.technical_changelog_file:
            changelog_path = Path(args.technical_changelog_file)
            technical_text = read_text(changelog_path)
        else:
            changelog_path = Path(args.changelog_file)
            changelog_text = read_text(changelog_path)
            technical_text = extract_release_section(changelog_text, args.version)
    except OSError as exc:
        log_event(
            logging.ERROR,
            "changelog_read_failed",
            path=str(changelog_path),
            error=str(exc),
        )
        return 1

    if not technical_text:
        log_event(logging.ERROR, "empty_changelog")
        return 1

    product_name = infer_product_name(args.product_name)
    version = args.version.strip() if args.version else "latest"
    prompt = render_prompt(template_text, product_name, version, technical_text)

    models_to_try = [args.model]
    if args.fallback_models:
        models_to_try.extend(
            candidate.strip()
            for candidate in args.fallback_models.split(",")
            if candidate.strip()
        )

    last_error: Exception | None = None
    for model in models_to_try:
        try:
            synthesized = synthesize_notes(
                api_url=args.api_url,
                api_key=args.api_key,
                model=model,
                prompt=prompt,
                timeout=args.timeout,
                retries=args.retries,
                retry_backoff=args.retry_backoff,
            )
            log_event(logging.INFO, "synthesis_succeeded", model=model)
            print(synthesized)
            return 0
        except (requests.HTTPError, requests.RequestException, RuntimeError) as exc:
            last_error = exc
            error_fields: dict[str, Any] = {"error": str(exc)}
            if isinstance(exc, requests.HTTPError) and exc.response is not None:
                error_fields["status_code"] = exc.response.status_code
                error_fields["response_body"] = exc.response.text
            log_event(logging.WARNING, "model_failed", model=model, **error_fields)
            continue

    log_event(
        logging.ERROR,
        "all_models_failed",
        models_tried=models_to_try,
        last_error=str(last_error) if last_error is not None else "",
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
