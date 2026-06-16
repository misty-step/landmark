# Harden release integrity policy

Priority: P0 · Status: done · Estimate: XL

## Goal
Make Landmark's release pipeline fail, warn, publish, and tag according to an explicit integrity policy instead of scattered step-local conventions.

## Oracle
- [x] A policy test matrix covers `synthesis-required`, `synthesis-quality`, release-body update failure, artifact write failure, RSS commit failure, and floating-tag update gating.
- [x] Required pipelines fail on degraded or failed notes when policy says they must, and optional pipelines report partial publication without moving protected outputs.
- [x] Every external call in `action.yml` has bounded timeout, retry behavior, and reviewable error output.
- [x] Static `GITHUB_OUTPUT` heredoc delimiters cannot be truncated by generated notes content.
- [x] The legacy `sync-v1-tag` workflow is either removed or proven not to race Landmark's built-in floating-tag step.
- [x] CI proves the policy with unit tests plus an action-level replay.

## Children
1. Define the release integrity state machine: no release, released without synthesis, valid synthesis published, degraded synthesis, release body update failed, distribution failed.
2. Replace the release-body `curl -sSf` shell helper with a Python helper using `request_with_retry`, timeout, structured logs, and explicit 404 handling.
3. Decide whether `synthesis-required=true` treats `synthesis-quality=degraded` as failure, warning, or a separate opt-in.
4. Replace static `LANDMARK_NOTES_EOF` output writing with collision-safe output serialization.
5. Pin or constrain Python runtime dependencies installed by the action and prove installability in CI.
6. Move release policy decisions out of shell snippets and into a tested Python orchestration layer, leaving `action.yml` as a thin wrapper.
7. Delete `.github/workflows/sync-v1-tag.yml`, or keep it only with a race/concurrency oracle proving it cannot fight `floating-tags: "true"`.
8. Add tests that assert failure boundaries instead of only checking shell interpolation.

## Notes
- Evidence: `action.yml:329-334` fetches release body with `curl -sSf` and no timeout or retry.
- Evidence: `scripts/synthesize.py:762-828` returns degraded notes with exit 0 after validation retry failure; `action.yml:459-465` marks the synthesis step succeeded.
- Evidence: `action.yml:510-526` converts release body update failure into a warning and output state; the release is already published.
- Evidence: `action.yml:560-562` uses a static `LANDMARK_NOTES_EOF` delimiter for generated content.
- Evidence: `action.yml:165-166` installs latest `requests` at runtime.
- Evidence: `.github/workflows/release.yml` already passes `floating-tags: "true"`, while `.github/workflows/sync-v1-tag.yml` still force-moves `v1` independently.
- Evidence: `action.yml` repeats shell helpers such as `set_output` and keeps release policy state spread across multiple run blocks.
- Why: security/operations review found multiple issues with the same root cause: release integrity is implicit and duplicated across shell steps.

## Delivery
- Implemented in `scripts/release-policy.py`, `scripts/fetch-release-body.py`, `scripts/replay-action.py`, `action.yml`, and focused policy tests.
- Verification: `python -m pytest -q tests/`; `python scripts/replay-action.py --evidence-dir /tmp/landmark-replay-evidence`; `actionlint`; `check-jsonschema` for `action.yml`.
