from __future__ import annotations

import json
import subprocess
import sys
import urllib.request
from pathlib import Path


def test_replay_action_selected_consumer_scenarios_write_evidence(repo_root: Path, tmp_path: Path) -> None:
    evidence_dir = tmp_path / "evidence"
    result = subprocess.run(
        [
            sys.executable,
            str(repo_root / "scripts" / "replay-action.py"),
            "--evidence-dir",
            str(evidence_dir),
            "--scenario",
            "consumer_full_mode_success",
            "--scenario",
            "consumer_degraded_required_fails",
        ],
        cwd=repo_root,
        text=True,
        capture_output=True,
        check=True,
    )

    evidence_path = evidence_dir / "replay-result.json"
    assert str(evidence_path) in result.stdout
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))

    assert evidence["verdict"] == "passed"
    assert evidence["scenario_count"] == 2

    full_mode = evidence["scenarios"]["consumer_full_mode_success"]
    assert full_mode["mode"] == "full"
    assert full_mode["action_outputs"]["released"] == "true"
    assert full_mode["action_outputs"]["synthesis-quality"] == "valid"
    assert "## What's New" in full_mode["release_body_after"]
    assert "v1" in full_mode["tags"]
    assert full_mode["artifacts"]["json"][0]["version"] == "1.2.3"

    degraded = evidence["scenarios"]["consumer_degraded_required_fails"]
    assert degraded["quality"] == "degraded"
    assert degraded["policy_outputs"]["can_update_release"] == "false"
    assert degraded["structured_failure_logs"]


def test_replay_action_rejects_unknown_scenario(repo_root: Path, tmp_path: Path) -> None:
    result = subprocess.run(
        [
            sys.executable,
            str(repo_root / "scripts" / "replay-action.py"),
            "--evidence-dir",
            str(tmp_path),
            "--scenario",
            "missing",
        ],
        cwd=repo_root,
        text=True,
        capture_output=True,
        check=False,
    )

    assert result.returncode == 2
    assert "unknown replay scenario(s): missing" in result.stderr


def test_fake_service_updates_release_body(replay_action) -> None:
    state = replay_action.FakeServiceState()
    state.add_release("v1.0.0", "old")

    with replay_action.fake_service(state) as api_url:
        release_response = urllib.request.urlopen(f"{api_url}/repos/octo/replay/releases/tags/v1.0.0", timeout=2)
        release = json.loads(release_response.read().decode("utf-8"))
        assert release["body"] == "old"

        request = urllib.request.Request(
            f"{api_url}/repos/octo/replay/releases/1",
            data=json.dumps({"body": "new"}).encode("utf-8"),
            headers={"Content-Type": "application/json"},
            method="PATCH",
        )
        update_response = urllib.request.urlopen(request, timeout=2)
        updated = json.loads(update_response.read().decode("utf-8"))

    assert updated["body"] == "new"
    assert state.releases["v1.0.0"]["body"] == "new"
