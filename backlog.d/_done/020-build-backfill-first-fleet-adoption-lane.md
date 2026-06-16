# Build a backfill-first fleet adoption lane for repos with no release surface

Priority: P1 · Status: done · Estimate: L

## Goal
Let Landmark safely adopt active application/library repos that have packages but no release tags or release automation by producing local artifacts, an initial version policy, and an operator-approved first release path.

## Oracle
- [x] `fleet plan` distinguishes no-release repos that need initial versioning from repos that should remain non-release.
- [x] `fleet open-prs` can produce a safe manifest-only/local-artifacts PR for backfill-first repos without requiring release mutation.
- [x] The plan includes the initial tag/version recommendation, artifact paths, rollback, and the command to preview historical artifacts.
- [x] Replay fixtures cover Rust, TypeScript, Go, Python, and multi-package no-release repos.
- [x] The rollout report explains which repos remain blocked and why.

## Notes
- Evidence: the June 15, 2026 scan found 31 active application/library repos in `backfill-first` mode across `phrazzld` and `misty-step`.
- These should not get release-mutating workflows until the first-release policy is explicit.

## Delivered
- Promoted package-bearing no-release application/library repositories from blocked to ready `backfill-first` plans while keeping no-package non-release repositories skipped.
- Added fleet plan fields for initial version/tag recommendation, local artifact paths, historical artifacts-only preview command, and rollback guidance.
- Kept `fleet open-prs` backfill-first output manifest-only/local-artifacts first: `.landmark.yml`, `diff.md`, receipts, and no generated GitHub release workflow.
- Extended fleet replay fixtures across TypeScript, Rust, Go, Python, and multi-package no-release repositories, plus blocked/skipped rollout reporting.
- Verified with `cargo test -p landmark fleet_plan_ -- --nocapture`, `cargo run --locked -p landmark -- replay-action --scenario fleet_adoption_planner --evidence-dir .landmark/replay-020-backfill-first --format json`, and `bin/gate`.
