from __future__ import annotations

from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
RELEASE_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"


def _release_workflow() -> str:
    return RELEASE_WORKFLOW_PATH.read_text(encoding="utf-8")


def test_release_workflow_runs_local_landfall_action() -> None:
    workflow = _release_workflow()
    assert "uses: ./" in workflow
    assert "name: Run Landfall" in workflow


def test_release_workflow_is_manual_for_protected_master() -> None:
    workflow = _release_workflow()
    assert "workflow_dispatch:" in workflow
    assert "push:" not in workflow


def test_release_workflow_uses_landfall_for_strictness_and_floating_tags() -> None:
    workflow = _release_workflow()
    assert 'synthesis-required: "true"' in workflow
    assert 'floating-tags: "true"' in workflow
    assert "synthesis-strict" not in workflow
    assert "Move v1 major tag" not in workflow
    assert "git tag -f v1" not in workflow
