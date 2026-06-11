#!/usr/bin/env python3
"""Replay Landfall action behavior in disposable consumer fixtures."""

from __future__ import annotations

import argparse
import contextlib
import json
import os
import subprocess
import sys
import tempfile
import threading
from pathlib import Path
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any
from urllib.parse import unquote


REPO_ROOT = Path(__file__).resolve().parents[1]
ACTION_PATH = REPO_ROOT / "action.yml"
POLICY_SCRIPT = REPO_ROOT / "scripts" / "release-policy.py"
SYNC_V1_WORKFLOW = REPO_ROOT / ".github" / "workflows" / "sync-v1-tag.yml"
SCRIPTS_DIR = REPO_ROOT / "scripts"
VALID_NOTES = """## Improvements

- Added a replay harness that checks release behavior in a disposable repo.
- Captured release body updates, artifacts, tags, and structured logs.
- Kept the run local so no production secrets or GitHub releases are touched.
"""
INVALID_NOTES = "hello, here are the release notes"


class ReplayCommandError(RuntimeError):
    def __init__(self, command: list[str], result: subprocess.CompletedProcess[str]):
        self.command = command
        self.result = result
        super().__init__(f"{' '.join(command)} exited {result.returncode}")


class FakeServiceState:
    def __init__(
        self,
        *,
        llm_status: int = 200,
        llm_notes: str = VALID_NOTES,
        update_status: int = 200,
    ) -> None:
        self.llm_status = llm_status
        self.llm_notes = llm_notes
        self.update_status = update_status
        self.releases: dict[str, dict[str, Any]] = {}
        self.requests: list[dict[str, Any]] = []

    def add_release(self, tag: str, body: str) -> None:
        release_id = len(self.releases) + 1
        self.releases[tag] = {
            "id": release_id,
            "tag_name": tag,
            "body": body,
            "html_url": f"https://example.invalid/releases/{tag}",
        }

    def release_by_id(self, release_id: int) -> dict[str, Any] | None:
        for release in self.releases.values():
            if release["id"] == release_id:
                return release
        return None


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Replay action-level release integrity scenarios.")
    parser.add_argument(
        "--evidence-dir",
        default="",
        help="Directory for replay-result.json. Defaults to a temporary directory.",
    )
    parser.add_argument(
        "--scenario",
        action="append",
        default=[],
        help="Scenario name to run. May be passed multiple times. Defaults to all scenarios.",
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


def run_command(
    command: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    process_env = os.environ.copy()
    if env:
        process_env.update(env)
    result = subprocess.run(
        command,
        cwd=cwd,
        env=process_env,
        text=True,
        capture_output=True,
    )
    if check and result.returncode != 0:
        raise ReplayCommandError(command, result)
    return result


def run_script(
    script_name: str,
    *args: str,
    cwd: Path,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    return run_command([sys.executable, str(SCRIPTS_DIR / script_name), *args], cwd=cwd, check=check)


def command_receipt(result: subprocess.CompletedProcess[str]) -> dict[str, Any]:
    return {
        "returncode": result.returncode,
        "stdout": result.stdout.strip(),
        "stderr": result.stderr.strip(),
    }


def init_fixture_repo(path: Path, *, name: str, release_tag: str, changelog: str) -> None:
    path.mkdir(parents=True, exist_ok=True)
    run_command(["git", "init", "-q"], cwd=path)
    run_command(["git", "config", "user.name", "Landfall Replay"], cwd=path)
    run_command(["git", "config", "user.email", "replay@example.invalid"], cwd=path)
    (path / "README.md").write_text(f"# {name}\n", encoding="utf-8")
    (path / "CHANGELOG.md").write_text(changelog, encoding="utf-8")
    run_command(["git", "add", "."], cwd=path)
    run_command(["git", "commit", "-q", "-m", "feat: seed replay fixture"], cwd=path)
    run_command(["git", "tag", release_tag], cwd=path)


def git_tags(path: Path) -> list[str]:
    result = run_command(["git", "tag", "--list", "--sort=refname"], cwd=path)
    return [line for line in result.stdout.splitlines() if line.strip()]


def write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def build_fake_handler(state: FakeServiceState) -> type[BaseHTTPRequestHandler]:
    class FakeHandler(BaseHTTPRequestHandler):
        def _record(self, body: str = "") -> None:
            state.requests.append({"method": self.command, "path": self.path, "body": body})

        def _json(self, status: int, payload: Any) -> None:
            body = json.dumps(payload).encode("utf-8")
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def _text(self, status: int, body: str) -> None:
            raw = body.encode("utf-8")
            self.send_response(status)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(raw)))
            self.end_headers()
            self.wfile.write(raw)

        def do_GET(self) -> None:  # noqa: N802
            self._record()
            marker = "/releases/tags/"
            if marker not in self.path:
                self._text(404, "not found")
                return
            tag = unquote(self.path.rsplit(marker, 1)[1])
            release = state.releases.get(tag)
            if release is None:
                self._json(404, {"message": "Not Found"})
                return
            self._json(200, release)

        def do_PATCH(self) -> None:  # noqa: N802
            raw = self.rfile.read(int(self.headers.get("Content-Length", "0"))).decode("utf-8")
            self._record(raw)
            if state.update_status >= 400:
                self._json(state.update_status, {"message": "update failed"})
                return
            try:
                release_id = int(self.path.rsplit("/releases/", 1)[1])
            except (IndexError, ValueError):
                self._text(404, "not found")
                return
            release = state.release_by_id(release_id)
            if release is None:
                self._json(404, {"message": "Not Found"})
                return
            payload = json.loads(raw or "{}")
            if isinstance(payload.get("body"), str):
                release["body"] = payload["body"]
            self._json(200, release)

        def do_POST(self) -> None:  # noqa: N802
            raw = self.rfile.read(int(self.headers.get("Content-Length", "0"))).decode("utf-8")
            self._record(raw)
            if self.path != "/chat/completions":
                self._text(404, "not found")
                return
            if state.llm_status >= 400:
                self._json(state.llm_status, {"error": {"message": "fake LLM failure"}})
                return
            self._json(
                200,
                {"choices": [{"message": {"content": state.llm_notes}}]},
            )

        def log_message(self, format: str, *args: Any) -> None:
            return

    return FakeHandler


@contextlib.contextmanager
def fake_service(state: FakeServiceState):
    server = ThreadingHTTPServer(("127.0.0.1", 0), build_fake_handler(state))
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}"
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


