from __future__ import annotations

import importlib.util
from pathlib import Path
from types import ModuleType

import pytest


REPO_ROOT = Path(__file__).resolve().parents[1]


def load_script_module(module_name: str, relative_path: str) -> ModuleType:
    module_path = REPO_ROOT / relative_path
    spec = importlib.util.spec_from_file_location(module_name, module_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Unable to load module from {module_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.fixture
def synthesize():
    return load_script_module("landfall_synthesize", "scripts/synthesize.py")


@pytest.fixture
def update_release():
    return load_script_module("landfall_update_release", "scripts/update-release.py")


@pytest.fixture
def report_synthesis_failure():
    return load_script_module(
        "landfall_report_synthesis_failure",
        "scripts/report-synthesis-failure.py",
    )
