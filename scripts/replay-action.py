#!/usr/bin/env python3
"""Replay Landfall release-integrity action policy scenarios."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
ACTION_PATH = REPO_ROOT / "action.yml"
POLICY_SCRIPT = REPO_ROOT / "scripts" / "release-policy.py"
SYNC_V1_WORKFLOW = REPO_ROOT / ".github" / "workflows" / "sync-v1-tag.yml"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Replay action-level release integrity scenarios.")
    parser.add_argument(
        "--evidence-dir",
        default="",
        help="Directory for replay-result.json. Defaults to a temporary directory.",
    )
    return parser.parse_args()


def parse_github_output(path: Path) -> dict[str, str]:
    outputs: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        outputs[key] = value
    return outputs


def run_policy(*args: str) -> dict[str, str]:
    with tempfile.NamedTemporaryFile("w+", encoding="utf-8", delete=False) as handle:
        output_path = Path(handle.name)
    try:
        subprocess.run(
            [sys.executable, str(POLICY_SCRIPT), *args, "--github-output", str(output_path)],
            cwd=REPO_ROOT,
            check=True,
            text=True,
            capture_output=True,
        )
        return parse_github_output(output_path)
    finally:
        output_path.unlink(missing_ok=True)


def assert_equal(actual: Any, expected: Any, message: str) -> None:
    if actual != expected:
        raise AssertionError(f"{message}: expected {expected!r}, got {actual!r}")


def scenario_publication_degraded_required() -> dict[str, str]:
    outputs = run_policy(
        "publication",
        "--synthesis-required",
        "true",
        "--synthesis-strict",
        "false",
        "--synth-succeeded",
        "true",
        "--synth-quality",
        "degraded",
    )
    assert_equal(outputs["can_update_release"], "false", "required degraded notes must not publish")
    assert_equal(outputs["succeeded"], "false", "required degraded notes must fail synthesis")
    assert_equal(outputs["failure_stage"], "synthesis_quality", "required degraded failure stage")
    return outputs


def scenario_publication_degraded_optional() -> dict[str, str]:
    outputs = run_policy(
        "publication",
        "--synthesis-required",
        "false",
        "--synthesis-strict",
        "false",
        "--synth-succeeded",
        "true",
        "--synth-quality",
        "degraded",
    )
    assert_equal(outputs["can_update_release"], "true", "optional degraded notes may publish")
    assert_equal(outputs["succeeded"], "true", "optional degraded notes keep synthesis successful")
    assert_equal(outputs["quality"], "degraded", "optional degraded quality is retained")
    return outputs


def scenario_summary_release_update_failed() -> dict[str, str]:
    outputs = run_policy(
        "summary",
        "--synthesis-enabled",
        "true",
        "--released",
        "true",
        "--synth-succeeded",
        "true",
        "--synth-quality",
        "valid",
        "--update-succeeded",
        "false",
        "--update-failure-stage",
        "release_update",
        "--update-failure-message",
        "patch failed",
        "--artifact-succeeded",
        "false",
    )
    assert_equal(outputs["succeeded"], "false", "release update failure must fail final status")
    assert_equal(outputs["failure_stage"], "release_update", "release update failure stage")
    return outputs


def scenario_summary_artifact_failed() -> dict[str, str]:
    outputs = run_policy(
        "summary",
        "--synthesis-enabled",
        "true",
        "--released",
        "true",
        "--synth-succeeded",
        "true",
        "--synth-quality",
        "valid",
        "--update-succeeded",
        "true",
        "--artifact-succeeded",
        "false",
        "--artifact-failure-stage",
        "artifact_write",
        "--artifact-failure-message",
        "write failed",
    )
    assert_equal(outputs["succeeded"], "false", "artifact failure must fail final status")
    assert_equal(outputs["failure_stage"], "artifact_write", "artifact failure stage")
    return outputs


def scenario_summary_rss_failed() -> dict[str, str]:
    outputs = run_policy(
        "summary",
        "--synthesis-enabled",
        "true",
        "--released",
        "true",
        "--synth-succeeded",
        "true",
        "--synth-quality",
        "valid",
        "--update-succeeded",
        "true",
        "--artifact-succeeded",
        "true",
        "--rss-enabled",
        "true",
        "--rss-succeeded",
        "false",
        "--rss-failure-stage",
        "rss_update",
        "--rss-failure-message",
        "push failed",
    )
    assert_equal(outputs["succeeded"], "false", "RSS failure must fail final status")
    assert_equal(outputs["failure_stage"], "rss_update", "RSS failure stage")
    return outputs


def scenario_action_static_contract() -> dict[str, str]:
    action_text = ACTION_PATH.read_text(encoding="utf-8")
    checks = {
        "no_curl_release_fetch": "curl -sSf" not in action_text,
        "uses_fetch_release_body": "scripts/fetch-release-body.py" in action_text,
        "pinned_requests": 'python -m pip install "requests==' in action_text,
        "dynamic_notes_delimiter": "landfall-notes-eof-" in action_text and "grep -Fxq" in action_text,
        "floating_tag_requires_success": "steps.synthesis_result.outputs.succeeded == 'true'" in action_text,
        "notifications_require_artifacts": (
            "steps.write_artifacts.outputs.succeeded == 'true'" in action_text
            and "inputs.webhook-url != '' && steps.resolve_release.outputs.released == 'true' && "
            "steps.synthesize.outputs.succeeded == 'true'" not in action_text
            and "inputs.slack-webhook-url != '' && steps.resolve_release.outputs.released == 'true' && "
            "steps.synthesize.outputs.succeeded == 'true'" not in action_text
        ),
        "git_network_calls_are_bounded": (
            "timeout 120s \"$@\"" in action_text
            and 'retry_command "${rss_log}" git push origin "HEAD:${GITHUB_REF_NAME}"' in action_text
            and 'retry_command "${tag_log}" git fetch --tags --force' in action_text
            and 'retry_command "${tag_log}" git push origin "refs/tags/${major_tag}" --force' in action_text
        ),
        "legacy_sync_v1_removed": not SYNC_V1_WORKFLOW.exists(),
    }
    failed = [name for name, ok in checks.items() if not ok]
    if failed:
        raise AssertionError(f"action static contract failed: {', '.join(failed)}")
    return {name: str(ok).lower() for name, ok in checks.items()}


def main() -> int:
    args = parse_args()
    if args.evidence_dir:
        evidence_dir = Path(args.evidence_dir)
    else:
        evidence_dir = Path(tempfile.mkdtemp(prefix="landfall-replay-"))
    evidence_dir.mkdir(parents=True, exist_ok=True)

    scenario_fns = {
        "publication_degraded_required": scenario_publication_degraded_required,
        "publication_degraded_optional": scenario_publication_degraded_optional,
        "summary_release_update_failed": scenario_summary_release_update_failed,
        "summary_artifact_failed": scenario_summary_artifact_failed,
        "summary_rss_failed": scenario_summary_rss_failed,
        "action_static_contract": scenario_action_static_contract,
    }
    scenarios: dict[str, dict[str, str]] = {}
    errors: dict[str, str] = {}
    for name, scenario_fn in scenario_fns.items():
        try:
            scenarios[name] = scenario_fn()
        except AssertionError as exc:
            errors[name] = str(exc)

    evidence = {
        "verdict": "failed" if errors else "passed",
        "scenarios": scenarios,
    }
    if errors:
        evidence["errors"] = errors

    evidence_path = evidence_dir / "replay-result.json"
    evidence_path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(evidence_path)
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
