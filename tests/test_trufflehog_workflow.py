from __future__ import annotations

from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
TRUFFLEHOG_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "trufflehog.yml"


def _trufflehog_workflow() -> str:
    return TRUFFLEHOG_WORKFLOW_PATH.read_text(encoding="utf-8")


def test_trufflehog_uses_event_range_on_master_push() -> None:
    workflow = _trufflehog_workflow()
    assert "- master" in workflow
    assert "- main" not in workflow
    assert "github.event.before" in workflow
    assert "github.sha" in workflow


def test_trufflehog_uses_pr_sha_range_on_pull_request() -> None:
    workflow = _trufflehog_workflow()
    assert "github.event.pull_request.base.sha" in workflow
    assert "github.event.pull_request.head.sha" in workflow


def test_trufflehog_does_not_compare_default_branch_to_head() -> None:
    workflow = _trufflehog_workflow()
    assert "github.event.repository.default_branch" not in workflow
    assert "head: HEAD" not in workflow
