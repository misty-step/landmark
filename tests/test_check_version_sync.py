from __future__ import annotations

import json
from pathlib import Path
from types import SimpleNamespace


def test_normalize_tag_version_accepts_semver_tags(check_version_sync):
    assert check_version_sync.normalize_tag_version("v1.2.3") == "1.2.3"
    assert check_version_sync.normalize_tag_version("1.2.3-beta.1") == "1.2.3-beta.1"


def test_normalize_tag_version_rejects_non_semver_tags(check_version_sync):
    assert check_version_sync.normalize_tag_version("v1") is None
    assert check_version_sync.normalize_tag_version("not-a-tag") is None


def test_latest_semver_version_from_tags_skips_floating_tag(check_version_sync):
    latest = check_version_sync.latest_semver_version_from_tags(["v2", "v1", "v1.4.0", "v1.3.9"])
    assert latest == "1.4.0"


def test_detect_drift_returns_file_level_mismatches(check_version_sync):
    mismatches = check_version_sync.detect_drift(
        "1.4.0",
        {
            "package.json": "1.4.0",
            "pyproject.toml": "1.3.9",
        },
    )
    assert mismatches == ["pyproject.toml: expected 1.4.0, found 1.3.9"]


def test_main_passes_when_versions_match_latest_tag(check_version_sync, monkeypatch, tmp_path: Path):
    package_json = tmp_path / "package.json"
    package_json.write_text(json.dumps({"version": "1.2.3"}) + "\n", encoding="utf-8")
    pyproject = tmp_path / "pyproject.toml"
    pyproject.write_text("[project]\nversion = \"1.2.3\"\n", encoding="utf-8")

    monkeypatch.setattr(check_version_sync, "load_sorted_tags", lambda _repo_root, _reference: ["v1", "v1.2.3"])

    exit_code = check_version_sync.main(["--repo-root", str(tmp_path)])

    assert exit_code == 0


def test_main_fails_when_version_drift_detected(check_version_sync, monkeypatch, tmp_path: Path):
    package_json = tmp_path / "package.json"
    package_json.write_text(json.dumps({"version": "1.2.3"}) + "\n", encoding="utf-8")
    pyproject = tmp_path / "pyproject.toml"
    pyproject.write_text("[project]\nversion = \"1.2.2\"\n", encoding="utf-8")

    monkeypatch.setattr(check_version_sync, "load_sorted_tags", lambda _repo_root, _reference: ["v1.2.3"])

    exit_code = check_version_sync.main(["--repo-root", str(tmp_path)])

    assert exit_code == 1


def test_main_skips_when_no_semver_tags(check_version_sync, monkeypatch, tmp_path: Path):
    monkeypatch.setattr(check_version_sync, "load_sorted_tags", lambda _repo_root, _reference: ["v2", "latest"])

    exit_code = check_version_sync.main(["--repo-root", str(tmp_path)])

    assert exit_code == 0


def test_load_sorted_tags_scopes_to_head_merged_tags(check_version_sync, monkeypatch, tmp_path: Path):
    captured: dict[str, object] = {}

    def fake_run(cmd, *, check, capture_output, text):
        captured["cmd"] = cmd
        captured["check"] = check
        captured["capture_output"] = capture_output
        captured["text"] = text
        return SimpleNamespace(stdout="v1.2.3\nv1.2.2\n")

    monkeypatch.setattr(check_version_sync.subprocess, "run", fake_run)

    tags = check_version_sync.load_sorted_tags(tmp_path, "HEAD^2")

    assert tags == ["v1.2.3", "v1.2.2"]
    assert captured["cmd"] == [
        "git",
        "-C",
        str(tmp_path),
        "tag",
        "--merged",
        "HEAD^2",
        "--sort=-version:refname",
    ]
    assert captured["check"] is True
    assert captured["capture_output"] is True
    assert captured["text"] is True
