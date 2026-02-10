from __future__ import annotations

import argparse
import logging

import pytest
import requests

import synthesize


def test_extract_release_section_finds_matching_version():
    # Arrange
    changelog = (
        "## [1.2.0] - 2026-02-09\n\n- newest\n\n"
        "## [1.1.0] - 2026-02-01\n\n- older\n"
    )

    # Act
    section = synthesize.extract_release_section(changelog, "1.1.0")

    # Assert
    assert section.startswith("## [1.1.0]")
    assert "- older" in section
    assert "- newest" not in section


def test_extract_release_section_falls_back_to_latest_when_version_missing():
    # Arrange
    changelog = "## 1.2.0\n\n- newest\n\n## 1.1.0\n\n- older\n"

    # Act
    section = synthesize.extract_release_section(changelog, "9.9.9")

    # Assert
    assert section.startswith("## 1.2.0")
    assert "- newest" in section


def test_extract_release_section_returns_full_text_when_no_headings():
    # Arrange
    changelog = "### Features\n- a\n\n### Fixes\n- b\n"

    # Act
    section = synthesize.extract_release_section(changelog, version=None)

    # Assert
    assert section == changelog.strip()


def test_render_prompt_replaces_template_tokens():
    # Arrange
    template = "Name={{PRODUCT_NAME}} Version={{VERSION}}\n\n{{TECHNICAL_CHANGELOG}}"

    # Act
    rendered = synthesize.render_prompt(
        template_text=template,
        product_name="Landfall",
        version="1.2.3",
        technical="### Fixes\n- stability",
    )

    # Assert
    assert rendered == "Name=Landfall Version=1.2.3\n\n### Fixes\n- stability"


def test_validate_args_accepts_valid_inputs():
    # Arrange
    args = argparse.Namespace(
        api_key="secret",
        model="test-model",
        timeout=10,
        retries=0,
        retry_backoff=0.0,
        api_url="https://api.example.test/chat/completions",
        version=None,
    )

    # Act / Assert
    synthesize.validate_args(args)


def test_validate_args_rejects_blank_api_key():
    # Arrange
    args = argparse.Namespace(
        api_key="   ",
        model="test-model",
        timeout=10,
        retries=0,
        retry_backoff=0.0,
        api_url="https://api.example.test/chat/completions",
        version=None,
    )

    # Act / Assert
    with pytest.raises(ValueError, match="api-key must be non-empty"):
        synthesize.validate_args(args)


def test_validate_args_rejects_blank_model():
    # Arrange
    args = argparse.Namespace(
        api_key="secret",
        model=" ",
        timeout=10,
        retries=0,
        retry_backoff=0.0,
        api_url="https://api.example.test/chat/completions",
        version=None,
    )

    # Act / Assert
    with pytest.raises(ValueError, match="model must be non-empty"):
        synthesize.validate_args(args)


def test_validate_args_rejects_non_positive_timeout():
    # Arrange
    args = argparse.Namespace(
        api_key="secret",
        model="test-model",
        timeout=0,
        retries=0,
        retry_backoff=0.0,
        api_url="https://api.example.test/chat/completions",
        version=None,
    )

    # Act / Assert
    with pytest.raises(ValueError, match="timeout must be greater than zero"):
        synthesize.validate_args(args)


def test_validate_args_rejects_negative_retries():
    # Arrange
    args = argparse.Namespace(
        api_key="secret",
        model="test-model",
        timeout=10,
        retries=-1,
        retry_backoff=0.0,
        api_url="https://api.example.test/chat/completions",
        version=None,
    )

    # Act / Assert
    with pytest.raises(ValueError, match="retries cannot be negative"):
        synthesize.validate_args(args)


def test_validate_args_rejects_negative_retry_backoff():
    # Arrange
    args = argparse.Namespace(
        api_key="secret",
        model="test-model",
        timeout=10,
        retries=0,
        retry_backoff=-0.5,
        api_url="https://api.example.test/chat/completions",
        version=None,
    )

    # Act / Assert
    with pytest.raises(ValueError, match="retry-backoff cannot be negative"):
        synthesize.validate_args(args)


def test_validate_args_rejects_invalid_api_url_scheme():
    # Arrange
    args = argparse.Namespace(
        api_key="secret",
        model="test-model",
        timeout=10,
        retries=0,
        retry_backoff=0.0,
        api_url="ftp://example.test",
        version=None,
    )

    # Act / Assert
    with pytest.raises(ValueError, match="api-url must start with http:// or https://"):
        synthesize.validate_args(args)


def test_validate_args_rejects_blank_version_when_provided():
    # Arrange
    args = argparse.Namespace(
        api_key="secret",
        model="test-model",
        timeout=10,
        retries=0,
        retry_backoff=0.0,
        api_url="https://api.example.test/chat/completions",
        version="   ",
    )

    # Act / Assert
    with pytest.raises(ValueError, match="version cannot be blank when provided"):
        synthesize.validate_args(args)


def test_validate_args_logs_warning_for_insecure_non_local_http_url(monkeypatch):
    # Arrange
    events: list[dict[str, object]] = []

    def fake_log_event(_logger, level: int, event: str, **fields):
        events.append({"level": level, "event": event, **fields})

    monkeypatch.setattr(synthesize, "log_event", fake_log_event)
    args = argparse.Namespace(
        api_key="secret",
        model="test-model",
        timeout=10,
        retries=0,
        retry_backoff=0.0,
        api_url="http://example.test/chat/completions",
        version=None,
    )

    # Act
    synthesize.validate_args(args)

    # Assert
    assert events[0]["event"] == "insecure_api_url"
    assert events[0]["level"] == logging.WARNING


