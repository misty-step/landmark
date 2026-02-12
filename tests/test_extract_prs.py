from __future__ import annotations

from datetime import UTC, datetime

import pytest


def test_validate_args_rejects_invalid_repository(extract_prs, make_namespace):
    # Arrange
    args = make_namespace(
        github_token="secret",
        repository="not-a-repo",
        release_tag="v1.2.0",
        output_file="out.md",
        timeout=60,
        retries=2,
        retry_backoff=1.0,
        body_chars=500,
    )

    # Act / Assert
    with pytest.raises(ValueError, match="repository must match owner/repo"):
        extract_prs.validate_args(args)


def test_filter_prs_by_window_includes_only_merged_prs_in_range(extract_prs):
    # Arrange
    pulls = [
        {"number": 1, "merged_at": "2026-02-01T00:00:00Z", "title": "old"},
        {"number": 2, "merged_at": "2026-02-10T12:00:00Z", "title": "target"},
        {"number": 3, "merged_at": None, "title": "not merged"},
        {"number": 4, "merged_at": "2026-03-01T00:00:00Z", "title": "future"},
    ]
    start = datetime(2026, 2, 9, tzinfo=UTC)
    end = datetime(2026, 2, 20, tzinfo=UTC)

    # Act
    filtered = extract_prs.filter_prs_by_window(pulls, start, end)

    # Assert
    assert [pr["number"] for pr in filtered] == [2]


def test_render_pr_changelog_formats_title_author_labels_and_excerpt(extract_prs):
    # Arrange
    pulls = [
        {
            "number": 42,
            "title": "Add synthesis retries",
            "merged_at": "2026-02-10T12:00:00Z",
            "body": "Adds retry behavior for transient failures.\n\nIncludes tests.",
            "labels": [{"name": "type/enhancement"}, {"name": "domain/synthesis"}],
            "user": {"login": "misty"},
        }
    ]

    # Act
    markdown = extract_prs.render_pr_changelog(pulls, "v1.2.0", body_chars=500)

    # Assert
    assert markdown.startswith("## Pull Request Changelog (v1.2.0)")
    assert "### #42 Add synthesis retries" in markdown
    assert "- Author: @misty" in markdown
    assert "- Labels: type/enhancement, domain/synthesis" in markdown
    assert "Adds retry behavior for transient failures." in markdown


def test_trim_text_enforces_character_limit(extract_prs):
    # Arrange
    text = "A" * 20

    # Act
    trimmed = extract_prs.trim_text(text, 10)

    # Assert
    assert trimmed == "AAAAAAAAAA..."
