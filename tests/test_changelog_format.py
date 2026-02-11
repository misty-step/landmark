from __future__ import annotations

import re
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
CHANGELOG_PATH = REPO_ROOT / "CHANGELOG.md"


def test_changelog_does_not_include_manual_keep_a_changelog_sections() -> None:
    changelog = CHANGELOG_PATH.read_text(encoding="utf-8")

    assert "# Changelog" not in changelog
    assert "## [Unreleased]" not in changelog


def test_changelog_begins_with_semantic_release_version_heading() -> None:
    changelog = CHANGELOG_PATH.read_text(encoding="utf-8")
    first_non_empty_line = next(line for line in changelog.splitlines() if line.strip())

    assert re.match(r"^##? \[\d+\.\d+\.\d+\]", first_non_empty_line)
