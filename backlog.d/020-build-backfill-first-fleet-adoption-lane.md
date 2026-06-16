# Build a backfill-first fleet adoption lane for repos with no release surface

Priority: P1 · Status: pending · Estimate: L

## Goal
Let Landmark safely adopt active application/library repos that have packages but no release tags or release automation by producing local artifacts, an initial version policy, and an operator-approved first release path.

## Oracle
- [ ] `fleet plan` distinguishes no-release repos that need initial versioning from repos that should remain non-release.
- [ ] `fleet open-prs` can produce a safe manifest-only/local-artifacts PR for backfill-first repos without requiring release mutation.
- [ ] The plan includes the initial tag/version recommendation, artifact paths, rollback, and the command to preview historical artifacts.
- [ ] Replay fixtures cover Rust, TypeScript, Go, Python, and multi-package no-release repos.
- [ ] The rollout report explains which repos remain blocked and why.

## Notes
- Evidence: the June 15, 2026 scan found 31 active application/library repos in `backfill-first` mode across `phrazzld` and `misty-step`.
- These should not get release-mutating workflows until the first-release policy is explicit.
