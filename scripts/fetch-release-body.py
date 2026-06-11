#!/usr/bin/env python3
"""Fetch a GitHub Release body to a file."""

from __future__ import annotations

import argparse
import logging
import re
from pathlib import Path

import requests

from shared import configure_logging, log_event, request_with_retry


LOGGER = logging.getLogger("landfall.fetch_release_body")
REPOSITORY_RE = re.compile(r"^[^/\s]+/[^/\s]+$")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Fetch a GitHub Release body by tag.")
    parser.add_argument("--github-token", required=True, help="GitHub token with repo read access.")
    parser.add_argument("--repository", required=True, help="GitHub repository in owner/repo format.")
    parser.add_argument("--release-tag", required=True, help="Release tag to fetch.")
    parser.add_argument("--output-file", required=True, help="File to write release body markdown into.")
    parser.add_argument("--api-base-url", default="https://api.github.com", help="GitHub API base URL.")
    parser.add_argument("--timeout", type=int, default=30, help="HTTP timeout seconds.")
    parser.add_argument("--retries", type=int, default=2, help="Retry count for retryable failures.")
    parser.add_argument("--retry-backoff", type=float, default=1.0, help="Base backoff seconds.")
    parser.add_argument(
        "--log-level",
        default="INFO",
        choices=("DEBUG", "INFO", "WARNING", "ERROR"),
        help="Structured log verbosity.",
    )
    return parser.parse_args()


def validate_args(args: argparse.Namespace) -> None:
    if not args.github_token or not args.github_token.strip():
        raise ValueError("github-token must be non-empty")
    if not args.repository or not REPOSITORY_RE.match(args.repository):
        raise ValueError("repository must match owner/repo")
    if not args.release_tag or not args.release_tag.strip():
        raise ValueError("release-tag must be non-empty")
    if not args.output_file or not args.output_file.strip():
        raise ValueError("output-file must be non-empty")
    if not args.api_base_url.startswith(("http://", "https://")):
        raise ValueError("api-base-url must start with http:// or https://")
    if args.timeout <= 0:
        raise ValueError("timeout must be greater than zero")
    if args.retries < 0:
        raise ValueError("retries cannot be negative")
    if args.retry_backoff < 0:
        raise ValueError("retry-backoff cannot be negative")


def github_headers(token: str) -> dict[str, str]:
    return {
        "Accept": "application/vnd.github+json",
        "Authorization": f"Bearer {token}",
        "X-GitHub-Api-Version": "2022-11-28",
    }


def fetch_release_body(
    *,
    api_base_url: str,
    repository: str,
    release_tag: str,
    token: str,
    timeout: int,
    retries: int,
    retry_backoff: float,
    session: requests.Session | None = None,
) -> str | None:
    url = f"{api_base_url}/repos/{repository}/releases/tags/{release_tag}"
    created_session = session is None
    http = session or requests.Session()
    try:
        response = request_with_retry(
            LOGGER,
            http,
            "GET",
            url,
            headers=github_headers(token),
            timeout=timeout,
            retries=retries,
            retry_backoff=retry_backoff,
        )
        payload = response.json()
    except requests.HTTPError as exc:
        if exc.response is not None and exc.response.status_code == 404:
            return None
        raise
    finally:
        if created_session:
            http.close()

    body = payload.get("body", "")
    return body if isinstance(body, str) and body.strip() else None


def main() -> int:
    args = parse_args()
    configure_logging(args.log_level)
    try:
        validate_args(args)
    except ValueError as exc:
        log_event(LOGGER, logging.ERROR, "invalid_input", error=str(exc))
        return 1

    try:
        body = fetch_release_body(
            api_base_url=args.api_base_url,
            repository=args.repository,
            release_tag=args.release_tag,
            token=args.github_token,
            timeout=args.timeout,
            retries=args.retries,
            retry_backoff=args.retry_backoff,
        )
    except requests.HTTPError as exc:
        status = exc.response.status_code if exc.response is not None else "unknown"
        text = exc.response.text if exc.response is not None else str(exc)
        log_event(LOGGER, logging.ERROR, "github_fetch_http_error", status_code=status, response_body=text)
        return 1
    except requests.RequestException as exc:
        log_event(LOGGER, logging.ERROR, "github_fetch_request_failed", error=str(exc))
        return 1

    if body is None:
        log_event(LOGGER, logging.WARNING, "release_body_unavailable", release_tag=args.release_tag)
        return 0

    destination = Path(args.output_file)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(body, encoding="utf-8")
    log_event(LOGGER, logging.INFO, "release_body_written", path=str(destination))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
