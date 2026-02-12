#!/usr/bin/env python3
"""Generate user-facing release notes from technical changelog content."""

from __future__ import annotations

import argparse
import logging
import os
import re
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

import requests

from shared import configure_logging, log_event, request_with_retry


DEFAULT_API_URL = "https://openrouter.ai/api/v1/chat/completions"
SECTION_HEADING_RE = re.compile(r"^##\s+.+$", re.MULTILINE)
REQUIRED_TEMPLATE_TOKENS = (
    "{{PRODUCT_NAME}}",
    "{{VERSION}}",
    "{{TECHNICAL_CHANGELOG}}",
)
BUILT_IN_PROMPT_TEMPLATES = {
    "general": "general.md",
    "developer": "developer.md",
    "end-user": "end-user.md",
    "enterprise": "enterprise.md",
}
CHANGELOG_SOURCES = ("auto", "changelog", "release-body", "prs")
LOGGER = logging.getLogger("landfall.synthesize")


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
        default="",
        help=(
            "Path to prompt template markdown file. "
            "When omitted, uses the built-in template for --audience."
        ),
    )
    parser.add_argument(
        "--audience",
        default="general",
        help=(
            "Built-in prompt variant to use when --prompt-template is not provided. "
            "One of: general, developer, end-user, enterprise."
        ),
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
        "--changelog-source",
        default="auto",
        help="Technical changelog source: auto, changelog, release-body, or prs.",
    )
    parser.add_argument(
        "--release-body-file",
        default="",
        help="Optional path to a file containing release-body markdown.",
    )
    parser.add_argument(
        "--pr-changelog-file",
        default="",
        help="Optional path to a file containing PR-derived changelog markdown.",
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
        log_event(LOGGER, logging.WARNING, "insecure_api_url", url=args.api_url)
    if args.version is not None and not args.version.strip():
        raise ValueError("version cannot be blank when provided")
    prompt_template = getattr(args, "prompt_template", None)
    if prompt_template is not None and prompt_template != "" and not prompt_template.strip():
        raise ValueError("prompt-template cannot be blank when provided")
    audience = getattr(args, "audience", "general")
    if audience is not None and not str(audience).strip():
        raise ValueError("audience must be non-empty")
    normalize_audience(str(audience))
    changelog_source = getattr(args, "changelog_source", "auto")
    if changelog_source is not None and not str(changelog_source).strip():
        raise ValueError("changelog-source must be non-empty")
    normalize_changelog_source(str(changelog_source))


def normalize_version(version: str) -> str:
    return version.strip().lstrip("v")


def normalize_changelog_source(changelog_source: str) -> str:
    source_key = changelog_source.strip().lower()
    if source_key not in CHANGELOG_SOURCES:
        valid_sources = ", ".join(CHANGELOG_SOURCES)
        raise ValueError(f"changelog-source must be one of: {valid_sources}")
    return source_key


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
                LOGGER,
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


def resolve_technical_changelog(
    *,
    changelog_source: str,
    version: str | None,
    changelog_file: Path,
    release_body_file: Path | None,
    pr_changelog_file: Path | None,
) -> tuple[str, str]:
    source_key = normalize_changelog_source(changelog_source)
    candidates: list[tuple[str, Path | None]]

    if source_key == "auto":
        candidates = [
            ("changelog", changelog_file),
            ("release-body", release_body_file),
            ("prs", pr_changelog_file),
        ]
    elif source_key == "changelog":
        candidates = [("changelog", changelog_file)]
    elif source_key == "release-body":
        candidates = [("release-body", release_body_file)]
    else:
        candidates = [("prs", pr_changelog_file)]

    for name, path in candidates:
        if path is None:
            continue
        try:
            if name == "changelog":
                changelog_text = read_text(path)
                if not changelog_text:
                    continue
                technical = extract_release_section(changelog_text, version)
            else:
                technical = read_text(path)
        except OSError:
            continue

        technical = technical.strip()
        if technical:
            return technical, name

    if source_key == "auto":
        raise ValueError("no technical changelog source available for changelog-source 'auto'")
    raise ValueError(f"selected changelog-source '{source_key}' is unavailable")


BREAKING_CHANGE_RE = re.compile(r"^#{1,4}\s+BREAKING\s+CHANGES?", re.MULTILINE | re.IGNORECASE)

SIGNIFICANCE_BULLET_MAP = {
    "major": "5-10",
    "feature": "3-7",
    "patch": "1-3",
}


def classify_release(version: str, technical: str) -> tuple[str, str]:
    """Classify release significance and suggest bullet count range.

    Returns (significance, bullet_target) where significance is one of
    'major', 'feature', or 'patch'.
    """
    normalized = normalize_version(version)
    # Strip prerelease/build metadata (e.g. "1.2.0-rc.1" → "1.2.0")
    normalized = re.split(r"[-+]", normalized, maxsplit=1)[0]
    parts = normalized.split(".")
    # Pad to 3 parts for partial versions (e.g. "2" → ["2","0","0"])
    while len(parts) < 3:
        parts.append("0")

    has_breaking = bool(BREAKING_CHANGE_RE.search(technical))

    if has_breaking:
        significance = "major"
    elif parts[2] == "0" and parts[1] == "0" and parts[0] != "0":
        significance = "major"
    elif parts[2] != "0":
        significance = "patch"
    else:
        significance = "feature"

    return significance, SIGNIFICANCE_BULLET_MAP[significance]


def estimate_bullet_target(version: str, technical: str) -> str:
    """Suggest a bullet count range based on version bump and changelog size."""
    _, bullet_target = classify_release(version, technical)
    return bullet_target


def render_prompt(template_text: str, product_name: str, version: str, technical: str) -> str:
    bullet_target = estimate_bullet_target(version, technical)
    return (
        template_text.replace("{{PRODUCT_NAME}}", product_name)
        .replace("{{VERSION}}", version)
        .replace("{{BULLET_TARGET}}", bullet_target)
        .replace("{{TECHNICAL_CHANGELOG}}", technical)
    )


def validate_template_tokens(template_text: str) -> None:
    missing = [token for token in REQUIRED_TEMPLATE_TOKENS if token not in template_text]
    if missing:
        raise ValueError(f"prompt template missing required token(s): {', '.join(missing)}")


def normalize_audience(audience: str) -> str:
    audience_key = audience.strip().lower()
    if audience_key not in BUILT_IN_PROMPT_TEMPLATES:
        valid_audiences = ", ".join(BUILT_IN_PROMPT_TEMPLATES.keys())
        raise ValueError(f"audience must be one of: {valid_audiences}")
    return audience_key


def resolve_prompt_template_path(prompt_template: str | None, audience: str) -> Path:
    if prompt_template and prompt_template.strip():
        return Path(prompt_template)

    audience_key = normalize_audience(audience)

    return (
        Path(__file__).resolve().parents[1]
        / "templates"
        / "prompts"
        / BUILT_IN_PROMPT_TEMPLATES[audience_key]
    )


def infer_product_name(explicit_name: str | None) -> str:
    if explicit_name:
        return explicit_name
    repository = os.getenv("GITHUB_REPOSITORY", "")
    if "/" in repository:
        return repository.split("/", 1)[1]
    return "this product"


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
                "content": (
                    "You are a technical writer who transforms developer changelogs into "
                    "release notes that users actually want to read. "
                    "Explain what changed and why it matters. "
                    "For new features, frame as 'You can now...' to highlight capability. "
                    "For bug fixes, frame as 'Fixed...' to confirm resolution. "
                    "For improvements, frame as 'The X now...' to show what got better. "
                    "Never leak implementation details: no PR numbers, commit hashes, "
                    "file paths, function names, or internal process references. "
                    "Skip CI, tooling, refactors, and dependency bumps unless user-visible."
                ),
            },
            {"role": "user", "content": prompt},
        ],
    }

    created_session = session is None
    http = session or requests.Session()

    try:
        response = request_with_retry(
            LOGGER,
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
        template_path = resolve_prompt_template_path(args.prompt_template, args.audience)
    except ValueError as exc:
        log_event(LOGGER, logging.ERROR, "invalid_input", error=str(exc))
        return 1

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

    changelog_path = Path(args.changelog_file)
    release_body_path = Path(args.release_body_file) if args.release_body_file else None
    pr_changelog_path = Path(args.pr_changelog_file) if args.pr_changelog_file else None

    try:
        if args.technical_changelog_file:
            technical_path = Path(args.technical_changelog_file)
            technical_text = read_text(technical_path)
            source_used = "technical-changelog-file"
        else:
            technical_text, source_used = resolve_technical_changelog(
                changelog_source=args.changelog_source,
                version=args.version,
                changelog_file=changelog_path,
                release_body_file=release_body_path,
                pr_changelog_file=pr_changelog_path,
            )
    except OSError as exc:
        log_event(
            LOGGER,
            logging.ERROR,
            "changelog_read_failed",
            path=str(changelog_path),
            error=str(exc),
        )
        return 1
    except ValueError as exc:
        log_event(LOGGER, logging.ERROR, "changelog_source_unavailable", error=str(exc))
        return 1

    if not technical_text:
        log_event(LOGGER, logging.ERROR, "empty_changelog")
        return 1

    log_event(LOGGER, logging.INFO, "changelog_source_selected", source=source_used)

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
    status_codes: list[int] = []
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
            log_event(LOGGER, logging.INFO, "synthesis_succeeded", model=model)
            print(synthesized)
            return 0
        except (requests.HTTPError, requests.RequestException, RuntimeError) as exc:
            last_error = exc
            error_fields: dict[str, Any] = {"error": str(exc)}
            if isinstance(exc, requests.HTTPError) and exc.response is not None:
                error_fields["status_code"] = exc.response.status_code
                error_fields["response_body"] = exc.response.text
                status_codes.append(exc.response.status_code)
            log_event(LOGGER, logging.WARNING, "model_failed", model=model, **error_fields)
            continue

    # Surface actionable diagnosis for common failure patterns
    if status_codes and all(code == 401 for code in status_codes):
        log_event(
            LOGGER,
            logging.ERROR,
            "authentication_failed",
            models_tried=models_to_try,
            message=(
                "API key rejected by provider (HTTP 401). "
                "Verify your llm-api-key secret is a valid API key for the configured provider."
            ),
        )
    elif status_codes and all(code == 403 for code in status_codes):
        log_event(
            LOGGER,
            logging.ERROR,
            "authorization_failed",
            models_tried=models_to_try,
            message=(
                "API key lacks required permissions (HTTP 403). "
                "Check your provider account for rate limits, billing, or model access restrictions."
            ),
        )
    else:
        log_event(
            LOGGER,
            logging.ERROR,
            "all_models_failed",
            models_tried=models_to_try,
            last_error=str(last_error) if last_error is not None else "",
        )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
