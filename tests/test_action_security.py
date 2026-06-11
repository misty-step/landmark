from __future__ import annotations

import re
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
ACTION_PATH = REPO_ROOT / "action.yml"
SYNC_V1_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "sync-v1-tag.yml"
RUN_DECLARATION_PATTERN = re.compile(r"^(?:\s*-\s+|\s+)run:\s*(.*)$")


def _run_block_violations(forbidden_pattern: re.Pattern[str]) -> list[tuple[int, str]]:
    lines = ACTION_PATH.read_text(encoding="utf-8").splitlines()
    violations: list[tuple[int, str]] = []
    in_run_block = False
    run_indent = 0

    for line_number, line in enumerate(lines, start=1):
        stripped = line.strip()
        indent = len(line) - len(line.lstrip(" "))

        if in_run_block:
            if stripped and indent <= run_indent:
                in_run_block = False
            elif forbidden_pattern.search(line):
                violations.append((line_number, stripped))

        match = RUN_DECLARATION_PATTERN.match(line)
        if not match:
            continue

        run_value = match.group(1).strip()
        if forbidden_pattern.search(run_value):
            violations.append((line_number, stripped))

        if run_value == "" or run_value.startswith("|") or run_value.startswith(">"):
            in_run_block = True
            run_indent = indent

    return violations


def test_action_run_blocks_do_not_inline_inputs_expressions() -> None:
    violations = _run_block_violations(re.compile(r"\${{\s*inputs\."))
    assert not violations, f"inline inputs expressions in run blocks: {violations}"


def test_action_run_blocks_do_not_inline_step_output_expressions() -> None:
    violations = _run_block_violations(re.compile(r"\${{\s*steps\."))
    assert not violations, f"inline steps expressions in run blocks: {violations}"


def test_action_run_blocks_do_not_inline_github_context_expressions() -> None:
    violations = _run_block_violations(re.compile(r"\${{\s*github\."))
    assert not violations, f"inline github expressions in run blocks: {violations}"


def test_floating_tag_update_runs_after_synthesis_and_gates_on_required_success() -> None:
    action_text = ACTION_PATH.read_text(encoding="utf-8")

    summarize_index = action_text.index("- name: Summarize synthesis status")
    floating_index = action_text.index("- name: Update floating major tag")
    assert floating_index > summarize_index

    assert "steps.synthesis_result.outputs.succeeded == 'true'" in action_text


def test_action_release_body_fetch_uses_bounded_python_helper() -> None:
    action_text = ACTION_PATH.read_text(encoding="utf-8")

    assert "curl -sSf" not in action_text
    assert "scripts/fetch-release-body.py" in action_text


def test_action_python_runtime_dependency_is_pinned() -> None:
    action_text = ACTION_PATH.read_text(encoding="utf-8")

    assert 'python -m pip install "requests==' in action_text
    assert "python -m pip install requests\n" not in action_text


def test_action_release_notes_output_delimiter_is_not_static() -> None:
    action_text = ACTION_PATH.read_text(encoding="utf-8")

    assert 'echo "notes<<LANDFALL_NOTES_EOF"' not in action_text
    assert "landfall-notes-eof-" in action_text
    assert "grep -Fxq" in action_text


def test_notifications_wait_for_artifact_publication_success() -> None:
    action_text = ACTION_PATH.read_text(encoding="utf-8")

    assert (
        "inputs.webhook-url != '' && steps.resolve_release.outputs.released == 'true' "
        "&& steps.write_artifacts.outputs.succeeded == 'true'"
    ) in action_text
    assert (
        "inputs.slack-webhook-url != '' && steps.resolve_release.outputs.released == 'true' "
        "&& steps.write_artifacts.outputs.succeeded == 'true'"
    ) in action_text
    assert (
        "inputs.webhook-url != '' && steps.resolve_release.outputs.released == 'true' "
        "&& steps.synthesize.outputs.succeeded == 'true'"
    ) not in action_text
    assert (
        "inputs.slack-webhook-url != '' && steps.resolve_release.outputs.released == 'true' "
        "&& steps.synthesize.outputs.succeeded == 'true'"
    ) not in action_text


def test_action_git_network_calls_are_bounded_and_retried() -> None:
    action_text = ACTION_PATH.read_text(encoding="utf-8")

    assert 'timeout 120s "$@"' in action_text
    assert 'retry_command "${rss_log}" git push origin "HEAD:${GITHUB_REF_NAME}"' in action_text
    assert 'retry_command "${tag_log}" git fetch --tags --force' in action_text
    assert 'retry_command "${tag_log}" git push origin "refs/tags/${major_tag}" --force' in action_text


def test_legacy_sync_v1_workflow_is_removed() -> None:
    assert not SYNC_V1_WORKFLOW_PATH.exists()
