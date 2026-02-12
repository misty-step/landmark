#!/usr/bin/env python3
"""Create a GitHub issue when Landfall synthesis fails."""

from __future__ import annotations

import argparse
import json
import logging
import re
import sys
from typing import Any

import requests


REPOSITORY_RE = re.compile(r"^[^/\s]+/[^/\s]+$")
LOGGER = logging.getLogger("landfall.report_synthesis_failure")


def configure_logging(level_name: str) -> None:
    level = getattr(logging, level_name.upper(), logging.INFO)
    logging.basicConfig(level=level, format="%(message)s", stream=sys.stderr)


def log_event(level: int, event: str, **fields: Any) -> None:
    payload = {"event": event, **fields}
    LOGGER.log(level, json.dumps(payload, sort_keys=True, default=str))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Create an issue in the consuming repository when synthesis fails."
    )
    parser.add_argument("--github-token", required=True, help="GitHub token with issues write access.")
    parser.add_argument("--repository", required=True, help="GitHub repository in owner/repo format.")
    parser.add_argument("--release-tag", required=True, help="Release tag where synthesis failed.")
    parser.add_argument("--failure-stage", required=True, help="Failure stage identifier.")
    parser.add_argument("--failure-message", required=True, help="Human-readable failure summary.")
    parser.add_argument("--workflow-run-url", required=True, help="URL to the failed workflow run.")
    parser.add_argument("--workflow-name", required=True, help="Workflow name.")
    parser.add_argument("--api-base-url", default="https://api.github.com", help="GitHub API base URL.")
    parser.add_argument("--timeout", type=int, default=30, help="HTTP timeout in seconds.")
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
    if not args.failure_stage or not args.failure_stage.strip():
        raise ValueError("failure-stage must be non-empty")
    if not args.failure_message or not args.failure_message.strip():
        raise ValueError("failure-message must be non-empty")
    if not args.workflow_run_url or not args.workflow_run_url.startswith(("http://", "https://")):
        raise ValueError("workflow-run-url must start with http:// or https://")
    if not args.workflow_name or not args.workflow_name.strip():
        raise ValueError("workflow-name must be non-empty")
    if args.timeout <= 0:
        raise ValueError("timeout must be greater than zero")
    if not args.api_base_url.startswith(("http://", "https://")):
        raise ValueError("api-base-url must start with http:// or https://")


def github_headers(token: str) -> dict[str, str]:
    return {
        "Accept": "application/vnd.github+json",
        "Authorization": f"Bearer {token}",
        "X-GitHub-Api-Version": "2022-11-28",
        "Content-Type": "application/json",
    }


def describe_failure_stage(failure_stage: str) -> str:
    labels = {
        "configuration": "Configuration",
        "synthesis": "Synthesis request",
        "synthesis_empty": "Synthesis output validation",
        "release_update": "Release body update",
        "unknown": "Unknown stage",
    }
    return labels.get(failure_stage, "Synthesis pipeline")


def compose_issue_title(release_tag: str) -> str:
    return f"[Landfall] Synthesis failed for {release_tag}"


def compose_issue_body(
    repository: str,
    release_tag: str,
    failure_stage: str,
    failure_message: str,
    workflow_name: str,
    workflow_run_url: str,
) -> str:
    return (
        "Landfall could not complete release-note synthesis for a published release.\n\n"
        f"- Repository: `{repository}`\n"
        f"- Release tag: `{release_tag}`\n"
        f"- Failure stage: {describe_failure_stage(failure_stage)}\n"
        f"- Workflow: `{workflow_name}`\n"
        f"- Workflow run: {workflow_run_url}\n\n"
        "### Failure details\n"
        f"{failure_message.strip()}\n\n"
        "_Created automatically by Landfall._\n"
    )


def find_existing_failure_issue(
    api_base_url: str,
    repository: str,
    title: str,
    headers: dict[str, str],
    timeout: int,
    session: requests.Session | None = None,
) -> dict[str, Any] | None:
    """Return an existing open issue with the exact title, or None."""
    url = f"{api_base_url}/repos/{repository}/issues"
    params = {"state": "open", "per_page": 100}

    created_session = session is None
    http = session or requests.Session()
    try:
        response = http.get(url=url, headers=headers, params=params, timeout=timeout)
        response.raise_for_status()
        issues = response.json()
    finally:
        if created_session:
            http.close()

    for issue in issues:
        if isinstance(issue, dict) and issue.get("title") == title:
            return issue
    return None


def create_issue(
    api_base_url: str,
    repository: str,
    headers: dict[str, str],
    title: str,
    body: str,
    timeout: int,
    session: requests.Session | None = None,
) -> dict[str, Any]:
    url = f"{api_base_url}/repos/{repository}/issues"
    payload = {"title": title, "body": body}

    created_session = session is None
    http = session or requests.Session()
    try:
        response = http.post(url=url, headers=headers, json=payload, timeout=timeout)
        response.raise_for_status()
        return response.json()
    finally:
        if created_session:
            http.close()


def main() -> int:
    args = parse_args()
    configure_logging(args.log_level)

    try:
        validate_args(args)
    except ValueError as exc:
        log_event(logging.ERROR, "invalid_input", error=str(exc))
        return 1

    title = compose_issue_title(args.release_tag)
    headers = github_headers(args.github_token)

    # Deduplicate: skip creation if an open issue with the same title already exists.
    try:
        existing = find_existing_failure_issue(
            api_base_url=args.api_base_url,
            repository=args.repository,
            title=title,
            headers=headers,
            timeout=args.timeout,
        )
    except requests.RequestException:
        existing = None  # If search fails, proceed with creation

    if existing:
        existing_url = existing.get("html_url", "")
        log_event(
            logging.INFO,
            "duplicate_failure_issue_skipped",
            repository=args.repository,
            release_tag=args.release_tag,
            existing_issue_number=existing.get("number"),
            existing_issue_url=existing_url,
        )
        print(existing_url)
        return 0

    body = compose_issue_body(
        repository=args.repository,
        release_tag=args.release_tag,
        failure_stage=args.failure_stage,
        failure_message=args.failure_message,
        workflow_name=args.workflow_name,
        workflow_run_url=args.workflow_run_url,
    )

    try:
        issue = create_issue(
            api_base_url=args.api_base_url,
            repository=args.repository,
            headers=headers,
            title=title,
            body=body,
            timeout=args.timeout,
        )
    except requests.HTTPError as exc:
        status = exc.response.status_code if exc.response is not None else "unknown"
        text = exc.response.text if exc.response is not None else str(exc)
        log_event(
            logging.ERROR,
            "github_issue_create_http_error",
            status_code=status,
            response=text[:500],
            repository=args.repository,
        )
        return 1
    except requests.RequestException as exc:
        log_event(
            logging.ERROR,
            "github_issue_create_request_error",
            error_type=type(exc).__name__,
            error=str(exc),
            repository=args.repository,
        )
        return 1

    issue_url = issue.get("html_url", "")
    log_event(
        logging.INFO,
        "synthesis_failure_issue_created",
        repository=args.repository,
        release_tag=args.release_tag,
        failure_stage=args.failure_stage,
        issue_url=issue_url,
    )
    print(issue_url)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
