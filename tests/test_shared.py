from __future__ import annotations

import json
import logging
import sys
from pathlib import Path

import pytest
import requests

import shared


class LoggerStub:
    def __init__(self) -> None:
        self.calls: list[tuple[int, str]] = []

    def log(self, level: int, message: str) -> None:
        self.calls.append((level, message))


def test_configure_logging_sets_level_format_and_stream(monkeypatch):
    # Arrange
    captured: dict[str, object] = {}

    def fake_basic_config(**kwargs):
        captured.update(kwargs)

    monkeypatch.setattr(shared.logging, "basicConfig", fake_basic_config)

    # Act
    shared.configure_logging("DEBUG")

    # Assert
    assert captured["level"] == logging.DEBUG
    assert captured["format"] == "%(message)s"
    assert captured["stream"] is sys.stderr


def test_configure_logging_unknown_level_falls_back_to_info(monkeypatch):
    # Arrange
    captured: dict[str, object] = {}

    def fake_basic_config(**kwargs):
        captured.update(kwargs)

    monkeypatch.setattr(shared.logging, "basicConfig", fake_basic_config)

    # Act
    shared.configure_logging("NOT_A_LEVEL")

    # Assert
    assert captured["level"] == logging.INFO


def test_log_event_emits_json_payload_with_fields():
    # Arrange
    logger = LoggerStub()

    # Act
    shared.log_event(logger, logging.INFO, "hello", path=Path("notes.md"), count=3)

    # Assert
    assert len(logger.calls) == 1
    level, payload = logger.calls[0]
    assert level == logging.INFO
    assert json.loads(payload) == {"count": 3, "event": "hello", "path": "notes.md"}


def test_request_with_retry_returns_response_on_first_attempt(
    monkeypatch, request_session_factory, caplog
):
    # Arrange
    session = request_session_factory([shared_test_response(status_code=200)])
    events: list[dict[str, object]] = []
    sleeps: list[float] = []

    def fake_log_event(_logger, _level, event: str, **fields):
        events.append({"event": event, **fields})

    monkeypatch.setattr(shared, "log_event", fake_log_event)
    monkeypatch.setattr(shared.time, "sleep", lambda seconds: sleeps.append(seconds))

    # Act
    response = shared.request_with_retry(
        logging.getLogger("test"),
        session,
        "get",
        "https://example.test",
        timeout=1,
        retries=2,
        retry_backoff=0.25,
    )

    # Assert
    assert response.status_code == 200
    assert session.calls[0]["method"] == "GET"
    assert sleeps == []
    assert events == []


def test_request_with_retry_retries_on_retryable_status_with_exponential_backoff(
    monkeypatch, request_session_factory
):
    # Arrange
    session = request_session_factory(
        [
            shared_test_response(status_code=503),
            shared_test_response(status_code=429),
            shared_test_response(status_code=200),
        ]
    )
    events: list[dict[str, object]] = []
    sleeps: list[float] = []

    def fake_log_event(_logger, _level, event: str, **fields):
        events.append({"event": event, **fields})

    monkeypatch.setattr(shared, "log_event", fake_log_event)
    monkeypatch.setattr(shared.time, "sleep", lambda seconds: sleeps.append(seconds))

    # Act
    response = shared.request_with_retry(
        logging.getLogger("test"),
        session,
        "GET",
        "https://example.test",
        timeout=1,
        retries=2,
        retry_backoff=0.5,
    )

    # Assert
    assert response.status_code == 200
    assert sleeps == [0.5, 1.0]
    assert [event["event"] for event in events] == ["http_retry_status", "http_retry_status"]
    assert events[0]["attempt"] == 1
    assert events[0]["max_attempts"] == 3
    assert events[0]["status_code"] == 503
    assert events[0]["wait_seconds"] == 0.5
    assert events[1]["attempt"] == 2
    assert events[1]["wait_seconds"] == 1.0
    assert len(session.calls) == 3


def test_request_with_retry_retries_on_timeout_exception_then_succeeds(
    monkeypatch, request_session_factory
):
    # Arrange
    session = request_session_factory([requests.Timeout("timeout"), shared_test_response(status_code=200)])
    events: list[dict[str, object]] = []
    sleeps: list[float] = []

    def fake_log_event(_logger, _level, event: str, **fields):
        events.append({"event": event, **fields})

    monkeypatch.setattr(shared, "log_event", fake_log_event)
    monkeypatch.setattr(shared.time, "sleep", lambda seconds: sleeps.append(seconds))

    # Act
    response = shared.request_with_retry(
        logging.getLogger("test"),
        session,
        "post",
        "https://example.test",
        timeout=1,
        retries=1,
        retry_backoff=1.0,
    )

    # Assert
    assert response.status_code == 200
    assert sleeps == [1.0]
    assert events[0]["event"] == "http_retry_exception"
    assert events[0]["error_type"] == "Timeout"
    assert events[0]["attempt"] == 1
    assert events[0]["max_attempts"] == 2
    assert len(session.calls) == 2


def test_request_with_retry_raises_after_exhausted_retries_on_exception(
    monkeypatch, request_session_factory
):
    # Arrange
    session = request_session_factory(
        [
            requests.ConnectionError("first"),
            requests.ConnectionError("second"),
        ]
    )
    events: list[dict[str, object]] = []
    sleeps: list[float] = []

    def fake_log_event(_logger, _level, event: str, **fields):
        events.append({"event": event, **fields})

    monkeypatch.setattr(shared, "log_event", fake_log_event)
    monkeypatch.setattr(shared.time, "sleep", lambda seconds: sleeps.append(seconds))

    # Act / Assert
    with pytest.raises(requests.ConnectionError):
        shared.request_with_retry(
            logging.getLogger("test"),
            session,
            "GET",
            "https://example.test",
            timeout=1,
            retries=1,
            retry_backoff=0.25,
        )

    assert sleeps == [0.25]
    assert [event["event"] for event in events] == ["http_retry_exception"]
    assert len(session.calls) == 2


def test_request_with_retry_raises_http_error_on_last_attempt_retryable_status(
    monkeypatch, request_session_factory
):
    # Arrange
    session = request_session_factory([shared_test_response(status_code=503), shared_test_response(status_code=503)])
    events: list[dict[str, object]] = []
    sleeps: list[float] = []

    def fake_log_event(_logger, _level, event: str, **fields):
        events.append({"event": event, **fields})

    monkeypatch.setattr(shared, "log_event", fake_log_event)
    monkeypatch.setattr(shared.time, "sleep", lambda seconds: sleeps.append(seconds))

    # Act / Assert
    with pytest.raises(requests.HTTPError):
        shared.request_with_retry(
            logging.getLogger("test"),
            session,
            "GET",
            "https://example.test",
            timeout=1,
            retries=1,
            retry_backoff=0.5,
        )

    assert sleeps == [0.5]
    assert [event["event"] for event in events] == ["http_retry_status"]
    assert len(session.calls) == 2


def shared_test_response(*, status_code: int):
    class _Response:
        def __init__(self, status_code: int):
            self.status_code = status_code
            self.text = ""

        def raise_for_status(self) -> None:
            if self.status_code >= 400:
                error = requests.HTTPError(f"HTTP {self.status_code}")
                error.response = self
                raise error

    return _Response(status_code=status_code)
