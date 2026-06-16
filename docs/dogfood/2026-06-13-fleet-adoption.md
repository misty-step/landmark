# Fleet Adoption Dogfood — 2026-06-13

## Scope

Dogfood Landmark against active `phrazzld` and `misty-step` repositories using the Rust fleet workflow.

Evidence directory, ignored by git: `.landmark/dogfood/fleet-2026-06-13/`

## Commands

```bash
target/debug/landmark fleet scan --owner phrazzld --owner misty-step --active-only --output .landmark/dogfood/fleet-2026-06-13/active-scan.json
target/debug/landmark fleet scan --owner phrazzld --owner misty-step --active-only --deep-checks --concurrency 6 --output .landmark/dogfood/fleet-2026-06-13/deep-active-scan-after-fix.json
target/debug/landmark fleet plan --input .landmark/dogfood/fleet-2026-06-13/deep-active-scan-after-fix.json --output-dir .landmark/dogfood/fleet-2026-06-13/deep-plan-after-fix
target/debug/landmark fleet open-prs --dry-run --plan-dir .landmark/dogfood/fleet-2026-06-13/deep-plan-after-fix --output-dir .landmark/dogfood/fleet-2026-06-13/deep-plan-after-fix/prs-v2
```

## Result

- Scanned 75 active repositories.
- Ready after deep checks: `misty-step/bitterblossom`, `phrazzld/glance`.
- Blocked by missing required secrets: 73 repositories.
- Missing `GH_RELEASE_TOKEN`: 73 repositories.
- Missing `OPENROUTER_API_KEY`: 61 repositories.
- `misty-step/bitterblossom` adopted Landmark in synthesis-only mode in PR
  [#853](https://github.com/misty-step/bitterblossom/pull/853), merged as
  `175c9dfd83c015f41c09994b5218fc36860e842e`.
- `phrazzld/glance` added a Landmark manifest and kept its full release
  workflow in PR [#73](https://github.com/phrazzld/glance/pull/73), merged as
  `abdaa93ad4c0a86696b6021ff7f556598c9995b6`.
- Glance dogfood exposed a Landmark no-release summary bug. Landmark fixed it
  in PR [#131](https://github.com/misty-step/landmark/pull/131), released it
  as [v1.23.1](https://github.com/misty-step/landmark/releases/tag/v1.23.1)
  via PR [#132](https://github.com/misty-step/landmark/pull/132), then Glance
  updated to the fixed action pin in PR
  [#74](https://github.com/phrazzld/glance/pull/74), merged as
  `b74081d12772301cfdbe3c82da0d1e549ea7d676`.
- Post-merge verification passed for Landmark v1.23.1 and the Glance release
  workflow. Glance Release run
  [27481472455](https://github.com/phrazzld/glance/actions/runs/27481472455)
  replayed the exact no-release path that previously failed and completed
  successfully.

## 2026-06-15 Rollout-Safety Update

- `fleet scan` and `fleet plan` now carry `repository_kind`,
  `release_surface`, `integration_mode`, `integration_rationale`, and
  mode-scoped `required_secrets`.
- Fleet planning now distinguishes applications, libraries, infrastructure,
  archived repositories, experiments, non-release repositories,
  already-adopted repositories, and no-release-tool repositories before
  recommending rollout.
- Fleet modes now include `local`, `generic-ci`, `github-full`,
  `github-synthesis-only`, `manifest-only`, and `backfill-first`. GitHub secret
  blockers apply only to GitHub integration modes.
- `fleet open-prs` now records branch names, commit messages, rollback guidance,
  disposition, evidence directory, and monitoring status in `open-prs.json`.
  Confirmed rollout requires `--confirm-remote --max-prs 1` and writes an
  `APPLY.md` packet for the one repository being advanced.
- Local, generic CI, backfill-first, and manifest-only plans render manifests
  and receipts without adding GitHub release workflows.

## Findings

### LF-DOGFOOD-001: default scan produces a fully blocked plan
Severity: medium
Category: secret-readiness
Evidence: default scan/plan under `.landmark/dogfood/fleet-2026-06-13/plan/` reported 75 blocked repositories because secret metadata was unavailable.
Impact: first-run operators get no actionable PR plan from the README command.
Action: repo-local skill now requires `--deep-checks` before recommending PRs; product follow-up should make this clearer in `fleet plan` output.

### LF-DOGFOOD-002: Cargo echoed a token-bearing command
Severity: critical
Category: operator-ux
Evidence: running `cargo run --locked -- fleet scan --github-token ...` caused Cargo to echo the full argv.
Impact: operators can accidentally expose token values in terminal logs.
Action: use env fallback and `target/debug/landmark` in dogfood skill; CLI help now documents `GITHUB_TOKEN` fallback for fleet scan.

### LF-DOGFOOD-003: existing Landmark workflow was missed
Severity: high
Category: classification
Evidence: `phrazzld/glance` has `.github/workflows/release.yml` using `misty-step/landmark`, but initial scan reported `existing_landmark: false`.
Impact: Landmark proposed duplicate adoption instead of manifest/upgrade work.
Action: fixed scanner to inspect workflow contents and added replay/unit coverage.

### LF-DOGFOOD-004: manifest-only still generated a workflow
Severity: high
Category: generated-diff
Evidence: `fleet open-prs --dry-run` generated `.github/workflows/landmark-release.yml` for `phrazzld/glance` after it was classified as `manifest-only`.
Impact: adoption PRs could introduce duplicate release-note workflows.
Action: fixed dry-run rendering so `manifest-only` writes only `.landmark.yml` plus `diff.md`.

### LF-DOGFOOD-005: active repository is not the same as active application
Severity: medium
Category: classification
Evidence: fleet scan included configs, docs, splash pages, and other non-application repos in the 75 active repositories.
Impact: full rollout needs a repo-type/app classifier before safe bulk secret provisioning or PR creation.
Action: captured in `skills/landmark-dogfood`; product follow-up should add app classification and allow/deny controls to fleet planning.
Status 2026-06-15: addressed for first-pass rollout by repository kind,
release-surface classification, explicit integration modes, and dogfood skill
updates. Allow/deny controls remain a future ergonomics improvement, not a
safe-rollout blocker.

### LF-DOGFOOD-006: generated manifests are generic
Severity: medium
Category: generated-diff
Evidence: generated product descriptions use “Release notes and changelog automation for owner/repo.”
Impact: release notes will be less contextual than the Landmark vision promises unless each repo is manually edited.
Action: future fleet planning should pull package descriptions, README summaries, repository descriptions, and existing release bodies into manifest inference.

### LF-DOGFOOD-007: local Linux artifact build hung under Docker emulation
Severity: medium
Category: verification
Evidence: `bin/build-linux-action --write` on macOS stayed in the final Landmark compile/link phase for more than ten minutes and did not rewrite `dist/landmark`.
Impact: local Rust changes to the action runtime still need hosted Linux artifact recovery unless the Docker path is made faster or more observable.
Action: use hosted CI artifact upload as the fallback binary builder for this PR; product follow-up should improve local build progress and timeout behavior.

### LF-DOGFOOD-008: downstream clones inside a Rust workspace broke Cargo
Severity: medium
Category: verification
Evidence: cloning downstream repositories under `.landmark/dogfood/...` inside the Landmark checkout caused Cargo to report that the downstream package believed it was in a workspace when it was not.
Impact: dogfood verification can fail for consumer repos for reasons created by Landmark's evidence directory layout.
Action: run downstream clones outside the Landmark repo, such as `/tmp/landmark-dogfood-downstream`; product follow-up should default fleet workdirs outside the producer repo or generate a `.cargo/config.toml`/workspace boundary when nested clones are unavoidable.

### LF-DOGFOOD-009: no-release full-mode run failed final summary
Severity: critical
Category: release-path
Evidence: Glance Release run [27481025693](https://github.com/phrazzld/glance/actions/runs/27481025693) found no semantic-release-worthy commits, then failed in `release-policy summary` because `--attempts-file` and `--context-metadata-file` were passed empty values.
Impact: a healthy no-op release could fail the release workflow after semantic-release correctly decided not to publish a new version.
Action: fixed in Landmark PR [#131](https://github.com/misty-step/landmark/pull/131), released as [v1.23.1](https://github.com/misty-step/landmark/releases/tag/v1.23.1), and verified in Glance Release run [27481472455](https://github.com/phrazzld/glance/actions/runs/27481472455).

### LF-DOGFOOD-010: manual-tag adoption can double-trigger synthesis
Severity: high
Category: generated-diff
Evidence: the generated manual-tag workflow shape included both tag push and `release.published` triggers; Bitterblossom was manually narrowed to `release.published` only before merge.
Impact: generated adoption PRs can spend LLM budget twice and race two release-note synthesis jobs for the same human-created release.
Action: keep Bitterblossom on the single `release.published` trigger; product follow-up should make manual-tag/synthesis-only setup pick exactly one trigger by default.

### LF-DOGFOOD-011: Node 20 action warnings are now time-bound
Severity: medium
Category: verification
Evidence: hosted Landmark and Glance workflows warn that Node 20 JavaScript actions will run on Node 24 by default starting 2026-06-16, three days after this report, and Node 20 will be removed on 2026-09-16.
Impact: Landmark adopters will see noisy warnings now, and untested action dependencies may break as GitHub changes the runtime default.
Action: add a hardening ticket to test Landmark and generated workflows with `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24=true`, then document or update pins before the default changes.

## What Worked Well

- `fleet scan` and `fleet plan` were fast enough across 75 active repos.
- Secret metadata never printed values in scan artifacts.
- The dry-run PR artifact is easy to inspect and can be handed to downstream repo work.
- Replay coverage made it cheap to lock product fixes to the dogfood failure modes.
- SHA-pinned downstream rollout is practical when the generated diff is small:
  Glance moved from v1.23.0 to v1.23.1 with a one-line workflow change.
- The healthcheck/no-release/full-mode path now has real downstream evidence,
  not just local unit coverage.

## Next Rollout Steps

1. Add Node 24 workflow-runtime verification before GitHub switches hosted actions to Node 24 by default on 2026-06-16.
2. Add a Landmark fix so synthesis-only/manual-tag setup generates exactly one trigger, avoiding duplicate LLM spend.
3. Run the updated fleet plan against the active repositories, then choose a
   small first wave by `integration_mode` and repository kind.
4. Provision `GH_RELEASE_TOKEN` and `OPENROUTER_API_KEY` only for repositories
   intentionally entering `github-full` or `github-synthesis-only` mode.
5. Merge one downstream PR at a time, monitor the release run or generic CI
   artifact path named in `open-prs.json`, then continue the rollout.
6. Keep using real downstream release runs as the acceptance oracle for Landmark action changes; local CLI tests are necessary but not sufficient.