def test_validate_template_tokens_raises_when_missing_required_tokens():
    # Arrange
    template = "Hello {{PRODUCT_NAME}}"

    # Act / Assert
    with pytest.raises(ValueError) as excinfo:
        synthesize.validate_template_tokens(template)

    message = str(excinfo.value)
    assert "{{VERSION}}" in message
    assert "{{TECHNICAL_CHANGELOG}}" in message


def test_normalize_version_strips_whitespace_and_v_prefix():
    # Arrange / Act
    normalized = synthesize.normalize_version("  v1.2.3  ")

    # Assert
    assert normalized == "1.2.3"


def test_infer_product_name_uses_explicit_value(monkeypatch):
    # Arrange
    monkeypatch.delenv("GITHUB_REPOSITORY", raising=False)

    # Act
    name = synthesize.infer_product_name("Explicit")

    # Assert
    assert name == "Explicit"


def test_infer_product_name_uses_github_repository_env(monkeypatch):
    # Arrange
    monkeypatch.setenv("GITHUB_REPOSITORY", "octo/rocket")

    # Act
    name = synthesize.infer_product_name(None)

    # Assert
    assert name == "rocket"


def test_infer_product_name_falls_back_when_env_missing(monkeypatch):
    # Arrange
    monkeypatch.delenv("GITHUB_REPOSITORY", raising=False)

    # Act
    name = synthesize.infer_product_name(None)

    # Assert
    assert name == "this product"


def test_synthesize_notes_returns_content_and_uses_request_with_retry(monkeypatch, request_session_factory):
    # Arrange
    captured: dict[str, object] = {}

    def fake_request_with_retry(logger, session, method, url, **kwargs):
        captured["method"] = method
        captured["url"] = url
        captured["headers"] = kwargs["headers"]
        captured["json"] = kwargs["json"]
        return synthesize_test_response(
            status_code=200,
            json_data={"choices": [{"message": {"content": "  ## Notes\n- Faster  \n"}}]},
        )

    monkeypatch.setattr(synthesize, "request_with_retry", fake_request_with_retry)

    # Act
    notes = synthesize.synthesize_notes(
        api_url="https://api.example.test/chat/completions",
        api_key="secret",
        model="test-model",
        prompt="prompt text",
        timeout=5,
        retries=0,
        retry_backoff=0.0,
        session=request_session_factory([]),
    )

    # Assert
    assert notes == "## Notes\n- Faster"
    assert captured["method"] == "POST"
    assert captured["url"] == "https://api.example.test/chat/completions"
    assert str(captured["headers"]["Authorization"]).startswith("Bearer ")
    assert captured["json"]["model"] == "test-model"
    assert captured["json"]["messages"][1]["content"] == "prompt text"


def test_synthesize_notes_raises_when_provider_returns_empty_content(monkeypatch, request_session_factory):
    # Arrange
    def fake_request_with_retry(*_args, **_kwargs):
        return synthesize_test_response(
            status_code=200,
            json_data={"choices": [{"message": {"content": "   "}}]},
        )

    monkeypatch.setattr(synthesize, "request_with_retry", fake_request_with_retry)

    # Act / Assert
    with pytest.raises(RuntimeError, match="empty synthesized notes"):
        synthesize.synthesize_notes(
            api_url="https://api.example.test/chat/completions",
            api_key="secret",
            model="test-model",
            prompt="prompt text",
            timeout=5,
            retries=0,
            retry_backoff=0.0,
            session=request_session_factory([]),
        )


def test_synthesize_notes_raises_when_response_shape_missing_choices(monkeypatch, request_session_factory):
    # Arrange
    def fake_request_with_retry(*_args, **_kwargs):
        return synthesize_test_response(status_code=200, json_data={})

    monkeypatch.setattr(synthesize, "request_with_retry", fake_request_with_retry)

    # Act / Assert
    with pytest.raises(RuntimeError, match="did not include choices\\[0\\]\\.message\\.content"):
        synthesize.synthesize_notes(
            api_url="https://api.example.test/chat/completions",
            api_key="secret",
            model="test-model",
            prompt="prompt text",
            timeout=5,
            retries=0,
            retry_backoff=0.0,
            session=request_session_factory([]),
        )


def test_synthesize_notes_propagates_http_error(monkeypatch, request_session_factory):
    # Arrange
    def fake_request_with_retry(*_args, **_kwargs):
        raise requests.HTTPError("HTTP 500")

    monkeypatch.setattr(synthesize, "request_with_retry", fake_request_with_retry)

    # Act / Assert
    with pytest.raises(requests.HTTPError):
        synthesize.synthesize_notes(
            api_url="https://api.example.test/chat/completions",
            api_key="secret",
            model="test-model",
            prompt="prompt text",
            timeout=5,
            retries=0,
            retry_backoff=0.0,
            session=request_session_factory([]),
        )


def synthesize_test_response(*, status_code: int, json_data: object):
    class _Response:
        def __init__(self, status_code: int, json_data: object):
            self.status_code = status_code
            self._json_data = json_data

        def json(self) -> object:
            return self._json_data

        def raise_for_status(self) -> None:
            if self.status_code >= 400:
                error = requests.HTTPError(f"HTTP {self.status_code}")
                error.response = self
                raise error

    return _Response(status_code=status_code, json_data=json_data)

