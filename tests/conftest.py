from __future__ import annotations

import argparse
import importlib.util
import sys
from pathlib import Path
from types import ModuleType
from typing import Any, Callable

import pytest
import requests


REPO_ROOT = Path(__file__).resolve().parents[1]


def load_script_module(module_name: str, relative_path: str) -> ModuleType:
    module_path = REPO_ROOT / relative_path
    scripts_dir = str(module_path.parent)
    if scripts_dir not in sys.path:
        sys.path.insert(0, scripts_dir)

    spec = importlib.util.spec_from_file_location(module_name, module_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load module from {module_path}")

    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="session")
def repo_root() -> Path:
    return REPO_ROOT


@pytest.fixture
def make_namespace() -> Callable[..., argparse.Namespace]:
    def _factory(**kwargs: Any) -> argparse.Namespace:
        return argparse.Namespace(**kwargs)

    return _factory


class FakeResponse:
    def __init__(self, *, status_code: int, json_data: Any = None, text: str = ""):
        self.status_code = status_code
        self._json_data = json_data
        self.text = text

    def json(self) -> Any:
        return self._json_data

    def raise_for_status(self) -> None:
        if self.status_code >= 400:
            error = requests.HTTPError(f"HTTP {self.status_code}")
            error.response = self
            raise error


class RequestSequenceSession:
    def __init__(self, outcomes: list[object]):
        self._outcomes = list(outcomes)
        self.calls: list[dict[str, Any]] = []
        self.closed = False

    def request(self, *, method: str, url: str, timeout: int, **kwargs: Any) -> FakeResponse:
        self.calls.append({"method": method, "url": url, "timeout": timeout, "kwargs": kwargs})
        outcome = self._outcomes.pop(0)
        if isinstance(outcome, Exception):
            raise outcome
        return outcome

    def close(self) -> None:
        self.closed = True


class PostCaptureSession:
    def __init__(self, outcome: object):
        self._outcome = outcome
        self.calls: list[dict[str, Any]] = []
        self.closed = False

    def post(self, *, url: str, headers: dict[str, str], json: Any, timeout: int) -> FakeResponse:
        self.calls.append({"url": url, "headers": headers, "json": json, "timeout": timeout})
        if isinstance(self._outcome, Exception):
            raise self._outcome
        return self._outcome

    def close(self) -> None:
        self.closed = True


@pytest.fixture
def request_session_factory():
    return RequestSequenceSession


@pytest.fixture
def post_session_factory():
    return PostCaptureSession


@pytest.fixture(scope="session")
def update_floating_tag():
    return load_script_module("landfall_update_floating_tag", "scripts/update-floating-tag.py")


@pytest.fixture(scope="session")
def update_release():
    return load_script_module("landfall_update_release", "scripts/update-release.py")


@pytest.fixture(scope="session")
def write_artifacts():
    return load_script_module("landfall_write_artifacts", "scripts/write-artifacts.py")


@pytest.fixture(scope="session")
def update_version_metadata():
    return load_script_module("landfall_update_version_metadata", "scripts/update-version-metadata.py")


@pytest.fixture(scope="session")
def check_version_sync():
    return load_script_module("landfall_check_version_sync", "scripts/check-version-sync.py")


@pytest.fixture(scope="session")
def report_synthesis_failure():
    return load_script_module(
        "landfall_report_synthesis_failure",
        "scripts/report-synthesis-failure.py",
    )
