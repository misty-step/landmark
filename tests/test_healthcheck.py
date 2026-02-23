from __future__ import annotations

import pytest
import requests

import healthcheck


def _make_response(status_code: int, json_data: object = None):
    class _Resp:
        def __init__(self):
            self.status_code = status_code
            self._json = json_data or {}
            self.text = ""

        def json(self):
            return self._json

        def raise_for_status(self):
            if self.status_code >= 400:
                err = requests.HTTPError(f"HTTP {self.status_code}")
                err.response = self
                raise err

    return _Resp()


def test_probe_api_succeeds_with_valid_response(monkeypatch):
    def fake_retry(_logger, _session, _method, _url, **_kw):
        return _make_response(200, {"choices": [{"message": {"content": "OK"}}]})

    monkeypatch.setattr(healthcheck, "request_with_retry", fake_retry)
    healthcheck.probe_api("https://api.test/v1/chat/completions", "key", "model", 10)


def test_probe_api_raises_on_401(monkeypatch):
    def fake_retry(_logger, _session, _method, _url, **_kw):
        resp = _make_response(401)
        resp.raise_for_status()

    monkeypatch.setattr(healthcheck, "request_with_retry", fake_retry)
    with pytest.raises(requests.HTTPError):
        healthcheck.probe_api("https://api.test/v1/chat/completions", "bad", "model", 10)


def test_probe_api_raises_on_empty_content(monkeypatch):
    def fake_retry(_logger, _session, _method, _url, **_kw):
        return _make_response(200, {"choices": [{"message": {"content": "   "}}]})

    monkeypatch.setattr(healthcheck, "request_with_retry", fake_retry)
    with pytest.raises(RuntimeError, match="empty response"):
        healthcheck.probe_api("https://api.test/v1/chat/completions", "key", "model", 10)


def test_main_returns_0_on_success(monkeypatch):
    monkeypatch.setattr(
        "sys.argv",
        ["healthcheck.py", "--api-key", "valid-key", "--model", "test"],
    )

    def fake_retry(_logger, _session, _method, _url, **_kw):
        return _make_response(200, {"choices": [{"message": {"content": "OK"}}]})

    monkeypatch.setattr(healthcheck, "request_with_retry", fake_retry)
    assert healthcheck.main() == 0


def test_main_returns_1_on_401(monkeypatch):
    monkeypatch.setattr(
        "sys.argv",
        ["healthcheck.py", "--api-key", "bad-key", "--model", "test"],
    )

    events: list[dict] = []

    def capture_log(_logger, level, event, **fields):
        events.append({"level": level, "event": event, **fields})

    monkeypatch.setattr(healthcheck, "log_event", capture_log)

    def fake_retry(_logger, _session, _method, _url, **_kw):
        resp = _make_response(401)
        resp.raise_for_status()

    monkeypatch.setattr(healthcheck, "request_with_retry", fake_retry)
    assert healthcheck.main() == 1

    failed = [e for e in events if e["event"] == "healthcheck_failed"]
    assert len(failed) == 1
    assert failed[0]["status_code"] == 401


def test_main_401_openrouter_url_mentions_sk_or_format(monkeypatch, capsys):
    monkeypatch.setattr(
        "sys.argv",
        [
            "healthcheck.py",
            "--api-key",
            "bad-key",
            "--model",
            "test",
            "--api-url",
            "https://openrouter.ai/api/v1/chat/completions",
        ],
    )
    monkeypatch.setattr(healthcheck, "log_event", lambda *a, **kw: None)

    def fake_retry(_logger, _session, _method, _url, **_kw):
        resp = _make_response(401)
        resp.raise_for_status()

    monkeypatch.setattr(healthcheck, "request_with_retry", fake_retry)
    assert healthcheck.main() == 1
    captured = capsys.readouterr()
    assert "::error::" in captured.err
    assert "sk-or-" in captured.err
    assert "openrouter.ai/keys" in captured.err
    assert "Synthesis skipped" not in captured.err  # misleading on fatal path


