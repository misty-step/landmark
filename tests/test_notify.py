from __future__ import annotations

import argparse
import hashlib
import hmac
from unittest.mock import patch

import pytest

from conftest import FakeResponse, load_script_module


@pytest.fixture(scope="session")
def notify():
    return load_script_module("landfall_notify", "scripts/notify.py")


# --- validate_args ---


def test_validate_args_accepts_valid_inputs(notify):
    args = argparse.Namespace(
        webhook_url="https://hooks.example.com/release",
        webhook_secret="s3cret",
        version="v1.2.3",
        repository="octo/example",
        release_url="https://github.com/octo/example/releases/tag/v1.2.3",
        notes_file="notes.md",
        timeout=10,
        retries=2,
        retry_backoff=1.0,
        log_level="INFO",
    )
    notify.validate_args(args)


def test_validate_args_rejects_empty_webhook_url(notify):
    args = argparse.Namespace(
        webhook_url="",
        webhook_secret="",
        version="v1.2.3",
        repository="octo/example",
        release_url="https://github.com/octo/example/releases/tag/v1.2.3",
        notes_file="notes.md",
        timeout=10,
        retries=2,
        retry_backoff=1.0,
        log_level="INFO",
    )
    with pytest.raises(ValueError, match="webhook-url must be non-empty"):
        notify.validate_args(args)


def test_validate_args_rejects_non_http_webhook_url(notify):
    args = argparse.Namespace(
        webhook_url="ftp://hooks.example.com",
        webhook_secret="",
        version="v1.2.3",
        repository="octo/example",
        release_url="https://github.com/octo/example/releases/tag/v1.2.3",
        notes_file="notes.md",
        timeout=10,
        retries=2,
        retry_backoff=1.0,
        log_level="INFO",
    )
    with pytest.raises(ValueError, match="webhook-url must start with http"):
        notify.validate_args(args)


def test_validate_args_rejects_empty_version(notify):
    args = argparse.Namespace(
        webhook_url="https://hooks.example.com/release",
        webhook_secret="",
        version="  ",
        repository="octo/example",
        release_url="https://github.com/octo/example/releases/tag/v1.2.3",
        notes_file="notes.md",
        timeout=10,
        retries=2,
        retry_backoff=1.0,
        log_level="INFO",
    )
    with pytest.raises(ValueError, match="version must be non-empty"):
        notify.validate_args(args)


def test_validate_args_rejects_invalid_repository(notify):
    args = argparse.Namespace(
        webhook_url="https://hooks.example.com/release",
        webhook_secret="",
        version="v1.2.3",
        repository="not-a-repo",
        release_url="https://github.com/octo/example/releases/tag/v1.2.3",
        notes_file="notes.md",
        timeout=10,
        retries=2,
        retry_backoff=1.0,
        log_level="INFO",
    )
    with pytest.raises(ValueError, match="repository must match owner/repo"):
        notify.validate_args(args)


def test_validate_args_rejects_non_positive_timeout(notify):
    args = argparse.Namespace(
        webhook_url="https://hooks.example.com/release",
        webhook_secret="",
        version="v1.2.3",
        repository="octo/example",
        release_url="https://github.com/octo/example/releases/tag/v1.2.3",
        notes_file="notes.md",
        timeout=0,
        retries=2,
        retry_backoff=1.0,
        log_level="INFO",
    )
    with pytest.raises(ValueError, match="timeout must be greater than zero"):
        notify.validate_args(args)


# --- build_payload ---


def test_build_payload_contains_all_fields(notify):
    notes_md = "## What's New\n\n- Faster startup"
    payload = notify.build_payload(
        version="v1.2.3",
        repository="octo/example",
        release_url="https://github.com/octo/example/releases/tag/v1.2.3",
        notes_markdown=notes_md,
    )
    assert payload["version"] == "v1.2.3"
    assert payload["repository"] == "octo/example"
    assert payload["release_url"] == "https://github.com/octo/example/releases/tag/v1.2.3"
    assert payload["notes"] == notes_md
    assert "notes_html" in payload
    assert "notes_plaintext" in payload
    assert "timestamp" in payload


def test_build_payload_html_contains_markup(notify):
    notes_md = "## What's New\n\n- Faster startup"
    payload = notify.build_payload(
        version="v1.2.3",
        repository="octo/example",
        release_url="https://github.com/octo/example/releases/tag/v1.2.3",
        notes_markdown=notes_md,
    )
    assert "<" in payload["notes_html"]
    assert "Faster startup" in payload["notes_html"]


def test_build_payload_plaintext_strips_markdown(notify):
    notes_md = "## What's New\n\n- **Bold** feature"
    payload = notify.build_payload(
        version="v1.2.3",
        repository="octo/example",
        release_url="https://github.com/octo/example/releases/tag/v1.2.3",
        notes_markdown=notes_md,
    )
    assert "**" not in payload["notes_plaintext"]
    assert "Bold feature" in payload["notes_plaintext"]


def test_build_payload_html_rejects_javascript_urls(notify):
    notes_md = "- [click](javascript:alert(1))"
    payload = notify.build_payload(
        version="v1.0.0",
        repository="a/b",
        release_url="https://github.com/a/b/releases/tag/v1.0.0",
        notes_markdown=notes_md,
    )
    assert "javascript:" not in payload["notes_html"]
    assert "click" in payload["notes_html"]


def test_build_payload_timestamp_is_iso8601(notify):
    payload = notify.build_payload(
        version="v1.0.0",
        repository="a/b",
        release_url="https://github.com/a/b/releases/tag/v1.0.0",
        notes_markdown="notes",
    )
    ts = payload["timestamp"]
    assert ts.endswith("Z") or "+" in ts
    assert "T" in ts


