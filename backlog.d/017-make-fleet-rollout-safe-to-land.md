# Make fleet rollout safe to land across active projects

Priority: P1 · Status: pending · Estimate: XL

## Goal
Turn fleet adoption from a dry-run planner into a safe, monitored rollout path
for personal and Mistystep organization repositories.

## Oracle
- [ ] Fleet classification distinguishes applications, libraries, infrastructure, archived repos, experiments, and repos that should not release.
- [ ] Fleet plan chooses local, generic CI, GitHub full, GitHub synthesis-only, manifest-only, or backfill-first modes with explicit rationale.
- [ ] Secret requirements are minimized per mode and provisioning/verification never prints secret values.
- [ ] A guarded non-dry-run PR path exists with branch naming, commit messages, rollback/disposition, and per-repo evidence receipts.
- [ ] Downstream release runs are monitored and summarized after PR merge before rollout continues.
- [ ] The repo-local dogfood skill and dogfood report stay aligned with the fleet workflow.

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
