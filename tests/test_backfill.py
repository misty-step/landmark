from __future__ import annotations

import argparse
from pathlib import Path
from unittest.mock import patch

import pytest

import backfill


def write_template(tmp_path: Path) -> Path:
    template_path = tmp_path / "prompt.md"
    template_path.write_text(
        "Name={{PRODUCT_NAME}} Version={{VERSION}}\n\n{{TECHNICAL_CHANGELOG}}\n",
        encoding="utf-8",
    )
    return template_path


def make_args(tmp_path: Path, **overrides) -> argparse.Namespace:
    template = write_template(tmp_path)
    defaults = {
        "repo": "octo/example",
        "github_token": "gh_token",
        "llm_api_key": "llm_key",
        "prompt_template": str(template),
        "model": "primary-model",
        "fallback_models": "",
        "api_url": "https://api.example.test/chat/completions",
        "product_name": None,
        "dry_run": True,
        "rate_limit": 0.0,
        "timeout": 5,
        "retries": 0,
        "retry_backoff": 0.0,
        "log_level": "INFO",
    }
    defaults.update(overrides)
    return argparse.Namespace(**defaults)


def test_fetch_all_releases_pagination(monkeypatch):
    # Arrange
    calls: list[str] = []
    page_payloads = [
        [{"id": 1}, {"id": 2}],
        [{"id": 3}],
        [],
    ]

    class ResponseStub:
        def __init__(self, payload):
            self._payload = payload

        def json(self):
            return self._payload

    def fake_request_with_retry(_logger, _session, _method, url: str, **_kwargs):
        calls.append(url)
        return ResponseStub(page_payloads[len(calls) - 1])

    monkeypatch.setattr(backfill, "request_with_retry", fake_request_with_retry)

    # Act
    releases = backfill.fetch_all_releases(
        api_base_url="https://api.github.com",
        repository="octo/example",
        headers={},
        timeout=5,
        retries=0,
        retry_backoff=0.0,
        session=object(),  # unused by our stub
    )

    # Assert
    assert releases == [{"id": 1}, {"id": 2}, {"id": 3}]
    assert calls[0].endswith("per_page=100&page=1")
    assert calls[1].endswith("per_page=100&page=2")
    assert calls[2].endswith("per_page=100&page=3")


def test_fetch_all_releases_raises_on_invalid_json(monkeypatch):
    class BadResponse:
        def json(self):
            raise ValueError("No JSON")

    def fake_request_with_retry(_logger, _session, _method, url: str, **_kwargs):
        return BadResponse()

    monkeypatch.setattr(backfill, "request_with_retry", fake_request_with_retry)

    with pytest.raises(RuntimeError, match="not valid JSON"):
        backfill.fetch_all_releases(
            api_base_url="https://api.github.com",
            repository="octo/example",
            headers={},
            timeout=5,
            retries=0,
            retry_backoff=0.0,
            session=object(),
        )


def test_continue_on_json_decode_error(tmp_path: Path):
    args = make_args(tmp_path, dry_run=False, rate_limit=0.0)
    releases = [
        {
            "id": 1,
            "tag_name": "v0.9.0",
            "body": "## Technical Changes\n- older\n",
            "published_at": "2020-01-01T00:00:00Z",
        },
        {
            "id": 2,
            "tag_name": "v1.0.0",
            "body": "## Technical Changes\n- newer\n",
            "published_at": "2020-02-01T00:00:00Z",
        },
    ]

    def fake_synthesize_notes(*, prompt: str, **_kwargs):
        if "v0.9.0" in prompt:
            raise ValueError("No JSON object could be decoded")
        return "## Improvements\n- ok\n"

    with (
        patch.object(backfill, "parse_args", return_value=args),
        patch.object(backfill, "fetch_all_releases", return_value=releases),
        patch.object(backfill, "synthesize_notes", side_effect=fake_synthesize_notes),
        patch.object(backfill, "update_release_body") as update_mock,
    ):
        exit_code = backfill.main()

    assert exit_code == 1
    assert update_mock.call_count == 1
    assert update_mock.call_args.kwargs["release_id"] == 2


def test_skip_releases_with_whats_new():
    # Arrange
    releases = [{"id": 1, "body": "## What's New\n\nSome notes\n\n## Technical Changes\n- internal\n"}]

    # Act
    pending, skipped_filled, skipped_empty = backfill.filter_releases(releases)

    # Assert
    assert pending == []
    assert skipped_filled == 1
    assert skipped_empty == 0


def test_skip_releases_with_empty_body():
    # Arrange
    releases = [{"id": 1, "body": None}, {"id": 2, "body": ""}, {"id": 3, "body": "   \n"}]

    # Act
    pending, skipped_filled, skipped_empty = backfill.filter_releases(releases)

    # Assert
    assert pending == []
    assert skipped_filled == 0
    assert skipped_empty == 3


