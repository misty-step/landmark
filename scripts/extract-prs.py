#!/usr/bin/env python3
"""Extract merged pull requests for a release window as pseudo-changelog markdown."""

from __future__ import annotations

import argparse
import logging
import re
import subprocess
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import requests

from shared import configure_logging, log_event, request_with_retry


DEFAULT_GITHUB_API_BASE_URL = "https://api.github.com"
REPOSITORY_RE = re.compile(r"^[^/\s]+/[^/\s]+$")
LOGGER = logging.getLogger("landfall.extract_prs")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Extract merged pull requests between the previous tag and a target release tag "
            "and render them as pseudo-changelog markdown."
        )
    )
    parser.add_argument("--github-token", required=True, help="GitHub token with repository read access.")
    parser.add_argument("--repository", required=True, help="GitHub repository in owner/repo format.")
    parser.add_argument("--release-tag", required=True, help="Release tag to extract PRs for (e.g., v1.2.3).")
    parser.add_argument("--output-file", required=True, help="Output markdown file path.")
    parser.add_argument(
        "--base-branch",
        default="main",
        help="Base branch used by pull requests (default: main).",
    )
    parser.add_argument(
        "--api-base-url",
        default=DEFAULT_GITHUB_API_BASE_URL,
        help="GitHub API base URL (default: https://api.github.com).",
    )
    parser.add_argument(
        "--body-chars",
        type=int,
        default=500,
        help="Maximum PR body excerpt length in characters (default: 500).",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=30,
        help="HTTP timeout in seconds (default: 30).",
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
    if not args.github_token or not args.github_token.strip():
        raise ValueError("github-token must be non-empty")
    if not args.repository or not REPOSITORY_RE.match(args.repository):
        raise ValueError("repository must match owner/repo")
    if not args.release_tag or not args.release_tag.strip():
        raise ValueError("release-tag must be non-empty")
    if not args.output_file or not str(args.output_file).strip():
        raise ValueError("output-file must be non-empty")
    if not args.base_branch or not args.base_branch.strip():
        raise ValueError("base-branch must be non-empty")
    if not args.api_base_url.startswith(("http://", "https://")):
        raise ValueError("api-base-url must start with http:// or https://")
    if args.body_chars <= 0:
        raise ValueError("body-chars must be greater than zero")
    if args.timeout <= 0:
        raise ValueError("timeout must be greater than zero")
    if args.retries < 0:
        raise ValueError("retries cannot be negative")
    if args.retry_backoff < 0:
        raise ValueError("retry-backoff cannot be negative")


def parse_iso8601(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00")).astimezone(UTC)


def trim_text(text: str, limit: int) -> str:
    collapsed = re.sub(r"\s+", " ", text).strip()
    if len(collapsed) <= limit:
        return collapsed
    return f"{collapsed[:limit]}..."


def git_output(*args: str) -> str:
    return subprocess.check_output(["git", *args], text=True).strip()


def resolve_tag_datetime(tag: str) -> datetime:
    timestamp = git_output("log", "-1", "--format=%cI", tag)
    if not timestamp:
        raise RuntimeError(f"tag '{tag}' did not resolve to a commit timestamp")
    return parse_iso8601(timestamp)


def resolve_previous_tag(release_tag: str) -> str | None:
    try:
        previous = git_output("describe", "--tags", "--abbrev=0", f"{release_tag}^")
    except subprocess.CalledProcessError:
        return None
    return previous if previous else None


def github_headers(github_token: str) -> dict[str, str]:
    return {
        "Authorization": f"token {github_token}",
        "Accept": "application/vnd.github+json",
    }


def fetch_closed_pull_requests(
    *,
    api_base_url: str,
    repository: str,
    base_branch: str,
    headers: dict[str, str],
    timeout: int,
    retries: int,
    retry_backoff: float,
    session: requests.Session | None = None,
) -> list[dict[str, Any]]:
    created_session = session is None
    http = session or requests.Session()
    pulls: list[dict[str, Any]] = []

    try:
        page = 1
        while True:
            response = request_with_retry(
                LOGGER,
                http,
                "GET",
                f"{api_base_url}/repos/{repository}/pulls",
                headers=headers,
                params={
                    "state": "closed",
                    "base": base_branch,
                    "sort": "updated",
                    "direction": "desc",
                    "per_page": 100,
                    "page": page,
                },
                timeout=timeout,
                retries=retries,
                retry_backoff=retry_backoff,
            )
            payload = response.json()
            if not isinstance(payload, list):
                raise RuntimeError("GitHub pull request response was not a list")
            if not payload:
                break
            pulls.extend(payload)
            if len(payload) < 100:
                break
            page += 1
    finally:
        if created_session:
            http.close()

    return pulls


def filter_prs_by_window(
    pulls: list[dict[str, Any]],
    start_at: datetime | None,
    end_at: datetime,
) -> list[dict[str, Any]]:
    filtered: list[dict[str, Any]] = []
    for pull in pulls:
        merged_at_value = pull.get("merged_at")
        if not isinstance(merged_at_value, str) or not merged_at_value:
            continue

        merged_at = parse_iso8601(merged_at_value)
        if start_at is not None and merged_at <= start_at:
            continue
        if merged_at > end_at:
            continue

        filtered.append(pull)

    filtered.sort(key=lambda pull: parse_iso8601(str(pull["merged_at"])))
    return filtered


def render_pr_changelog(
    pulls: list[dict[str, Any]],
    release_tag: str,
    *,
    body_chars: int,
) -> str:
    lines = [f"## Pull Request Changelog ({release_tag})", ""]
    if not pulls:
        lines.append("- No merged pull requests found for this release window.")
        return "\n".join(lines).strip() + "\n"

    for pull in pulls:
        number = pull.get("number")
        title = str(pull.get("title") or "").strip() or "(untitled)"
        author = str((pull.get("user") or {}).get("login") or "unknown")
        labels = [
            str(label.get("name"))
            for label in (pull.get("labels") or [])
            if isinstance(label, dict) and label.get("name")
        ]
        body_excerpt = trim_text(str(pull.get("body") or ""), body_chars)

        lines.append(f"### #{number} {title}")
        lines.append(f"- Author: @{author}")
        if labels:
            lines.append(f"- Labels: {', '.join(labels)}")
        if body_excerpt:
            lines.append(f"- Summary: {body_excerpt}")
        lines.append("")

    return "\n".join(lines).strip() + "\n"


def main() -> int:
    args = parse_args()
    configure_logging(args.log_level)

    try:
        validate_args(args)
    except ValueError as exc:
        log_event(LOGGER, logging.ERROR, "invalid_input", error=str(exc))
        return 1

    release_tag = args.release_tag.strip()
    output_file = Path(args.output_file)

    try:
        end_at = resolve_tag_datetime(release_tag)
    except (subprocess.CalledProcessError, RuntimeError) as exc:
        log_event(LOGGER, logging.ERROR, "release_tag_lookup_failed", tag=release_tag, error=str(exc))
        return 1

    previous_tag = resolve_previous_tag(release_tag)
    start_at: datetime | None = None
    if previous_tag is not None:
        try:
            start_at = resolve_tag_datetime(previous_tag)
        except (subprocess.CalledProcessError, RuntimeError) as exc:
            log_event(LOGGER, logging.ERROR, "previous_tag_lookup_failed", tag=previous_tag, error=str(exc))
            return 1

    try:
        pulls = fetch_closed_pull_requests(
            api_base_url=args.api_base_url,
            repository=args.repository,
            base_branch=args.base_branch,
            headers=github_headers(args.github_token),
            timeout=args.timeout,
            retries=args.retries,
            retry_backoff=args.retry_backoff,
        )
    except (requests.RequestException, RuntimeError) as exc:
        log_event(LOGGER, logging.ERROR, "pull_request_fetch_failed", error=str(exc))
        return 1

    filtered = filter_prs_by_window(pulls, start_at, end_at)
    changelog = render_pr_changelog(filtered, release_tag, body_chars=args.body_chars)

    output_file.parent.mkdir(parents=True, exist_ok=True)
    output_file.write_text(changelog, encoding="utf-8")

    log_event(
        LOGGER,
        logging.INFO,
        "pr_changelog_extracted",
        repository=args.repository,
        release_tag=release_tag,
        previous_tag=previous_tag or "",
        pull_request_count=len(filtered),
        output_file=str(output_file),
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