def synthesize_notes(
    *,
    repo: Path,
    api_url: str,
    release_tag: str,
    changelog_source: str,
    quality_path: Path,
    technical_changelog: Path | None = None,
    release_body: Path | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    args = [
        "--api-key",
        "fake-key",
        "--api-url",
        f"{api_url}/chat/completions",
        "--model",
        "fake/local",
        "--version",
        release_tag,
        "--changelog-source",
        changelog_source,
        "--quality-file",
        str(quality_path),
        "--timeout",
        "2",
        "--retries",
        "0",
        "--retry-backoff",
        "0",
    ]
    if technical_changelog is not None:
        args.extend(["--technical-changelog-file", str(technical_changelog)])
    if release_body is not None:
        args.extend(["--release-body-file", str(release_body)])
    return run_script("synthesize.py", *args, cwd=repo, check=check)


def update_release_body(
    *,
    repo: Path,
    api_url: str,
    release_tag: str,
    notes_file: Path,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    return run_script(
        "update-release.py",
        "--github-token",
        "fake-token",
        "--repository",
        "octo/replay",
        "--tag",
        release_tag,
        "--notes-file",
        str(notes_file),
        "--api-base-url",
        api_url,
        "--timeout",
        "2",
        "--retries",
        "0",
        "--retry-backoff",
        "0",
        cwd=repo,
        check=check,
    )


def write_note_artifacts(repo: Path, *, release_tag: str, notes_file: Path) -> subprocess.CompletedProcess[str]:
    return run_script(
        "write-artifacts.py",
        "--notes-file",
        str(notes_file),
        "--version",
        release_tag,
        "--output-file",
        "docs/releases/{version}.md",
        "--output-text-file",
        "docs/releases/{version}.txt",
        "--output-html-file",
        "docs/releases/{version}.html",
        "--output-json",
        "docs/releases/index.json",
        cwd=repo,
    )


def write_synthesis_result(repo: Path, *, release_tag: str, notes: str, quality: str) -> dict[str, str]:
    notes_path = repo / ".landfall" / "notes.md"
    notes_path.parent.mkdir(parents=True, exist_ok=True)
    notes_path.write_text(notes.strip() + "\n", encoding="utf-8")
    return {
        "released": "true",
        "release-tag": release_tag,
        "synthesis-succeeded": "true" if quality in {"valid", "degraded"} else "false",
        "synthesis-quality": quality,
        "release-notes": notes.strip(),
    }


def scenario_consumer_full_mode_success(tmp_root: Path) -> dict[str, Any]:
    repo = tmp_root / "full-mode-consumer"
    release_tag = "v1.2.3"
    changelog = """# Changelog

## 1.2.3

### Features
- Add a release replay harness for operators.
"""
    init_fixture_repo(repo, name="Full Mode Consumer", release_tag=release_tag, changelog=changelog)
    state = FakeServiceState()
    state.add_release(release_tag, "## 1.2.3\n\n- Technical release body before synthesis.\n")

    with fake_service(state) as api_url:
        body_before = state.releases[release_tag]["body"]
        quality_path = repo / ".landfall" / "quality.txt"
        synth = synthesize_notes(
            repo=repo,
            api_url=api_url,
            release_tag=release_tag,
            changelog_source="changelog",
            quality_path=quality_path,
        )
        quality = quality_path.read_text(encoding="utf-8")
        outputs = write_synthesis_result(repo, release_tag=release_tag, notes=synth.stdout, quality=quality)
        notes_file = repo / ".landfall" / "notes.md"
        update = update_release_body(repo=repo, api_url=api_url, release_tag=release_tag, notes_file=notes_file)
        artifacts = write_note_artifacts(repo, release_tag=release_tag, notes_file=notes_file)
        floating = run_script("update-floating-tag.py", "--release-tag", release_tag, cwd=repo)
        run_command(["git", "tag", "-f", floating.stdout.strip(), release_tag], cwd=repo)
        body_after = state.releases[release_tag]["body"]

    assert_equal(quality, "valid", "full mode quality")
    assert "## What's New" in body_after
    assert "Technical release body before synthesis" in body_after
    return {
        "fixture": str(repo),
        "mode": "full",
        "action_outputs": outputs,
        "release_body_before": body_before,
        "release_body_after": body_after,
        "generated_release_notes": synth.stdout.strip(),
        "tags": git_tags(repo),
        "artifacts": {
            "markdown": (repo / "docs" / "releases" / f"{release_tag}.md").read_text(encoding="utf-8").strip(),
            "json": json.loads((repo / "docs" / "releases" / "index.json").read_text(encoding="utf-8")),
        },
        "commands": {
            "synthesize": command_receipt(synth),
            "update_release": command_receipt(update),
            "write_artifacts": command_receipt(artifacts),
            "floating_tag": command_receipt(floating),
        },
        "fake_service_requests": state.requests,
    }


def scenario_consumer_synthesis_only_success(tmp_root: Path) -> dict[str, Any]:
    repo = tmp_root / "synthesis-only-consumer"
    release_tag = "v0.0.0"
    init_fixture_repo(
        repo,
        name="Synthesis Only Consumer",
        release_tag=release_tag,
        changelog="# Changelog\n\n## 0.0.0\n\n- Manual release fixture.\n",
    )
    state = FakeServiceState()
    release_body_text = "## 0.0.0\n\n- Release body fallback source.\n"
    state.add_release(release_tag, release_body_text)
    release_body_file = repo / ".landfall" / "release-body.md"
    release_body_file.parent.mkdir(parents=True, exist_ok=True)
    release_body_file.write_text(release_body_text, encoding="utf-8")

    with fake_service(state) as api_url:
        quality_path = repo / ".landfall" / "quality.txt"
        synth = synthesize_notes(
            repo=repo,
            api_url=api_url,
            release_tag=release_tag,
            changelog_source="release-body",
            quality_path=quality_path,
            release_body=release_body_file,
        )
        quality = quality_path.read_text(encoding="utf-8")
        outputs = write_synthesis_result(repo, release_tag=release_tag, notes=synth.stdout, quality=quality)
        notes_file = repo / ".landfall" / "notes.md"
        update = update_release_body(repo=repo, api_url=api_url, release_tag=release_tag, notes_file=notes_file)

    assert_equal(quality, "valid", "synthesis-only quality")
    return {
        "fixture": str(repo),
        "mode": "synthesis-only",
        "action_outputs": outputs,
        "release_body_after": state.releases[release_tag]["body"],
        "generated_release_notes": synth.stdout.strip(),
        "tags": git_tags(repo),
        "commands": {
            "synthesize": command_receipt(synth),
            "update_release": command_receipt(update),
        },
        "fake_service_requests": state.requests,
    }


def scenario_consumer_degraded_required_fails(tmp_root: Path) -> dict[str, Any]:
    repo = tmp_root / "degraded-required-consumer"
    release_tag = "v2.0.0"
    init_fixture_repo(
        repo,
        name="Degraded Required Consumer",
        release_tag=release_tag,
        changelog="# Changelog\n\n## 2.0.0\n\n- Breaking replay fixture.\n",
    )
    state = FakeServiceState(llm_notes=INVALID_NOTES)

    with fake_service(state) as api_url:
        quality_path = repo / ".landfall" / "quality.txt"
        synth = synthesize_notes(
            repo=repo,
            api_url=api_url,
            release_tag=release_tag,
            changelog_source="changelog",
            quality_path=quality_path,
        )
        quality = quality_path.read_text(encoding="utf-8")
        policy = run_policy(
            "publication",
            "--synthesis-required",
            "true",
            "--synthesis-strict",
            "false",
            "--synth-succeeded",
            "true",
            "--synth-quality",
            quality,
        )

    assert_equal(quality, "degraded", "required scenario records degraded notes")
    assert_equal(policy["can_update_release"], "false", "required degraded notes cannot update release")
    return {
        "fixture": str(repo),
        "mode": "failure",
        "generated_release_notes": synth.stdout.strip(),
        "quality": quality,
        "policy_outputs": policy,
        "structured_failure_logs": synth.stderr.strip().splitlines(),
        "fake_service_requests": state.requests,
    }


def scenario_consumer_release_update_failure(tmp_root: Path) -> dict[str, Any]:
    repo = tmp_root / "update-failure-consumer"
    release_tag = "v3.1.4"
    init_fixture_repo(
        repo,
        name="Update Failure Consumer",
        release_tag=release_tag,
        changelog="# Changelog\n\n## 3.1.4\n\n- Patch replay fixture.\n",
    )
    state = FakeServiceState(update_status=500)
    state.add_release(release_tag, "## 3.1.4\n\n- Existing body.\n")

    with fake_service(state) as api_url:
        quality_path = repo / ".landfall" / "quality.txt"
        synth = synthesize_notes(
            repo=repo,
            api_url=api_url,
            release_tag=release_tag,
            changelog_source="changelog",
            quality_path=quality_path,
        )
        outputs = write_synthesis_result(
            repo,
            release_tag=release_tag,
            notes=synth.stdout,
            quality=quality_path.read_text(encoding="utf-8"),
        )
        notes_file = repo / ".landfall" / "notes.md"
        update = update_release_body(
            repo=repo,
            api_url=api_url,
            release_tag=release_tag,
            notes_file=notes_file,
            check=False,
        )

    assert_equal(update.returncode, 1, "release update failure exits non-zero")
    return {
        "fixture": str(repo),
        "mode": "failure",
        "action_outputs": outputs,
        "release_body_after": state.releases[release_tag]["body"],
        "commands": {
            "synthesize": command_receipt(synth),
            "update_release": command_receipt(update),
        },
        "structured_failure_logs": update.stderr.strip().splitlines(),
        "fake_service_requests": state.requests,
    }


def scenario_consumer_floating_tag_behavior(tmp_root: Path) -> dict[str, Any]:
    repo = tmp_root / "floating-tag-consumer"
    init_fixture_repo(
        repo,
        name="Floating Tag Consumer",
        release_tag="v4.5.6",
        changelog="# Changelog\n\n## 4.5.6\n\n- Stable release.\n",
    )
    stable = run_script("update-floating-tag.py", "--release-tag", "v4.5.6", cwd=repo)
    prerelease = run_script("update-floating-tag.py", "--release-tag", "v4.5.7-rc.1", cwd=repo)
    invalid = run_script("update-floating-tag.py", "--release-tag", "not-a-tag", cwd=repo, check=False)
    run_command(["git", "tag", "-f", stable.stdout.strip(), "v4.5.6"], cwd=repo)

    assert_equal(stable.stdout.strip(), "v4", "stable semver emits major tag")
    assert_equal(prerelease.stdout.strip(), "", "prerelease does not emit floating tag")
    assert_equal(invalid.returncode, 1, "invalid tag fails")
    return {
        "fixture": str(repo),
        "stable_output": stable.stdout.strip(),
        "prerelease_output": prerelease.stdout.strip(),
        "invalid": command_receipt(invalid),
        "tags": git_tags(repo),
    }


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

    policy_scenario_fns = {
        "publication_degraded_required": scenario_publication_degraded_required,
        "publication_degraded_optional": scenario_publication_degraded_optional,
        "summary_release_update_failed": scenario_summary_release_update_failed,
        "summary_artifact_failed": scenario_summary_artifact_failed,
        "summary_rss_failed": scenario_summary_rss_failed,
        "action_static_contract": scenario_action_static_contract,
    }
    consumer_scenario_fns = {
        "consumer_full_mode_success": scenario_consumer_full_mode_success,
        "consumer_synthesis_only_success": scenario_consumer_synthesis_only_success,
        "consumer_degraded_required_fails": scenario_consumer_degraded_required_fails,
        "consumer_release_update_failure": scenario_consumer_release_update_failure,
        "consumer_floating_tag_behavior": scenario_consumer_floating_tag_behavior,
    }
    scenario_fns: dict[str, Any] = {**policy_scenario_fns, **consumer_scenario_fns}

    requested = args.scenario or list(scenario_fns)
    unknown = [name for name in requested if name not in scenario_fns]
    if unknown:
        print(f"unknown replay scenario(s): {', '.join(unknown)}", file=sys.stderr)
        return 2

    scenarios: dict[str, Any] = {}
    errors: dict[str, str] = {}
    with tempfile.TemporaryDirectory(prefix="landfall-consumer-replay-") as tmp:
        tmp_root = Path(tmp)
        for name in requested:
            scenario_fn = scenario_fns[name]
            try:
                if name in consumer_scenario_fns:
                    scenarios[name] = scenario_fn(tmp_root)
                else:
                    scenarios[name] = scenario_fn()
            except (AssertionError, ReplayCommandError) as exc:
                if isinstance(exc, ReplayCommandError):
                    errors[name] = json.dumps(
                        {
                            "error": str(exc),
                            "returncode": exc.result.returncode,
                            "stdout": exc.result.stdout.strip(),
                            "stderr": exc.result.stderr.strip(),
                        },
                        sort_keys=True,
                    )
                else:
                    errors[name] = str(exc)

    evidence = {
        "verdict": "failed" if errors else "passed",
        "scenario_count": len(requested),
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
