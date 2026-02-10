from __future__ import annotations

import argparse
import json
from datetime import date
from pathlib import Path

import pytest


def test_validate_args_accepts_valid_inputs(write_artifacts):
    # Arrange
    args = argparse.Namespace(
        notes_file="notes.md",
        version="v1.2.3",
        output_file="",
        output_json="",
        log_level="INFO",
    )

    # Act / Assert
    write_artifacts.validate_args(args)


def test_validate_args_rejects_blank_notes_file(write_artifacts):
    # Arrange
    args = argparse.Namespace(
        notes_file="  ",
        version="v1.2.3",
        output_file="",
        output_json="",
        log_level="INFO",
    )

    # Act / Assert
    with pytest.raises(ValueError, match="notes-file must be non-empty"):
        write_artifacts.validate_args(args)


def test_validate_args_rejects_blank_version(write_artifacts):
    # Arrange
    args = argparse.Namespace(
        notes_file="notes.md",
        version="  ",
        output_file="",
        output_json="",
        log_level="INFO",
    )

    # Act / Assert
    with pytest.raises(ValueError, match="version must be non-empty"):
        write_artifacts.validate_args(args)


def test_interpolate_output_path_replaces_version_placeholder(write_artifacts, tmp_path: Path):
    # Arrange
    template = str(tmp_path / "docs" / "{version}.md")

    # Act
    path = write_artifacts.interpolate_output_path(template, "v1.2.3")

    # Assert
    assert path == tmp_path / "docs" / "v1.2.3.md"


def test_write_notes_file_writes_to_interpolated_path_and_creates_parent_directory(
    write_artifacts, tmp_path: Path
):
    # Arrange
    notes = "## What's New\n- Faster."
    template = str(tmp_path / "nested" / "{version}.md")

    # Act
    written = write_artifacts.write_notes_file(notes, template, version="v1.2.3")

    # Assert
    assert written == tmp_path / "nested" / "v1.2.3.md"
    assert written.exists()
    assert written.read_text(encoding="utf-8") == notes


def test_normalize_json_version_strips_v_prefix(write_artifacts):
    # Arrange / Act
    normalized = write_artifacts.normalize_json_version("v1.2.3")

    # Assert
    assert normalized == "1.2.3"


def test_normalize_json_version_returns_input_when_no_v_prefix(write_artifacts):
    # Arrange / Act
    normalized = write_artifacts.normalize_json_version("1.2.3")

    # Assert
    assert normalized == "1.2.3"


def test_load_json_array_missing_file_returns_empty_list(write_artifacts, tmp_path: Path):
    # Arrange
    path = tmp_path / "missing.json"

    # Act
    entries = write_artifacts.load_json_array(path)

    # Assert
    assert entries == []


def test_load_json_array_valid_array_returns_list(write_artifacts, tmp_path: Path):
    # Arrange
    path = tmp_path / "releases.json"
    path.write_text(json.dumps([{"version": "1.0.0"}]), encoding="utf-8")

    # Act
    entries = write_artifacts.load_json_array(path)

    # Assert
    assert entries == [{"version": "1.0.0"}]


def test_load_json_array_invalid_root_raises_value_error(write_artifacts, tmp_path: Path):
    # Arrange
    path = tmp_path / "releases.json"
    path.write_text(json.dumps({"version": "1.0.0"}), encoding="utf-8")

    # Act / Assert
    with pytest.raises(ValueError, match="root must be a JSON array"):
        write_artifacts.load_json_array(path)


def test_append_json_entry_creates_new_file(write_artifacts, tmp_path: Path):
    # Arrange
    path = tmp_path / "releases.json"

    # Act
    written = write_artifacts.append_json_entry(
        notes="notes",
        version="v1.2.3",
        output_json_path=str(path),
        today=date(2026, 2, 10),
    )

    # Assert
    assert written == path
    payload = json.loads(path.read_text(encoding="utf-8"))
    assert payload == [{"version": "1.2.3", "date": "2026-02-10", "notes": "notes"}]
    assert path.read_text(encoding="utf-8").endswith("\n")


def test_append_json_entry_appends_to_existing_array(write_artifacts, tmp_path: Path):
    # Arrange
    path = tmp_path / "releases.json"
    path.write_text(json.dumps([{"version": "1.0.0", "date": "2026-02-01", "notes": "old"}]), encoding="utf-8")

    # Act
    write_artifacts.append_json_entry(
        notes="new notes",
        version="1.2.0",
        output_json_path=str(path),
        today=date(2026, 2, 10),
    )

    # Assert
    payload = json.loads(path.read_text(encoding="utf-8"))
    assert len(payload) == 2
    assert payload[1] == {"version": "1.2.0", "date": "2026-02-10", "notes": "new notes"}

