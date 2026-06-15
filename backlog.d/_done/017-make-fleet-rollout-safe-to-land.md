# Make fleet rollout safe to land across active projects

Priority: P1 · Status: done · Estimate: XL

## Goal
Turn fleet adoption from a dry-run planner into a safe, monitored rollout path
for personal and Mistystep organization repositories.

## Oracle
- [x] Fleet classification distinguishes applications, libraries, infrastructure, archived repos, experiments, and repos that should not release.
- [x] Fleet plan chooses local, generic CI, GitHub full, GitHub synthesis-only, manifest-only, or backfill-first modes with explicit rationale.
- [x] Secret requirements are minimized per mode and provisioning/verification never prints secret values.
- [x] A guarded non-dry-run PR path exists with branch naming, commit messages, rollback/disposition, and per-repo evidence receipts.
- [x] Downstream release runs are monitored and summarized after PR merge before rollout continues.
- [x] The repo-local dogfood skill and dogfood report stay aligned with the fleet workflow.

## Children
1. Add app/library/release-surface classification to `fleet scan` and `fleet plan`.
2. Add secret provisioning or verification affordances for GitHub and a no-secret local/generic mode.
3. Implement guarded PR creation with dry-run parity and refusal rules for risky repos.
4. Add post-merge release monitoring and a durable rollout dashboard.
5. Teach fleet to recommend non-GitHub/generic integration once `013` lands.
6. Keep `skills/landfall-dogfood` updated with the operational workflow.

## Notes
- Evidence: fleet currently refuses remote mutation and writes dry-run PR files only.
- Evidence: prior dogfood treated downstream hosted behavior as the real oracle.
- Why: Landfall's value accrues when many repos adopt it safely, not when one repo has a good demo.

## Delivered
- Added `repository_kind`, `release_surface`, `integration_mode`, `integration_rationale`, and mode-scoped `required_secrets` to fleet scan/plan artifacts.
- Added non-release classification, direct classifier tests, package-registry release-surface precedence, and incomplete secret metadata blocking for GitHub modes.
- Tightened `fleet open-prs` so confirmed rollout requires `--confirm-remote --max-prs 1`, writes a per-repo `APPLY.md`, and records branch, commit message, rollback, disposition, monitor status, and evidence directory.
- Updated README, fleet plan schema, dogfood report, and `skills/landfall-dogfood`.
- Evidence: `cargo run --locked -p landfall -- replay-action --scenario fleet_adoption_planner --evidence-dir .landfall/replay-017-fleet` passed with 14 fixture repositories.
- Evidence: `bin/gate` passed with 28 unit tests, action contract validation, full replay, and Linux action binary parity handoff.
- Review: Claude re-review returned `NO BLOCKING FINDINGS` after review fixes.