def test_dry_run_does_not_update(tmp_path: Path):
    # Arrange
    args = make_args(tmp_path, dry_run=True)
    releases = [
        {
            "id": 123,
            "tag_name": "v1.0.0",
            "body": "## Technical Changes\n- internal\n",
            "published_at": "2020-01-01T00:00:00Z",
        }
    ]

    def fake_synthesize_notes(**_kwargs):
        return "## Improvements\n- Faster\n"

    with (
        patch.object(backfill, "parse_args", return_value=args),
        patch.object(backfill, "fetch_all_releases", return_value=releases),
        patch.object(backfill, "synthesize_notes", side_effect=fake_synthesize_notes) as synth_mock,
        patch.object(backfill, "update_release_body") as update_mock,
    ):
        exit_code = backfill.main()

    # Assert
    assert exit_code == 0
    assert synth_mock.call_count == 1
    update_mock.assert_not_called()


def test_continue_on_synthesis_failure(tmp_path: Path):
    # Arrange
    args = make_args(tmp_path, dry_run=False, rate_limit=0.0)
    releases = [
        {
            "id": 1,
            "tag_name": "v0.9.0",
            "body": "## Technical Changes\n- older\n",
            "published_at": "2020-01-01T00:00:00Z",
        },
        {
            "id": 2,
            "tag_name": "v1.0.0",
            "body": "## Technical Changes\n- newer\n",
            "published_at": "2020-02-01T00:00:00Z",
        },
    ]

    def fake_synthesize_notes(*, prompt: str, **_kwargs):
        if "v0.9.0" in prompt:
            raise RuntimeError("boom")
        return "## Improvements\n- ok\n"

    with (
        patch.object(backfill, "parse_args", return_value=args),
        patch.object(backfill, "fetch_all_releases", return_value=releases),
        patch.object(backfill, "synthesize_notes", side_effect=fake_synthesize_notes) as synth_mock,
        patch.object(backfill, "update_release_body") as update_mock,
    ):
        exit_code = backfill.main()

    # Assert
    assert exit_code == 1
    assert synth_mock.call_count == 2
    assert update_mock.call_count == 1
    assert update_mock.call_args.kwargs["release_id"] == 2


def test_rate_limiting(tmp_path: Path):
    # Arrange
    args = make_args(tmp_path, rate_limit=2.5)
    releases = [
        {
            "id": 1,
            "tag_name": "v0.9.0",
            "body": "## Technical Changes\n- older\n",
            "published_at": "2020-01-01T00:00:00Z",
        },
        {
            "id": 2,
            "tag_name": "v1.0.0",
            "body": "## Technical Changes\n- newer\n",
            "published_at": "2020-02-01T00:00:00Z",
        },
    ]

    def fake_synthesize_notes(**_kwargs):
        return "## Improvements\n- ok\n"

    with (
        patch.object(backfill, "parse_args", return_value=args),
        patch.object(backfill, "fetch_all_releases", return_value=releases),
        patch.object(backfill, "synthesize_notes", side_effect=fake_synthesize_notes),
        patch.object(backfill.time, "sleep") as sleep_mock,
    ):
        exit_code = backfill.main()

    # Assert
    assert exit_code == 0
    assert sleep_mock.call_count == 1
    assert sleep_mock.call_args.args == (2.5,)


def test_summary_counts(tmp_path: Path, capsys):
    # Arrange
    args = make_args(tmp_path, dry_run=True, rate_limit=0.0)
    releases = [
        {
            "id": 1,
            "tag_name": "v0.1.0",
            "body": "## What's New\n\nAlready filled\n",
            "published_at": "2020-01-01T00:00:00Z",
        },
        {
            "id": 2,
            "tag_name": "v0.2.0",
            "body": None,
            "published_at": "2020-02-01T00:00:00Z",
        },
        {
            "id": 3,
            "tag_name": "v0.3.0",
            "body": "## Technical Changes\n- ok\n",
            "published_at": "2020-03-01T00:00:00Z",
        },
        {
            "id": 4,
            "tag_name": "v0.4.0",
            "body": "## Technical Changes\n- boom\n",
            "published_at": "2020-04-01T00:00:00Z",
        },
    ]

    def fake_synthesize_notes(*, prompt: str, **_kwargs):
        if "v0.4.0" in prompt:
            raise RuntimeError("synthesis failed")
        return "## Improvements\n- ok\n"

    with (
        patch.object(backfill, "parse_args", return_value=args),
        patch.object(backfill, "fetch_all_releases", return_value=releases),
        patch.object(backfill, "synthesize_notes", side_effect=fake_synthesize_notes),
    ):
        exit_code = backfill.main()

    # Assert
    captured = capsys.readouterr().out
    assert exit_code == 1
    assert "total=4" in captured
    assert "processed=1" in captured
    assert "skipped_filled=1" in captured
    assert "skipped_empty=1" in captured
    assert "failed=1" in captured


def test_validate_args_repo_format():
    # Arrange
    args = argparse.Namespace(
        repo="not-a-repo",
        github_token="gh",
        llm_api_key="llm",
        prompt_template="prompt.md",
        model="model",
        fallback_models="",
        api_url="https://api.example.test/chat/completions",
        product_name=None,
        dry_run=True,
        rate_limit=0.0,
        timeout=5,
        retries=0,
        retry_backoff=0.0,
        log_level="INFO",
    )

    # Act / Assert
    with pytest.raises(ValueError, match="repo must match owner/repo"):
        backfill.validate_args(args)