# --- compute_signature ---


def test_compute_signature_matches_expected_hmac(notify):
    secret = "test-secret"
    body = b'{"version":"1.0.0"}'
    expected = "sha256=" + hmac.new(
        secret.encode("utf-8"), body, hashlib.sha256
    ).hexdigest()
    assert notify.compute_signature(secret, body) == expected


def test_compute_signature_returns_none_when_no_secret(notify):
    assert notify.compute_signature("", b"data") is None
    assert notify.compute_signature(None, b"data") is None


# --- send_webhook ---


def test_send_webhook_posts_payload(notify, request_session_factory):
    session = request_session_factory(
        outcomes=[FakeResponse(status_code=200, text="OK")]
    )
    payload = {"version": "1.0.0", "notes": "test"}

    notify.send_webhook(
        webhook_url="https://hooks.example.com/release",
        payload=payload,
        webhook_secret="",
        timeout=10,
        retries=0,
        retry_backoff=0.0,
        session=session,
    )

    assert len(session.calls) == 1
    call = session.calls[0]
    assert call["method"] == "POST"
    assert call["url"] == "https://hooks.example.com/release"


def test_send_webhook_includes_signature_header(notify, request_session_factory):
    session = request_session_factory(
        outcomes=[FakeResponse(status_code=200, text="OK")]
    )
    payload = {"version": "1.0.0"}
    secret = "my-secret"

    notify.send_webhook(
        webhook_url="https://hooks.example.com/release",
        payload=payload,
        webhook_secret=secret,
        timeout=10,
        retries=0,
        retry_backoff=0.0,
        session=session,
    )

    call = session.calls[0]
    headers = call["kwargs"]["headers"]
    assert "X-Signature-256" in headers
    assert headers["X-Signature-256"].startswith("sha256=")


def test_send_webhook_omits_signature_header_without_secret(notify, request_session_factory):
    session = request_session_factory(
        outcomes=[FakeResponse(status_code=200, text="OK")]
    )
    payload = {"version": "1.0.0"}

    notify.send_webhook(
        webhook_url="https://hooks.example.com/release",
        payload=payload,
        webhook_secret="",
        timeout=10,
        retries=0,
        retry_backoff=0.0,
        session=session,
    )

    call = session.calls[0]
    headers = call["kwargs"]["headers"]
    assert "X-Signature-256" not in headers


def test_send_webhook_sends_json_content_type(notify, request_session_factory):
    session = request_session_factory(
        outcomes=[FakeResponse(status_code=200, text="OK")]
    )

    notify.send_webhook(
        webhook_url="https://hooks.example.com/release",
        payload={"version": "1.0.0"},
        webhook_secret="",
        timeout=10,
        retries=0,
        retry_backoff=0.0,
        session=session,
    )

    call = session.calls[0]
    headers = call["kwargs"]["headers"]
    assert headers["Content-Type"] == "application/json"


# --- main integration ---


def test_main_returns_0_on_success(notify, tmp_path, request_session_factory):
    notes_file = tmp_path / "notes.md"
    notes_file.write_text("## What's New\n\n- Feature A")

    session = request_session_factory(
        outcomes=[FakeResponse(status_code=200, text="OK")]
    )

    with patch.object(notify, "parse_args", return_value=argparse.Namespace(
        webhook_url="https://hooks.example.com/release",
        webhook_secret="secret",
        version="v1.2.3",
        repository="octo/example",
        release_url="https://github.com/octo/example/releases/tag/v1.2.3",
        notes_file=str(notes_file),
        timeout=10,
        retries=0,
        retry_backoff=0.0,
        log_level="INFO",
    )), patch("requests.Session", return_value=session):
        result = notify.main()

    assert result == 0
    assert len(session.calls) == 1


def test_main_returns_1_on_missing_notes_file(notify, tmp_path):
    with patch.object(notify, "parse_args", return_value=argparse.Namespace(
        webhook_url="https://hooks.example.com/release",
        webhook_secret="",
        version="v1.2.3",
        repository="octo/example",
        release_url="https://github.com/octo/example/releases/tag/v1.2.3",
        notes_file=str(tmp_path / "missing.md"),
        timeout=10,
        retries=0,
        retry_backoff=0.0,
        log_level="INFO",
    )):
        result = notify.main()

    assert result == 1


def test_main_returns_1_on_empty_notes_file(notify, tmp_path):
    notes_file = tmp_path / "empty.md"
    notes_file.write_text("   ")

    with patch.object(notify, "parse_args", return_value=argparse.Namespace(
        webhook_url="https://hooks.example.com/release",
        webhook_secret="",
        version="v1.2.3",
        repository="octo/example",
        release_url="https://github.com/octo/example/releases/tag/v1.2.3",
        notes_file=str(notes_file),
        timeout=10,
        retries=0,
        retry_backoff=0.0,
        log_level="INFO",
    )):
        result = notify.main()

    assert result == 1


def test_main_returns_1_on_validation_error(notify):
    with patch.object(notify, "parse_args", return_value=argparse.Namespace(
        webhook_url="ftp://bad",
        webhook_secret="",
        version="v1.2.3",
        repository="octo/example",
        release_url="https://github.com/octo/example/releases/tag/v1.2.3",
        notes_file="notes.md",
        timeout=10,
        retries=0,
        retry_backoff=0.0,
        log_level="INFO",
    )):
        result = notify.main()

    assert result == 1
