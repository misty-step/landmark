from __future__ import annotations

import argparse

import pytest


def test_strip_existing_whats_new_removes_existing_section(update_release):
    # Arrange
    body = (
        "## What's New\n\nOld notes.\n\n"
        "## Technical Changes\n- internal\n"
    )

    # Act
    cleaned = update_release.strip_existing_whats_new(body)

    # Assert
    assert "Old notes." not in cleaned
    assert cleaned.startswith("## Technical Changes")


def test_strip_existing_whats_new_returns_body_when_missing_section(update_release):
    # Arrange
    body = "\n\n## Technical Changes\n- internal\n\n"

    # Act
    cleaned = update_release.strip_existing_whats_new(body)

    # Assert
    assert cleaned == "## Technical Changes\n- internal"


def test_strip_existing_whats_new_removes_only_first_occurrence(update_release):
    # Arrange
    body = (
        "## What's New\n\nFirst.\n\n"
        "## Technical Changes\n- internal\n\n"
        "## What's New\n\nSecond.\n"
    )

    # Act
    cleaned = update_release.strip_existing_whats_new(body)

    # Assert
    assert "First." not in cleaned
    assert "Second." in cleaned
    assert cleaned.startswith("## Technical Changes")


def test_compose_release_body_prepends_notes_and_strips_old_whats_new(update_release):
    # Arrange
    synth_notes = "## Improvements\n- Faster startup."
    existing = "## What's New\n\nOld copy.\n\n## Technical Changes\n- internal item"

    # Act
    body = update_release.compose_release_body(synth_notes, existing)

    # Assert
    assert body.startswith("## What's New")
    assert "Old copy." not in body
    assert "## Technical Changes" in body
    assert body.endswith("\n")


def test_compose_release_body_handles_empty_existing_body(update_release):
    # Arrange
    synth_notes = "- Faster startup."

    # Act
    body = update_release.compose_release_body(synth_notes, existing_body="")

    # Assert
    assert body == "## What's New\n\n- Faster startup.\n"


def test_validate_args_accepts_valid_inputs(update_release):
    # Arrange
    args = argparse.Namespace(
        github_token="token",
        repository="octo/example",
        tag="v1.2.3",
        notes_file="notes.md",
        api_base_url="https://api.github.com",
        timeout=5,
        retries=0,
        retry_backoff=0.0,
        log_level="INFO",
    )

    # Act / Assert
    update_release.validate_args(args)


def test_validate_args_rejects_blank_github_token(update_release):
    # Arrange
    args = argparse.Namespace(
        github_token="   ",
        repository="octo/example",
        tag="v1.2.3",
        notes_file="notes.md",
        api_base_url="https://api.github.com",
        timeout=5,
        retries=0,
        retry_backoff=0.0,
        log_level="INFO",
    )

    # Act / Assert
    with pytest.raises(ValueError, match="github-token must be non-empty"):
        update_release.validate_args(args)


def test_validate_args_rejects_invalid_repository_format(update_release):
    # Arrange
    args = argparse.Namespace(
        github_token="token",
        repository="not-a-repo",
        tag="v1.2.3",
        notes_file="notes.md",
        api_base_url="https://api.github.com",
        timeout=5,
        retries=0,
        retry_backoff=0.0,
        log_level="INFO",
    )

    # Act / Assert
    with pytest.raises(ValueError, match="repository must match owner/repo"):
        update_release.validate_args(args)


def test_validate_args_rejects_blank_tag(update_release):
    # Arrange
    args = argparse.Namespace(
        github_token="token",
        repository="octo/example",
        tag=" ",
        notes_file="notes.md",
        api_base_url="https://api.github.com",
        timeout=5,
        retries=0,
        retry_backoff=0.0,
        log_level="INFO",
    )

    # Act / Assert
    with pytest.raises(ValueError, match="tag must be non-empty"):
        update_release.validate_args(args)


def test_validate_args_rejects_non_positive_timeout(update_release):
    # Arrange
    args = argparse.Namespace(
        github_token="token",
        repository="octo/example",
        tag="v1.2.3",
        notes_file="notes.md",
        api_base_url="https://api.github.com",
        timeout=0,
        retries=0,
        retry_backoff=0.0,
        log_level="INFO",
    )

    # Act / Assert
    with pytest.raises(ValueError, match="timeout must be greater than zero"):
        update_release.validate_args(args)


def test_validate_args_rejects_negative_retries(update_release):
    # Arrange
    args = argparse.Namespace(
        github_token="token",
        repository="octo/example",
        tag="v1.2.3",
        notes_file="notes.md",
        api_base_url="https://api.github.com",
        timeout=5,
        retries=-1,
        retry_backoff=0.0,
        log_level="INFO",
    )

    # Act / Assert
    with pytest.raises(ValueError, match="retries cannot be negative"):
        update_release.validate_args(args)


def test_validate_args_rejects_negative_retry_backoff(update_release):
    # Arrange
    args = argparse.Namespace(
        github_token="token",
        repository="octo/example",
        tag="v1.2.3",
        notes_file="notes.md",
        api_base_url="https://api.github.com",
        timeout=5,
        retries=0,
        retry_backoff=-1.0,
        log_level="INFO",
    )

    # Act / Assert
    with pytest.raises(ValueError, match="retry-backoff cannot be negative"):
        update_release.validate_args(args)


def test_validate_args_rejects_invalid_api_base_url_scheme(update_release):
    # Arrange
    args = argparse.Namespace(
        github_token="token",
        repository="octo/example",
        tag="v1.2.3",
        notes_file="notes.md",
        api_base_url="ftp://api.github.com",
        timeout=5,
        retries=0,
        retry_backoff=0.0,
        log_level="INFO",
    )

    # Act / Assert
    with pytest.raises(ValueError, match="api-base-url must start with http:// or https://"):
        update_release.validate_args(args)