def test_main_401_non_openrouter_url_generic_message(monkeypatch, capsys):
    monkeypatch.setattr(
        "sys.argv",
        [
            "healthcheck.py",
            "--api-key",
            "bad-key",
            "--model",
            "test",
            "--api-url",
            "https://api.openai.com/v1/chat/completions",
        ],
    )
    monkeypatch.setattr(healthcheck, "log_event", lambda *a, **kw: None)

    def fake_retry(_logger, _session, _method, _url, **_kw):
        resp = _make_response(401)
        resp.raise_for_status()

    monkeypatch.setattr(healthcheck, "request_with_retry", fake_retry)
    assert healthcheck.main() == 1
    captured = capsys.readouterr()
    assert "sk-or-" not in captured.err
    assert "api.openai.com" in captured.err


def test_main_warn_only_emits_warning_and_exits_0_on_auth_failure(monkeypatch, capsys):
    monkeypatch.setattr(
        "sys.argv",
        ["healthcheck.py", "--api-key", "bad-key", "--model", "test", "--warn-only"],
    )
    monkeypatch.setattr(healthcheck, "log_event", lambda *a, **kw: None)

    def fake_retry(_logger, _session, _method, _url, **_kw):
        resp = _make_response(401)
        resp.raise_for_status()

    monkeypatch.setattr(healthcheck, "request_with_retry", fake_retry)
    assert healthcheck.main() == 0
    captured = capsys.readouterr()
    assert "::warning::" in captured.err
    assert "::error::" not in captured.err


def test_main_warn_only_still_passes_on_success(monkeypatch):
    monkeypatch.setattr(
        "sys.argv",
        ["healthcheck.py", "--api-key", "valid-key", "--model", "test", "--warn-only"],
    )

    def fake_retry(_logger, _session, _method, _url, **_kw):
        return _make_response(200, {"choices": [{"message": {"content": "OK"}}]})

    monkeypatch.setattr(healthcheck, "request_with_retry", fake_retry)
    assert healthcheck.main() == 0


def test_main_returns_1_on_403(monkeypatch, capsys):
    monkeypatch.setattr(
        "sys.argv",
        ["healthcheck.py", "--api-key", "restricted-key", "--model", "test"],
    )
    monkeypatch.setattr(healthcheck, "log_event", lambda *a, **kw: None)

    def fake_retry(_logger, _session, _method, _url, **_kw):
        resp = _make_response(403)
        resp.raise_for_status()

    monkeypatch.setattr(healthcheck, "request_with_retry", fake_retry)
    assert healthcheck.main() == 1
    captured = capsys.readouterr()
    assert "403" in captured.err
    assert "billing" in captured.err


def test_main_warn_only_exits_0_on_403(monkeypatch, capsys):
    monkeypatch.setattr(
        "sys.argv",
        ["healthcheck.py", "--api-key", "restricted-key", "--model", "test", "--warn-only"],
    )
    monkeypatch.setattr(healthcheck, "log_event", lambda *a, **kw: None)

    def fake_retry(_logger, _session, _method, _url, **_kw):
        resp = _make_response(403)
        resp.raise_for_status()

    monkeypatch.setattr(healthcheck, "request_with_retry", fake_retry)
    assert healthcheck.main() == 0
    captured = capsys.readouterr()
    assert "::warning::" in captured.err


def test_main_returns_1_on_network_error(monkeypatch, capsys):
    monkeypatch.setattr(
        "sys.argv",
        ["healthcheck.py", "--api-key", "some-key", "--model", "test"],
    )
    monkeypatch.setattr(healthcheck, "log_event", lambda *a, **kw: None)

    def fake_retry(_logger, _session, _method, _url, **_kw):
        raise requests.ConnectionError("connection refused")

    monkeypatch.setattr(healthcheck, "request_with_retry", fake_retry)
    assert healthcheck.main() == 1
    captured = capsys.readouterr()
    assert "::error::" in captured.err


def test_main_returns_1_on_empty_key(monkeypatch):
    monkeypatch.setattr(
        "sys.argv",
        ["healthcheck.py", "--api-key", "", "--model", "test"],
    )

    events: list[dict] = []

    def capture_log(_logger, level, event, **fields):
        events.append({"level": level, "event": event, **fields})

    monkeypatch.setattr(healthcheck, "log_event", capture_log)
    assert healthcheck.main() == 1

    skipped = [e for e in events if e["event"] == "healthcheck_skipped"]
    assert len(skipped) == 1
