#!/usr/bin/env python3
"""Backfill user-facing What's New sections for existing GitHub releases."""

from __future__ import annotations

import argparse
import importlib.util
import logging
import time
from pathlib import Path
from types import ModuleType
from typing import Any

import requests

from shared import configure_logging, log_event, request_with_retry
from synthesize import read_text, render_prompt, synthesize_notes, validate_template_tokens


DEFAULT_GITHUB_API_BASE_URL = "https://api.github.com"
DEFAULT_LLM_API_URL = "https://openrouter.ai/api/v1/chat/completions"
LOGGER = logging.getLogger("landfall.backfill")


def load_update_release_module() -> ModuleType:
    module_path = Path(__file__).with_name("update-release.py")
    spec = importlib.util.spec_from_file_location("update_release", module_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load module from {module_path}")

    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


_update_release = load_update_release_module()
compose_release_body = _update_release.compose_release_body
github_headers = _update_release.github_headers
update_release_body = _update_release.update_release_body
REPOSITORY_RE = _update_release.REPOSITORY_RE
WHATS_NEW_RE = _update_release.WHATS_NEW_RE


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Backfill user-facing release notes for existing GitHub releases using an OpenAI-compatible LLM."
    )
    parser.add_argument("--repo", required=True, help="GitHub repository in owner/repo format.")
    parser.add_argument("--github-token", required=True, help="GitHub token with repo write access.")
    parser.add_argument("--llm-api-key", required=True, help="API key for LLM synthesis.")
    parser.add_argument(
        "--prompt-template",
        required=True,
        help="Path to prompt template markdown file.",
    )
    parser.add_argument(
        "--model",
        default="anthropic/claude-sonnet-4",
        help="Primary model ID (default: anthropic/claude-sonnet-4).",
    )
    parser.add_argument(
        "--fallback-models",
        default="",
        help="Comma-separated fallback model IDs (default: empty).",
    )
    parser.add_argument(
        "--api-url",
        default=DEFAULT_LLM_API_URL,
        help="OpenAI-compatible chat completions endpoint URL.",
    )
    parser.add_argument(
        "--product-name",
        help="Optional product name override (default: repo name after '/').",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Preview without updating releases.",
    )
    parser.add_argument(
        "--rate-limit",
        type=float,
        default=2.0,
        help="Seconds to sleep between releases (default: 2.0).",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=60,
        help="HTTP timeout in seconds (default: 60).",
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


def validate_args(args: argparse.Namespace) -> None:
    if not args.repo or not REPOSITORY_RE.match(args.repo):
        raise ValueError("repo must match owner/repo")
    if not args.github_token or not args.github_token.strip():
        raise ValueError("github-token must be non-empty")
    if not args.llm_api_key or not args.llm_api_key.strip():
        raise ValueError("llm-api-key must be non-empty")
    if not args.prompt_template or not args.prompt_template.strip():
        raise ValueError("prompt-template must be non-empty")
    if not args.model or not args.model.strip():
        raise ValueError("model must be non-empty")
    if not args.api_url or not args.api_url.startswith(("http://", "https://")):
        raise ValueError("api-url must start with http:// or https://")
    if args.timeout <= 0:
        raise ValueError("timeout must be greater than zero")
    if args.retries < 0:
        raise ValueError("retries cannot be negative")
    if args.retry_backoff < 0:
        raise ValueError("retry-backoff cannot be negative")
    if args.rate_limit < 0:
        raise ValueError("rate-limit cannot be negative")


def fetch_all_releases(
    api_base_url: str,
    repository: str,
    headers: dict[str, str],
    timeout: int,
    retries: int,
    retry_backoff: float,
    session: requests.Session | None = None,
) -> list[dict]:
    created_session = session is None
    http = session or requests.Session()
    releases: list[dict] = []

    try:
        page = 1
        while True:
            url = f"{api_base_url}/repos/{repository}/releases?per_page=100&page={page}"
            response = request_with_retry(
                LOGGER,
                http,
                "GET",
                url,
                headers=headers,
                timeout=timeout,
                retries=retries,
                retry_backoff=retry_backoff,
            )
            try:
                payload = response.json()
            except ValueError as exc:
                raise RuntimeError("GitHub releases response was not valid JSON") from exc
            if not payload:
                break
            if not isinstance(payload, list):
                raise RuntimeError("GitHub releases response was not a list")
            releases.extend(payload)
            page += 1
    finally:
        if created_session:
            http.close()

    return releases


def filter_releases(releases: list[dict]) -> tuple[list[dict], int, int]:
    pending: list[dict] = []
    skipped_filled = 0
    skipped_empty = 0

    for release in releases:
        body = release.get("body")
        body_text = body if isinstance(body, str) else ""
        if not body_text.strip():
            skipped_empty += 1
            continue
        if WHATS_NEW_RE.search(body_text):
            skipped_filled += 1
            continue
        pending.append(release)

    return pending, skipped_filled, skipped_empty


def release_sort_key(release: dict[str, Any]) -> str:
    return str(release.get("published_at") or release.get("created_at") or "")


def parse_fallback_models(value: str) -> list[str]:
    if not value:
        return []
    return [candidate.strip() for candidate in value.split(",") if candidate.strip()]


def main() -> int:
    args = parse_args()
    configure_logging(args.log_level)

    try:
        validate_args(args)
    except ValueError as exc:
        log_event(LOGGER, logging.ERROR, "invalid_input", error=str(exc))
        return 1

    template_path = Path(args.prompt_template)
    try:
        template_text = read_text(template_path)
    except OSError as exc:
        log_event(
            LOGGER,
            logging.ERROR,
            "prompt_template_read_failed",
            path=str(template_path),
            error=str(exc),
        )
        return 1

    try:
        validate_template_tokens(template_text)
    except ValueError as exc:
        log_event(LOGGER, logging.ERROR, "invalid_prompt_template", error=str(exc))
        return 1

    product_name = args.product_name.strip() if args.product_name else args.repo.split("/", 1)[1]

    headers = github_headers(args.github_token)

    http = requests.Session()
    try:
        try:
            releases = fetch_all_releases(
                api_base_url=DEFAULT_GITHUB_API_BASE_URL,
                repository=args.repo,
                headers=headers,
                timeout=args.timeout,
                retries=args.retries,
                retry_backoff=args.retry_backoff,
                session=http,
            )
        except requests.HTTPError as exc:
            status = exc.response.status_code if exc.response is not None else "unknown"
            text = exc.response.text if exc.response is not None else str(exc)
            log_event(
                LOGGER,
                logging.ERROR,
                "github_fetch_http_error",
                status_code=status,
                response_body=text,
                repository=args.repo,
            )
            return 1
        except (requests.RequestException, RuntimeError) as exc:
            log_event(
                LOGGER,
                logging.ERROR,
                "github_fetch_request_failed",
                error=str(exc),
                repository=args.repo,
            )
            return 1

        total = len(releases)
        pending, skipped_filled, skipped_empty = filter_releases(releases)
        pending.sort(key=release_sort_key)

        processed = 0
        failed = 0

        for index, release in enumerate(pending):
            tag_name = str(release.get("tag_name") or "")
            body = release.get("body") if isinstance(release.get("body"), str) else ""
            release_id = release.get("id")

            log_event(
                LOGGER,
                logging.INFO,
                "processing_release",
                repository=args.repo,
                tag_name=tag_name,
            )

            if not isinstance(release_id, int):
                log_event(
                    LOGGER,
                    logging.ERROR,
                    "missing_release_id",
                    repository=args.repo,
                    tag_name=tag_name,
                )
                failed += 1
                if index + 1 < len(pending) and args.rate_limit > 0:
                    time.sleep(args.rate_limit)
                continue

            models_to_try = [args.model] + parse_fallback_models(args.fallback_models)
            prompt = render_prompt(template_text, product_name, tag_name, body)

            synthesized_notes: str | None = None
            last_error: Exception | None = None
            for model in models_to_try:
                try:
                    synthesized_notes = synthesize_notes(
                        api_url=args.api_url,
                        api_key=args.llm_api_key,
                        model=model,
                        prompt=prompt,
                        timeout=args.timeout,
                        retries=args.retries,
                        retry_backoff=args.retry_backoff,
                        session=http,
                    )
                    log_event(LOGGER, logging.INFO, "synthesis_succeeded", model=model, tag_name=tag_name)
                    break
                except (requests.HTTPError, requests.RequestException, RuntimeError, ValueError) as exc:
                    last_error = exc
                    error_fields: dict[str, Any] = {"error": str(exc)}
                    if isinstance(exc, requests.HTTPError) and exc.response is not None:
                        error_fields["status_code"] = exc.response.status_code
                        error_fields["response_body"] = exc.response.text
                    log_event(LOGGER, logging.WARNING, "model_failed", model=model, tag_name=tag_name, **error_fields)
                    continue

            if synthesized_notes is None:
                log_event(
                    LOGGER,
                    logging.ERROR,
                    "all_models_failed",
                    tag_name=tag_name,
                    models_tried=models_to_try,
                    last_error=str(last_error) if last_error is not None else "",
                )
                failed += 1
                if index + 1 < len(pending) and args.rate_limit > 0:
                    time.sleep(args.rate_limit)
                continue

            updated_body = compose_release_body(synthesized_notes, body)

            if args.dry_run:
                log_event(
                    LOGGER,
                    logging.INFO,
                    "dry_run_preview",
                    repository=args.repo,
                    tag_name=tag_name,
                    notes_preview=synthesized_notes[:200],
                )
                processed += 1
                if index + 1 < len(pending) and args.rate_limit > 0:
                    time.sleep(args.rate_limit)
                continue

            try:
                update_release_body(
                    api_base_url=DEFAULT_GITHUB_API_BASE_URL,
                    repository=args.repo,
                    release_id=release_id,
                    body=updated_body,
                    headers=headers,
                    timeout=args.timeout,
                    retries=args.retries,
                    retry_backoff=args.retry_backoff,
                    session=http,
                )
            except requests.HTTPError as exc:
                status = exc.response.status_code if exc.response is not None else "unknown"
                text = exc.response.text if exc.response is not None else str(exc)
                log_event(
                    LOGGER,
                    logging.ERROR,
                    "github_update_http_error",
                    status_code=status,
                    response_body=text,
                    release_id=release_id,
                    repository=args.repo,
                    tag_name=tag_name,
                )
                failed += 1
            except requests.RequestException as exc:
                log_event(
                    LOGGER,
                    logging.ERROR,
                    "github_update_request_failed",
                    error=str(exc),
                    release_id=release_id,
                    repository=args.repo,
                    tag_name=tag_name,
                )
                failed += 1
            else:
                log_event(LOGGER, logging.INFO, "release_updated", repository=args.repo, tag_name=tag_name)
                processed += 1

            if index + 1 < len(pending) and args.rate_limit > 0:
                time.sleep(args.rate_limit)

        print(
            "Backfill summary:"
            f" total={total}"
            f" processed={processed}"
            f" skipped_filled={skipped_filled}"
            f" skipped_empty={skipped_empty}"
            f" failed={failed}"
        )
        return 1 if failed else 0
    finally:
        http.close()


if __name__ == "__main__":
    raise SystemExit(main())

