# Landfall Dogfood Finding Taxonomy

Use this reference while writing `.landfall/dogfood/<run>/report.md`.

## Severity

- `critical`: unsafe mutation, secret exposure, broken release path, or duplicate automation that can publish bad releases.
- `high`: blocks adoption for many repos or produces a materially wrong plan.
- `medium`: slows operators down, needs manual correction, or hides important evidence.
- `low`: copy, naming, output layout, or polish issue.

## Categories

- `classification`: wrong active/app/release-tool/existing-Landfall detection.
- `secret-readiness`: missing, unavailable, or confusing secret metadata.
- `generated-diff`: wrong workflow, duplicate workflow, weak checkout, generic manifest, or non-repo-fit output.
- `operator-ux`: unclear help, surprising defaults, noisy paths, missing examples, or weak summary.
- `cost-context`: LLM policy, prompt context, or artifact behavior does not match repo needs.
- `verification`: missing replay, weak dry-run, no hosted check, or insufficient post-merge evidence.
- `integration-blocker`: external repo state prevents safe rollout.

## Report Shape

Keep each finding compact:

```text
### LF-DOGFOOD-001: title
Severity: high
Category: classification
Evidence: command/path/PR URL
Impact: what would go wrong during real rollout
Action: fixed now / backlog / downstream PR / blocked by missing secret
```
