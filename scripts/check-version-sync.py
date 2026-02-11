#!/usr/bin/env python3
"""Fail when local metadata versions drift from latest semver git tag."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path


SEMVER_TAG_RE = re.compile(r"^v?(\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?)$")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Check metadata versions match latest semver git tag.")
    parser.add_argument("--repo-root", default=".", help="Repository root with git tags and metadata files.")
    parser.add_argument(
        "--reference",
        default="HEAD",
        help="Git reference used to scope merged semver tags (default: HEAD).",
    )
    parser.add_argument("--package-json", default="package.json", help="Path to package.json relative to repo-root.")
    parser.add_argument("--pyproject", default="pyproject.toml", help="Path to pyproject.toml relative to repo-root.")
    return parser.parse_args(argv)


def normalize_tag_version(tag: str) -> str | None:
    match = SEMVER_TAG_RE.match(tag.strip())
    if not match:
        return None
    return match.group(1)


def latest_semver_version_from_tags(tags: list[str]) -> str | None:
    for tag in tags:
        version = normalize_tag_version(tag)
        if version is not None:
            return version
    return None


def load_sorted_tags(repo_root: Path, reference: str) -> list[str]:
    result = subprocess.run(
        ["git", "-C", str(repo_root), "tag", "--merged", reference, "--sort=-version:refname"],
        check=True,
        capture_output=True,
        text=True,
    )
    return [line.strip() for line in result.stdout.splitlines() if line.strip()]


def load_package_version(path: Path) -> str:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("package.json root must be an object")
    version = str(payload.get("version", "")).strip()
    if not version:
        raise ValueError("package.json missing non-empty version field")
    return version


def load_pyproject_version(path: Path) -> str:
    payload = tomllib.loads(path.read_text(encoding="utf-8"))
    project = payload.get("project")
    if not isinstance(project, dict):
        raise ValueError("pyproject.toml missing [project] table")
    version = str(project.get("version", "")).strip()
    if not version:
        raise ValueError("pyproject.toml missing non-empty [project].version")
    return version


def detect_drift(expected_version: str, versions: dict[str, str]) -> list[str]:
    mismatches: list[str] = []
    for file_path, version in versions.items():
        if version != expected_version:
            mismatches.append(f"{file_path}: expected {expected_version}, found {version}")
    return mismatches


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    repo_root = Path(args.repo_root).resolve()

    try:
        tags = load_sorted_tags(repo_root, args.reference)
        expected_version = latest_semver_version_from_tags(tags)
        if expected_version is None:
            print("No semver tags found. Skipping metadata drift check.")
            return 0

        versions = {
            args.package_json: load_package_version(repo_root / args.package_json),
            args.pyproject: load_pyproject_version(repo_root / args.pyproject),
        }
        mismatches = detect_drift(expected_version, versions)
    except (OSError, subprocess.CalledProcessError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as exc:
        print(str(exc), file=sys.stderr)
        return 1

    if mismatches:
        print("Version drift detected:", file=sys.stderr)
        for mismatch in mismatches:
            print(f"- {mismatch}", file=sys.stderr)
        return 1

    print(f"Version sync ok: metadata matches latest tag version {expected_version}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
