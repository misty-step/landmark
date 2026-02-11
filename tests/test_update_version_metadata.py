from __future__ import annotations

import json
from pathlib import Path

import pytest


def test_normalize_version_strips_v_prefix(update_version_metadata):
    normalized = update_version_metadata.normalize_version(" v1.2.3 ")
    assert normalized == "1.2.3"


def test_normalize_version_rejects_invalid_semver(update_version_metadata):
    with pytest.raises(ValueError, match="invalid semver version"):
        update_version_metadata.normalize_version("v1.2")


def test_update_package_json_updates_version(update_version_metadata, tmp_path: Path):
    package_json = tmp_path / "package.json"
    package_json.write_text(json.dumps({"name": "demo", "version": "1.0.0"}) + "\n", encoding="utf-8")

    changed = update_version_metadata.update_package_json(package_json, "1.2.3")

    assert changed is True
    payload = json.loads(package_json.read_text(encoding="utf-8"))
    assert payload["version"] == "1.2.3"


def test_update_pyproject_updates_project_version(update_version_metadata, tmp_path: Path):
    pyproject = tmp_path / "pyproject.toml"
    pyproject.write_text(
        "[project]\nname = \"demo\"\nversion = \"1.0.0\"\n\n[tool.ruff]\nline-length = 120\n",
        encoding="utf-8",
    )

    changed = update_version_metadata.update_pyproject(pyproject, "1.2.3")

    assert changed is True
    text = pyproject.read_text(encoding="utf-8")
    assert 'version = "1.2.3"' in text
    assert "[tool.ruff]" in text


def test_update_pyproject_raises_when_project_version_missing(update_version_metadata, tmp_path: Path):
    pyproject = tmp_path / "pyproject.toml"
    pyproject.write_text("[project]\nname = \"demo\"\n", encoding="utf-8")

    with pytest.raises(ValueError, match="missing \\[project\\]\\.version"):
        update_version_metadata.update_pyproject(pyproject, "1.2.3")


def test_main_updates_metadata_files(update_version_metadata, tmp_path: Path):
    package_json = tmp_path / "package.json"
    package_json.write_text(json.dumps({"name": "demo", "version": "1.0.0"}) + "\n", encoding="utf-8")
    pyproject = tmp_path / "pyproject.toml"
    pyproject.write_text("[project]\nname = \"demo\"\nversion = \"1.0.0\"\n", encoding="utf-8")

    exit_code = update_version_metadata.main(
        [
            "--version",
            "1.4.0",
            "--package-json",
            str(package_json),
            "--pyproject",
            str(pyproject),
        ]
    )

    assert exit_code == 0
    package_payload = json.loads(package_json.read_text(encoding="utf-8"))
    assert package_payload["version"] == "1.4.0"
    assert 'version = "1.4.0"' in pyproject.read_text(encoding="utf-8")


def test_main_returns_error_on_invalid_version(update_version_metadata, tmp_path: Path):
    package_json = tmp_path / "package.json"
    package_json.write_text(json.dumps({"name": "demo", "version": "1.0.0"}) + "\n", encoding="utf-8")
    pyproject = tmp_path / "pyproject.toml"
    pyproject.write_text("[project]\nname = \"demo\"\nversion = \"1.0.0\"\n", encoding="utf-8")

    exit_code = update_version_metadata.main(
        [
            "--version",
            "bad-version",
            "--package-json",
            str(package_json),
            "--pyproject",
            str(pyproject),
        ]
    )

    assert exit_code == 1
