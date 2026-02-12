#!/usr/bin/env python3
"""Update an existing GitHub Release body with synthesized user-facing notes."""

from __future__ import annotations

import argparse
import logging
import re
from pathlib import Path

import requests

from shared import configure_logging, log_event, request_with_retry


WHATS_NEW_RE = re.compile(
    r"^## What's New\b.*?(?=^##\s+|\Z)",
    flags=re.MULTILINE | re.DOTALL,
)
REPOSITORY_RE = re.compile(r"^[^/\s]+/[^/\s]+$")
LOGGER = logging.getLogger("landfall.update_release")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Prepend synthesized release notes under a What's New section."
    )
    parser.add_argument("--github-token", required=True, help="GitHub token with repo write access.")
    parser.add_argument("--repository", required=True, help="GitHub repository in owner/repo format.")
    parser.add_argument("--tag", required=True, help="Release tag to update.")
    parser.add_argument("--notes-file", required=True, help="Path to synthesized notes markdown file.")
    parser.add_argument("--api-base-url", default="https://api.github.com", help="GitHub API base URL.")
    parser.add_argument("--timeout", type=int, default=30, help="HTTP timeout in seconds.")
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


def read_notes(path: Path) -> str:
    return path.read_text(encoding="utf-8").strip()


def validate_args(args: argparse.Namespace) -> None:
    if not args.github_token or not args.github_token.strip():
        raise ValueError("github-token must be non-empty")
    if not args.repository or not REPOSITORY_RE.match(args.repository):
        raise ValueError("repository must match owner/repo")
    if not args.tag or not args.tag.strip():
        raise ValueError("tag must be non-empty")
    if args.timeout <= 0:
        raise ValueError("timeout must be greater than zero")
    if args.retries < 0:
        raise ValueError("retries cannot be negative")
    if args.retry_backoff < 0:
        raise ValueError("retry-backoff cannot be negative")
    if not args.api_base_url.startswith(("http://", "https://")):
        raise ValueError("api-base-url must start with http:// or https://")


def strip_existing_whats_new(body: str) -> str:
    cleaned = WHATS_NEW_RE.sub("", body, count=1).strip()
    return cleaned


def github_headers(token: str) -> dict[str, str]:
    return {
        "Accept": "application/vnd.github+json",
        "Authorization": f"Bearer {token}",
        "X-GitHub-Api-Version": "2022-11-28",
        "Content-Type": "application/json",
    }


def fetch_release(
    api_base_url: str,
    repository: str,
    tag: str,
    headers: dict[str, str],
    timeout: int,
    retries: int,
    retry_backoff: float,
    session: requests.Session | None = None,
) -> dict | None:
    """Fetch a GitHub Release by tag. Returns None if no release exists (404)."""
    url = f"{api_base_url}/repos/{repository}/releases/tags/{tag}"
    created_session = session is None
    http = session or requests.Session()
    try:
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
        return response.json()
    except requests.HTTPError as exc:
        if exc.response is not None and exc.response.status_code == 404:
            return None
        raise
    finally:
        if created_session:
            http.close()


def update_release_body(
    api_base_url: str,
    repository: str,
    release_id: int,
    body: str,
    headers: dict[str, str],
    timeout: int,
    retries: int,
    retry_backoff: float,
    session: requests.Session | None = None,
) -> None:
    url = f"{api_base_url}/repos/{repository}/releases/{release_id}"
    created_session = session is None
    http = session or requests.Session()
    try:
        request_with_retry(
            LOGGER,
            http,
            "PATCH",
            url,
            headers=headers,
            json={"body": body},
            timeout=timeout,
            retries=retries,
            retry_backoff=retry_backoff,
        )
    finally:
        if created_session:
            http.close()


def compose_release_body(synth_notes: str, existing_body: str) -> str:
    technical_body = strip_existing_whats_new(existing_body) if existing_body else ""
    sections = [f"## What's New\n\n{synth_notes.strip()}"]
    if technical_body:
        sections.append(technical_body)
    return "\n\n".join(section.strip() for section in sections if section.strip()).strip() + "\n"


def main() -> int:
    args = parse_args()
    configure_logging(args.log_level)

    try:
        validate_args(args)
    except ValueError as exc:
        log_event(LOGGER, logging.ERROR, "invalid_input", error=str(exc))
        return 1

    headers = github_headers(args.github_token)

    try:
        synthesized_notes = read_notes(Path(args.notes_file))
    except OSError as exc:
        log_event(LOGGER, logging.ERROR, "notes_read_failed", path=args.notes_file, error=str(exc))
        return 1

    if not synthesized_notes:
        log_event(LOGGER, logging.ERROR, "empty_notes_file", path=args.notes_file)
        return 1

    try:
        release = fetch_release(
            api_base_url=args.api_base_url,
            repository=args.repository,
            tag=args.tag,
            headers=headers,
            timeout=args.timeout,
            retries=args.retries,
            retry_backoff=args.retry_backoff,
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
            tag=args.tag,
            repository=args.repository,
        )
        return 1
    except requests.RequestException as exc:
        log_event(
            LOGGER,
            logging.ERROR,
            "github_fetch_request_failed",
            error=str(exc),
            tag=args.tag,
            repository=args.repository,
        )
        return 1

    if release is None:
        log_event(
            LOGGER,
            logging.WARNING,
            "release_not_found",
            tag=args.tag,
            repository=args.repository,
            hint="Floating tags (v1, v2) do not have GitHub Releases; only semver tags do.",
        )
        print(f"No GitHub Release found for tag '{args.tag}'; skipping release body update.")
        return 0

    release_id = release.get("id")
    if not isinstance(release_id, int):
        log_event(LOGGER, logging.ERROR, "missing_release_id", tag=args.tag, repository=args.repository)
        return 1

    existing_body = release.get("body") or ""
    updated_body = compose_release_body(synthesized_notes, existing_body)

    try:
        update_release_body(
            api_base_url=args.api_base_url,
            repository=args.repository,
            release_id=release_id,
            body=updated_body,
            headers=headers,
            timeout=args.timeout,
            retries=args.retries,
            retry_backoff=args.retry_backoff,
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
            repository=args.repository,
        )
        return 1
    except requests.RequestException as exc:
        log_event(
            LOGGER,
            logging.ERROR,
            "github_update_request_failed",
            error=str(exc),
            release_id=release_id,
            repository=args.repository,
        )
        return 1

    log_event(LOGGER, logging.INFO, "release_updated", tag=args.tag, repository=args.repository)
    print(f"Updated release '{args.tag}' in '{args.repository}'.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
