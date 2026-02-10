from __future__ import annotations

import argparse

import pytest
import requests


def test_validate_args_accepts_valid_inputs(report_synthesis_failure):
    # Arrange
    args = argparse.Namespace(
        github_token="token",
        repository="octo/example",
        release_tag="v1.2.3",
        failure_stage="synthesis",
        failure_message="timed out",
        workflow_run_url="https://github.com/octo/example/actions/runs/1",
        workflow_name="Release",
        api_base_url="https://api.github.com",
        timeout=5,
        log_level="INFO",
    )

    # Act / Assert
    report_synthesis_failure.validate_args(args)


def test_validate_args_rejects_invalid_repository(report_synthesis_failure):
    # Arrange
    args = argparse.Namespace(
        github_token="token",
        repository="invalid",
        release_tag="v1.2.3",
        failure_stage="synthesis",
        failure_message="timed out",
        workflow_run_url="https://github.com/octo/example/actions/runs/1",
        workflow_name="Release",
        api_base_url="https://api.github.com",
        timeout=5,
        log_level="INFO",
    )

    # Act / Assert
    with pytest.raises(ValueError, match="repository must match owner/repo"):
        report_synthesis_failure.validate_args(args)


def test_validate_args_rejects_invalid_workflow_run_url_scheme(report_synthesis_failure):
    # Arrange
    args = argparse.Namespace(
        github_token="token",
        repository="octo/example",
        release_tag="v1.2.3",
        failure_stage="synthesis",
        failure_message="timed out",
        workflow_run_url="file:///tmp/nope",
        workflow_name="Release",
        api_base_url="https://api.github.com",
        timeout=5,
        log_level="INFO",
    )

    # Act / Assert
    with pytest.raises(ValueError, match="workflow-run-url must start with http:// or https://"):
        report_synthesis_failure.validate_args(args)


def test_describe_failure_stage_returns_known_label(report_synthesis_failure):
    # Arrange / Act
    label = report_synthesis_failure.describe_failure_stage("synthesis_empty")

    # Assert
    assert label == "Synthesis output validation"


def test_describe_failure_stage_returns_unknown_label_for_unknown_key(report_synthesis_failure):
    # Arrange / Act
    label = report_synthesis_failure.describe_failure_stage("totally-new-stage")

    # Assert
    assert label == "Synthesis pipeline"


def test_describe_failure_stage_returns_unknown_stage_for_literal_unknown(report_synthesis_failure):
    # Arrange / Act
    label = report_synthesis_failure.describe_failure_stage("unknown")

    # Assert
    assert label == "Unknown stage"


def test_compose_issue_title_includes_release_tag(report_synthesis_failure):
    # Arrange / Act
    title = report_synthesis_failure.compose_issue_title("v1.2.3")

    # Assert
    assert title == "[Landfall] Synthesis failed for v1.2.3"


def test_compose_issue_body_contains_all_fields(report_synthesis_failure):
    # Arrange
    body = report_synthesis_failure.compose_issue_body(
        repository="octo/example",
        release_tag="v1.2.3",
        failure_stage="release_update",
        failure_message="could not patch release body",
        workflow_name="Release",
        workflow_run_url="https://github.com/octo/example/actions/runs/123",
    )

    # Assert
    assert "`octo/example`" in body
    assert "`v1.2.3`" in body
    assert "Release body update" in body
    assert "`Release`" in body
    assert "actions/runs/123" in body
    assert "could not patch release body" in body


def test_create_issue_posts_expected_payload(report_synthesis_failure, post_session_factory):
    # Arrange
    session = post_session_factory(
        outcome=create_issue_test_response(status_code=201, json_data={"html_url": "https://x/y"})
    )
    headers = {"Authorization": "Bearer token"}

    # Act
    result = report_synthesis_failure.create_issue(
        api_base_url="https://api.github.test",
        repository="octo/example",
        headers=headers,
        title="title",
        body="body",
        timeout=5,
        session=session,
    )

    # Assert
    assert result["html_url"] == "https://x/y"
    assert session.calls[0]["url"].endswith("/repos/octo/example/issues")
    assert session.calls[0]["headers"] == headers
    assert session.calls[0]["json"] == {"title": "title", "body": "body"}
    assert session.calls[0]["timeout"] == 5


def create_issue_test_response(*, status_code: int, json_data: object):
    class _Response:
        def __init__(self, status_code: int, json_data: object):
            self.status_code = status_code
            self._json_data = json_data
            self.text = ""

        def json(self) -> object:
            return self._json_data

        def raise_for_status(self) -> None:
            if self.status_code >= 400:
                error = requests.HTTPError(f"HTTP {self.status_code}")
                error.response = self
                raise error

    return _Response(status_code=status_code, json_data=json_data)

