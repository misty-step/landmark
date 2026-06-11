from __future__ import annotations

import argparse

import pytest
import requests

from conftest import FakeResponse


def test_validate_args_accepts_valid_inputs(fetch_release_body):
    args = argparse.Namespace(
        github_token="token",
        repository="octo/example",
        release_tag="v1.2.3",
        output_file="release.md",
        api_base_url="https://api.github.com",
        timeout=30,
        retries=2,
        retry_backoff=1.0,
        log_level="INFO",
    )

    fetch_release_body.validate_args(args)


def test_validate_args_rejects_invalid_repository(fetch_release_body):
    args = argparse.Namespace(
        github_token="token",
        repository="not-a-repo",
        release_tag="v1.2.3",
        output_file="release.md",
        api_base_url="https://api.github.com",
        timeout=30,
        retries=2,
        retry_backoff=1.0,
        log_level="INFO",
    )

    with pytest.raises(ValueError, match="repository must match owner/repo"):
        fetch_release_body.validate_args(args)


def test_fetch_release_body_returns_body(fetch_release_body, request_session_factory):
    session = request_session_factory(
        [FakeResponse(status_code=200, json_data={"body": "## Notes\n- shipped"})]
    )

    body = fetch_release_body.fetch_release_body(
        api_base_url="https://api.github.test",
        repository="octo/example",
        release_tag="v1.2.3",
        token="token",
        timeout=7,
        retries=2,
        retry_backoff=0.5,
        session=session,
    )

    assert body == "## Notes\n- shipped"
    assert session.calls[0]["method"] == "GET"
    assert session.calls[0]["timeout"] == 7
    assert session.calls[0]["kwargs"]["headers"]["Authorization"] == "Bearer token"


def test_fetch_release_body_returns_none_on_404(fetch_release_body, request_session_factory):
    session = request_session_factory(
        [FakeResponse(status_code=404, json_data={"message": "Not Found"}, text="Not Found")]
    )

    body = fetch_release_body.fetch_release_body(
        api_base_url="https://api.github.test",
        repository="octo/example",
        release_tag="v1",
        token="token",
        timeout=7,
        retries=0,
        retry_backoff=0.5,
        session=session,
    )

    assert body is None


def test_fetch_release_body_raises_on_non_404(fetch_release_body, request_session_factory):
    session = request_session_factory(
        [FakeResponse(status_code=500, json_data={"message": "oops"}, text="oops")]
    )

    with pytest.raises(requests.HTTPError):
        fetch_release_body.fetch_release_body(
            api_base_url="https://api.github.test",
            repository="octo/example",
            release_tag="v1.2.3",
            token="token",
            timeout=7,
            retries=0,
            retry_backoff=0.5,
            session=session,
        )
