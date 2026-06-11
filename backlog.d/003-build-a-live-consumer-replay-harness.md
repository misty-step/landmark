# Build a live consumer replay harness

Priority: P1 · Status: done · Estimate: XL

## Goal
Prove Landfall behavior in realistic consumer repositories before release, not only through script unit tests and string checks.

## Oracle
- [x] One local command creates disposable fixture repositories and replays full mode, synthesis-only mode, failure paths, and floating-tag behavior without real secrets.
- [x] The harness captures an evidence packet: generated release notes, mutated release body, git refs/tags, action outputs, and structured failure logs.
- [x] CI runs a bounded version of the harness on pull requests and the full packet on `master`.
- [x] The dogfood release workflow is validated through the same harness shape rather than bespoke assertions.
- [x] Local setup is reproducible from one command without relying on whatever packages happen to be installed globally.

## Children
1. Choose the replay substrate: local shell harness, `act`, or a purpose-built Python runner for composite-action steps.
2. Build fake GitHub Release and LLM endpoints that exercise success, 401/403, malformed response, timeout, degraded validation, and update failure.
3. Create fixture repos for semantic-release full mode, release-please synthesis-only mode, changesets synthesis-only mode, and manual tag mode.
4. Add a PR-safe composite-action smoke run for `uses: ./`, `mode: synthesis-only`, `release-tag: v0.0.0`, and `synthesis: "false"`.
5. Add action-level fallback cases for changelog present, release body fallback, PR extraction fallback, invalid `changelog-source`, and strict failure.
6. Add `actionlint` and shell safety checks over workflows, examples, and action run blocks.
7. Record evidence artifacts under a deterministic path and teach CI to upload them on failure.
8. Retire tests that only assert workflow strings once behavior-level replay covers the same contract.

## Notes
- Evidence: CI currently runs `python -m pytest -q tests/`, `ruff`, metadata sync, schema validation, and `npm ci`; it does not execute the composite action end to end.
- Evidence: `tests/test_release_workflow_dogfooding.py` checks strings such as `uses: ./`, `synthesis-required`, and `floating-tags`, but does not run the workflow.
- Evidence: the verification lane's `python3 -m pytest --collect-only -q -p no:cacheprovider tests/` failed with `ModuleNotFoundError: No module named 'requests'`, so a cold local gate is not self-contained.
- Evidence: external behavior is spread across GitHub API calls, semantic-release, LLM HTTP calls, git tag movement, artifact commits, Slack, webhooks, and RSS.
- Why: verification perspective found the highest-risk failures live between scripts and GitHub Actions orchestration.

## Delivery
- Expanded `scripts/replay-action.py` from policy/static checks into a consumer
  replay harness with disposable git fixtures and local fake GitHub/LLM services.
- Added scenarios for full mode, synthesis-only mode, degraded strict failure,
  release-body update failure, and floating-tag behavior.
- Replay evidence now includes action outputs, generated notes, release body
  before/after, git tags, artifacts, structured logs, and fake service requests.
- CI runs a bounded replay on pull requests, the full replay on `master`, and
  uploads `.landfall/replay/` as an artifact.
- Added `bin/gate` as the local one-command verification loop.
