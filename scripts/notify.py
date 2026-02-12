#!/usr/bin/env python3
"""Send webhook notification on release with optional HMAC-SHA256 signing."""

from __future__ import annotations

import argparse
import datetime
import hashlib
import hmac
import html
import json
import logging
import re
from pathlib import Path
from urllib.parse import urlparse

import requests

from shared import configure_logging, log_event, request_with_retry

LOGGER = logging.getLogger("landfall.notify")
REPOSITORY_RE = re.compile(r"^[^/\s]+/[^/\s]+$")
MARKDOWN_STRONG_RE = re.compile(r"\*\*(.+?)\*\*")
MARKDOWN_LINK_RE = re.compile(r"\[([^\]]+)\]\(([^)]+)\)")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="POST release webhook notification.")
    parser.add_argument("--webhook-url", required=True, help="Webhook endpoint URL.")
    parser.add_argument("--webhook-secret", default="", help="HMAC-SHA256 signing secret.")
    parser.add_argument("--version", required=True, help="Release tag (e.g. v1.2.3).")
    parser.add_argument("--repository", required=True, help="GitHub repository (owner/repo).")
    parser.add_argument("--release-url", required=True, help="GitHub Release URL.")
    parser.add_argument("--notes-file", required=True, help="Path to synthesized notes markdown.")
    parser.add_argument("--timeout", type=int, default=10, help="HTTP timeout seconds.")
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
    if not args.webhook_url or not args.webhook_url.strip():
        raise ValueError("webhook-url must be non-empty")
    if not args.webhook_url.startswith(("http://", "https://")):
        raise ValueError("webhook-url must start with http:// or https://")
    if not args.version or not args.version.strip():
        raise ValueError("version must be non-empty")
    if not args.repository or not REPOSITORY_RE.match(args.repository):
        raise ValueError("repository must match owner/repo")
    if args.timeout <= 0:
        raise ValueError("timeout must be greater than zero")
    if args.retries < 0:
        raise ValueError("retries cannot be negative")
    if args.retry_backoff < 0:
        raise ValueError("retry-backoff cannot be negative")


# --- Lightweight markdown conversion (subset for webhook payload) ---


def _md_inline_to_plaintext(text: str) -> str:
    stripped = text.strip()
    if not stripped:
        return ""
    stripped = MARKDOWN_LINK_RE.sub(lambda m: f"{m.group(1)} ({m.group(2)})" if m.group(2) else m.group(1), stripped)
    stripped = re.sub(r"`([^`]+)`", r"\1", stripped)
    stripped = MARKDOWN_STRONG_RE.sub(r"\1", stripped)
    stripped = stripped.replace("*", "").replace("_", "")
    return re.sub(r"[ \t]+", " ", stripped).strip()


def markdown_to_plaintext(markdown: str) -> str:
    lines = markdown.splitlines()
    rendered: list[str] = []
    for raw in lines:
        line = raw.strip()
        if not line:
            rendered.append("")
        elif line.startswith("## "):
            rendered.append(_md_inline_to_plaintext(line[3:]))
            rendered.append("")
        elif line.startswith("- "):
            rendered.append(f"- {_md_inline_to_plaintext(line[2:])}")
        else:
            rendered.append(_md_inline_to_plaintext(line))
    text = "\n".join(rendered).strip()
    return re.sub(r"\n{3,}", "\n\n", text) if text else ""


def _safe_link_href(url: str) -> str | None:
    parsed = urlparse(url.strip())
    if parsed.scheme in ("http", "https"):
        return url.strip()
    return None


def _md_inline_to_html(text: str) -> str:
    out: list[str] = []
    i = 0
    while i < len(text):
        if text.startswith("**", i):
            end = text.find("**", i + 2)
            if end != -1:
                out.append(f"<strong>{html.escape(text[i + 2:end], quote=True)}</strong>")
                i = end + 2
                continue
        if text[i] == "`":
            end = text.find("`", i + 1)
            if end != -1:
                out.append(f"<code>{html.escape(text[i + 1:end], quote=True)}</code>")
                i = end + 1
                continue
        if text[i] == "[":
            mid = text.find("](", i + 1)
            if mid != -1:
                end = text.find(")", mid + 2)
                if end != -1:
                    label = text[i + 1:mid]
                    url = text[mid + 2:end]
                    href = _safe_link_href(url)
                    if href:
                        out.append(f'<a href="{html.escape(href, quote=True)}">{html.escape(label, quote=True)}</a>')
                    else:
                        out.append(html.escape(label, quote=True))
                    i = end + 1
                    continue
        out.append(html.escape(text[i], quote=True))
        i += 1
    return "".join(out)


