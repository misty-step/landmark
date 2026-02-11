#!/usr/bin/env python3
"""Synchronize repository metadata files to a release version."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


SEMVER_RE = re.compile(r"^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$")
PYPROJECT_VERSION_RE = re.compile(r"^(\s*version\s*=\s*)([\"'])([^\"']*)([\"'])(\s*(?:#.*)?)$")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Update package metadata version fields.")
    parser.add_argument("--version", required=True, help="Semver version without leading v (e.g. 1.2.3).")
    parser.add_argument("--package-json", default="package.json", help="Path to package.json.")
    parser.add_argument("--pyproject", default="pyproject.toml", help="Path to pyproject.toml.")
    return parser.parse_args(argv)


def normalize_version(version: str) -> str:
    normalized = version.strip()
    if normalized.startswith("v"):
        normalized = normalized[1:]
    if not SEMVER_RE.match(normalized):
        raise ValueError(f"invalid semver version: {version}")
    return normalized


def update_package_json(path: Path, version: str) -> bool:
    if not path.exists():
        return False

    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("package.json root must be an object")

    current_version = str(payload.get("version", "")).strip()
    if not current_version:
        raise ValueError("package.json missing non-empty version field")

    if current_version == version:
        return False

    payload["version"] = version
    path.write_text(f"{json.dumps(payload, indent=2)}\n", encoding="utf-8")
    return True


def update_pyproject(path: Path, version: str) -> bool:
    if not path.exists():
        return False

    lines = path.read_text(encoding="utf-8").splitlines()
    in_project = False
    updated = False

    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_project = stripped == "[project]"
            continue

        if not in_project:
            continue

        match = PYPROJECT_VERSION_RE.match(line)
        if not match:
            continue

        current_version = match.group(3).strip()
        if not current_version:
            raise ValueError("pyproject.toml [project].version must be non-empty")
        if current_version == version:
            return False

        lines[index] = f"{match.group(1)}{match.group(2)}{version}{match.group(4)}{match.group(5)}"
        updated = True
        break

    if not updated:
        raise ValueError("pyproject.toml missing [project].version field")

    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return True


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)

    try:
        version = normalize_version(args.version)
        package_updated = update_package_json(Path(args.package_json), version)
        pyproject_updated = update_pyproject(Path(args.pyproject), version)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(str(exc), file=sys.stderr)
        return 1

    updated_files: list[str] = []
    if package_updated:
        updated_files.append(args.package_json)
    if pyproject_updated:
        updated_files.append(args.pyproject)

    if updated_files:
        print("Updated metadata versions:", ", ".join(updated_files))
    else:
        print("Metadata already in sync.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
