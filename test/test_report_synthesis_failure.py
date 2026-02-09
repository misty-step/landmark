from __future__ import annotations

import sys

import requests


class FakeResponse:
    def __init__(self, status_code: int, text: str = ""):
        self.status_code = status_code
        self.text = text


def build_http_error(status_code: int, text: str) -> requests.HTTPError:
    response = FakeResponse(status_code=status_code, text=text)
    error = requests.HTTPError(f"HTTP {status_code}")
    error.response = response
    return error


def test_compose_issue_body_includes_failure_context(report_synthesis_failure):
    body = report_synthesis_failure.compose_issue_body(
        repository="octo/example",
        release_tag="v1.2.3",
        failure_stage="synthesis",
        failure_message="request timed out",
        workflow_name="Release",
        workflow_run_url="https://github.com/octo/example/actions/runs/123",
    )

    assert "octo/example" in body
    assert "v1.2.3" in body
    assert "Synthesis request" in body
    assert "request timed out" in body
    assert "actions/runs/123" in body


def test_main_returns_error_for_invalid_repository(report_synthesis_failure, monkeypatch):
    monkeypatch.setattr(
        sys,
        "argv",
        [
            "report-synthesis-failure.py",
            "--github-token",
            "token",
            "--repository",
            "invalid",
            "--release-tag",
            "v1.0.0",
            "--failure-stage",
            "synthesis",
            "--failure-message",
            "details",
            "--workflow-run-url",
            "https://github.com/octo/example/actions/runs/1",
            "--workflow-name",
            "Release",
        ],
    )

    assert report_synthesis_failure.main() == 1


def test_main_successfully_creates_issue(report_synthesis_failure, monkeypatch):
    captured: dict[str, str] = {}

    def fake_create_issue(**kwargs):
        captured["title"] = kwargs["title"]
        captured["body"] = kwargs["body"]
        return {"html_url": "https://github.com/octo/example/issues/99"}

    monkeypatch.setattr(report_synthesis_failure, "create_issue", fake_create_issue)
    monkeypatch.setattr(
        sys,
        "argv",
        [
            "report-synthesis-failure.py",
            "--github-token",
            "token",
            "--repository",
            "octo/example",
            "--release-tag",
            "v1.0.0",
            "--failure-stage",
            "release_update",
            "--failure-message",
            "could not patch release body",
            "--workflow-run-url",
            "https://github.com/octo/example/actions/runs/1",
            "--workflow-name",
            "Release",
        ],
    )

    assert report_synthesis_failure.main() == 0
    assert captured["title"] == "[Landfall] Synthesis failed for v1.0.0"
    assert "could not patch release body" in captured["body"]


def test_main_returns_error_on_http_error(report_synthesis_failure, monkeypatch):
    def fake_create_issue(**_kwargs):
        raise build_http_error(403, "forbidden")

    monkeypatch.setattr(report_synthesis_failure, "create_issue", fake_create_issue)
    monkeypatch.setattr(
        sys,
        "argv",
        [
            "report-synthesis-failure.py",
            "--github-token",
            "token",
            "--repository",
            "octo/example",
            "--release-tag",
            "v1.0.0",
            "--failure-stage",
            "synthesis",
            "--failure-message",
            "provider failed",
            "--workflow-run-url",
            "https://github.com/octo/example/actions/runs/1",
            "--workflow-name",
            "Release",
        ],
    )

    assert report_synthesis_failure.main() == 1