def markdown_to_html_fragment(markdown: str) -> str:
    lines = markdown.splitlines()
    rendered: list[str] = []
    in_list = False

    def close_list() -> None:
        nonlocal in_list
        if in_list:
            rendered.append("</ul>")
            in_list = False

    for raw in lines:
        line = raw.strip()
        if not line:
            close_list()
            continue
        if line.startswith("## "):
            close_list()
            rendered.append(f"<h2>{_md_inline_to_html(line[3:])}</h2>")
            continue
        if line.startswith("- "):
            if not in_list:
                rendered.append("<ul>")
                in_list = True
            rendered.append(f"<li>{_md_inline_to_html(line[2:])}</li>")
            continue
        close_list()
        rendered.append(f"<p>{_md_inline_to_html(line)}</p>")

    close_list()
    return "\n".join(rendered).strip()


# --- Core functions ---


def build_payload(
    *,
    version: str,
    repository: str,
    release_url: str,
    notes_markdown: str,
) -> dict:
    return {
        "version": version,
        "repository": repository,
        "release_url": release_url,
        "notes": notes_markdown,
        "notes_html": markdown_to_html_fragment(notes_markdown),
        "notes_plaintext": markdown_to_plaintext(notes_markdown),
        "timestamp": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    }


def compute_signature(secret: str | None, body: bytes) -> str | None:
    if not secret:
        return None
    return "sha256=" + hmac.new(secret.encode("utf-8"), body, hashlib.sha256).hexdigest()


def send_webhook(
    *,
    webhook_url: str,
    payload: dict,
    webhook_secret: str,
    timeout: int,
    retries: int,
    retry_backoff: float,
    session: requests.Session | None = None,
) -> None:
    body = json.dumps(payload, sort_keys=True).encode("utf-8")
    headers: dict[str, str] = {"Content-Type": "application/json"}
    signature = compute_signature(webhook_secret, body)
    if signature:
        headers["X-Signature-256"] = signature

    created_session = session is None
    http = session or requests.Session()
    try:
        request_with_retry(
            LOGGER,
            http,
            "POST",
            webhook_url,
            headers=headers,
            data=body,
            timeout=timeout,
            retries=retries,
            retry_backoff=retry_backoff,
        )
    finally:
        if created_session:
            http.close()


def main() -> int:
    args = parse_args()
    configure_logging(args.log_level)

    try:
        validate_args(args)
    except ValueError as exc:
        log_event(LOGGER, logging.ERROR, "invalid_input", error=str(exc))
        return 1

    try:
        notes = Path(args.notes_file).read_text(encoding="utf-8").strip()
    except OSError as exc:
        log_event(LOGGER, logging.ERROR, "notes_read_failed", path=args.notes_file, error=str(exc))
        return 1

    if not notes:
        log_event(LOGGER, logging.ERROR, "empty_notes_file", path=args.notes_file)
        return 1

    payload = build_payload(
        version=args.version,
        repository=args.repository,
        release_url=args.release_url,
        notes_markdown=notes,
    )

    try:
        send_webhook(
            webhook_url=args.webhook_url,
            payload=payload,
            webhook_secret=args.webhook_secret,
            timeout=args.timeout,
            retries=args.retries,
            retry_backoff=args.retry_backoff,
        )
    except requests.HTTPError as exc:
        status = exc.response.status_code if exc.response is not None else "unknown"
        text = exc.response.text if exc.response is not None else str(exc)
        log_event(LOGGER, logging.ERROR, "webhook_http_error", status_code=status, response_body=text)
        return 1
    except requests.RequestException as exc:
        log_event(LOGGER, logging.ERROR, "webhook_request_failed", error=str(exc))
        return 1

    redacted_url = urlparse(args.webhook_url).hostname or "unknown"
    log_event(LOGGER, logging.INFO, "webhook_sent", host=redacted_url, version=args.version)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
