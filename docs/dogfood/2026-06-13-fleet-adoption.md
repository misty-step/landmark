# Fleet Adoption Dogfood — 2026-06-13

## Scope

Dogfood Landfall against active `phrazzld` and `misty-step` repositories using the Rust fleet workflow.

Evidence directory, ignored by git: `.landfall/dogfood/fleet-2026-06-13/`

## Commands

```bash
target/debug/landfall fleet scan --owner phrazzld --owner misty-step --active-only --output .landfall/dogfood/fleet-2026-06-13/active-scan.json
target/debug/landfall fleet scan --owner phrazzld --owner misty-step --active-only --deep-checks --concurrency 6 --output .landfall/dogfood/fleet-2026-06-13/deep-active-scan-after-fix.json
target/debug/landfall fleet plan --input .landfall/dogfood/fleet-2026-06-13/deep-active-scan-after-fix.json --output-dir .landfall/dogfood/fleet-2026-06-13/deep-plan-after-fix
target/debug/landfall fleet open-prs --dry-run --plan-dir .landfall/dogfood/fleet-2026-06-13/deep-plan-after-fix --output-dir .landfall/dogfood/fleet-2026-06-13/deep-plan-after-fix/prs-v2
```

## Result

- Scanned 75 active repositories.
- Ready after deep checks: `misty-step/bitterblossom`, `phrazzld/glance`.
- Blocked by missing required secrets: 73 repositories.
- Missing `GH_RELEASE_TOKEN`: 73 repositories.
- Missing `OPENROUTER_API_KEY`: 61 repositories.
- `phrazzld/glance` already invokes Landfall in `.github/workflows/release.yml`; after the scanner fix it is classified as `manifest-only`.
- `misty-step/bitterblossom` has Landfall-style release bodies but no repo workflow or manifest detected; it remains a real adoption candidate.

## Findings

### LF-DOGFOOD-001: default scan produces a fully blocked plan
Severity: medium
Category: secret-readiness
Evidence: default scan/plan under `.landfall/dogfood/fleet-2026-06-13/plan/` reported 75 blocked repositories because secret metadata was unavailable.
Impact: first-run operators get no actionable PR plan from the README command.
Action: repo-local skill now requires `--deep-checks` before recommending PRs; product follow-up should make this clearer in `fleet plan` output.

### LF-DOGFOOD-002: Cargo echoed a token-bearing command
Severity: critical
Category: operator-ux
Evidence: running `cargo run --locked -- fleet scan --github-token ...` caused Cargo to echo the full argv.
Impact: operators can accidentally expose token values in terminal logs.
Action: use env fallback and `target/debug/landfall` in dogfood skill; CLI help now documents `GITHUB_TOKEN` fallback for fleet scan.

### LF-DOGFOOD-003: existing Landfall workflow was missed
Severity: high
Category: classification
Evidence: `phrazzld/glance` has `.github/workflows/release.yml` using `misty-step/landfall`, but initial scan reported `existing_landfall: false`.
Impact: Landfall proposed duplicate adoption instead of manifest/upgrade work.
Action: fixed scanner to inspect workflow contents and added replay/unit coverage.

### LF-DOGFOOD-004: manifest-only still generated a workflow
Severity: high
Category: generated-diff
Evidence: `fleet open-prs --dry-run` generated `.github/workflows/landfall-release.yml` for `phrazzld/glance` after it was classified as `manifest-only`.
Impact: adoption PRs could introduce duplicate release-note workflows.
Action: fixed dry-run rendering so `manifest-only` writes only `.landfall.yml` plus `diff.md`.

### LF-DOGFOOD-005: active repository is not the same as active application
Severity: medium
Category: classification
Evidence: fleet scan included configs, docs, splash pages, and other non-application repos in the 75 active repositories.
Impact: full rollout needs a repo-type/app classifier before safe bulk secret provisioning or PR creation.
Action: captured in `skills/landfall-dogfood`; product follow-up should add app classification and allow/deny controls to fleet planning.

### LF-DOGFOOD-006: generated manifests are generic
Severity: medium
Category: generated-diff
Evidence: generated product descriptions use “Release notes and changelog automation for owner/repo.”
Impact: release notes will be less contextual than the Landfall vision promises unless each repo is manually edited.
Action: future fleet planning should pull package descriptions, README summaries, repository descriptions, and existing release bodies into manifest inference.

### LF-DOGFOOD-007: local Linux artifact build hung under Docker emulation
Severity: medium
Category: verification
Evidence: `bin/build-linux-action --write` on macOS stayed in the final Landfall compile/link phase for more than ten minutes and did not rewrite `dist/landfall`.
Impact: local Rust changes to the action runtime still need hosted Linux artifact recovery unless the Docker path is made faster or more observable.
Action: use hosted CI artifact upload as the fallback binary builder for this PR; product follow-up should improve local build progress and timeout behavior.

## What Worked Well

- `fleet scan` and `fleet plan` were fast enough across 75 active repos.
- Secret metadata never printed values in scan artifacts.
- The dry-run PR artifact is easy to inspect and can be handed to downstream repo work.
- Replay coverage made it cheap to lock product fixes to the dogfood failure modes.

## Next Rollout Steps

1. Provision `GH_RELEASE_TOKEN` and `OPENROUTER_API_KEY` only after narrowing the target set to real applications.
2. Open a manifest-only PR to `phrazzld/glance` after reviewing whether its pinned Landfall workflow should also move to `@v1`.
3. Open an adoption PR to `misty-step/bitterblossom` with manifest plus synthesis-only workflow.
4. Add fleet app classification before attempting bulk adoption across the remaining 73 repositories.
