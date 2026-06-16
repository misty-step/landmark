# Build a fleet adoption planner

Priority: P0 · Status: done · Estimate: XL

## Goal
Let Landmark scan personal and Mistystep GitHub repositories, classify adoption readiness, and open safe per-repo installation pull requests with minimal manual setup.

## Oracle
- [x] `dist/landmark fleet scan --owner phrazzld --owner misty-step --output .landmark/fleet.json` lists repositories, activity, release tooling, default branch, tag format, package topology, existing workflows, and required secret status without mutating remote state.
- [x] `dist/landmark fleet plan --input .landmark/fleet.json --output-dir .landmark/fleet-plan` emits a ranked adoption plan with skip reasons, risk flags, and recommended Landmark mode for each active repo.
- [x] `dist/landmark fleet open-prs --dry-run` renders per-repo workflow and manifest diffs without pushing branches.
- [x] A replay or fixture test covers at least semantic-release, release-please, changesets, manual-tag, no-release-tool, archived, private, and branch-protected repository cases.
- [x] The command never prints secret values and clearly reports missing token scopes or unavailable secret metadata.

## Children
1. Add read-only GitHub inventory for repo metadata, recent activity, default branch, release files, tags, workflows, and package signals.
2. Extend setup diagnosis into a fleet classification model: full mode, synthesis-only mode, backfill first, manifest only, blocked, skipped.
3. Add secret and permission readiness checks that work for org repos and degrade honestly for personal repos where GitHub APIs hide details.
4. Generate per-repo adoption branches or PR plans with `.landmark.yml`, workflow files, artifact paths, and migration notes.
5. Add batching controls: active-only, dry-run, max PRs, owner filters, allow/deny lists, and existing-release-tool collision handling.
6. Produce a fleet report that becomes the operator dashboard for phased rollout across personal and Mistystep repositories.

## Notes
- Evidence: `gh repo list phrazzld --limit 200` returned 153 repositories and `gh repo list misty-step --limit 200` returned 73, making manual adoption too expensive and error-prone.
- Evidence: current `landmark setup` can analyze one checkout and emit candidate workflows, but there is no cross-repo inventory, PR generation, or secret readiness loop.
- Why: the adoption critic ranked org-scale rollout highest because it creates real-world usage data and makes every later synthesis/cost feature more grounded.

## Closure
- Delivered `fleet scan`, `fleet plan`, and dry-run `fleet open-prs` as Rust CLI subcommands.
- `fleet scan` is read-only and records repository activity, default branch, release tooling, tag format, package topology, workflow names, branch-protection availability, and required secret metadata without printing secret values.
- Default scans use bounded concurrency and shallow metadata so the 226-repo personal + Mistystep fleet is tractable; `--deep-checks` opts into branch-protection and Actions secret-name probes for smaller batches.
- `fleet plan` ranks repositories into `full`, `synthesis-only`, `manifest-only`, `backfill-first`, `blocked`, and `skipped` adoption lanes with risk flags, missing secrets, unavailable metadata, skip reasons, migration notes, and a generated manifest. Missing or unavailable required secret metadata blocks rollout readiness.
- `fleet open-prs --dry-run` renders per-repo `.landmark.yml`, workflow, and `diff.md` artifacts under `.landmark/fleet-plan/prs/`; non-dry-run mutation is intentionally refused until a later PR-opening slice adds hosted safeguards.
- Replay evidence: `cargo run --locked -- replay-action --evidence-dir .landmark/replay-009-fleet --scenario fleet_adoption_planner` wrote `.landmark/replay-009-fleet/replay-result.json` with semantic-release, release-please, changesets, manual-tag, no-release-tool, archived, private, and branch-protected fixture coverage.
