---
name: landfall-dogfood
description: Dogfood Landfall fleet adoption and release-note automation across GitHub repositories. Use when asked to integrate Landfall into personal or organization repos, run fleet scan/plan/open-prs, evaluate adoption friction, capture release-pipeline evidence, or turn dogfood findings into Landfall fixes/backlog.
---

# Landfall Dogfood

Use Landfall against real repositories before claiming adoption quality. The output is not just a plan: it is an evidence packet, concrete downstream PRs where safe, and upstream fixes or backlog where the dogfood loop exposes product gaps.

## Workflow

1. Start from the local Landfall checkout and read `AGENTS.md`.
2. Create an evidence directory under `.landfall/dogfood/<date-or-run>/`.
3. Build/run the Rust binary locally; on macOS use `cargo run --locked -- ...` or `target/debug/landfall`, not `dist/landfall` because the checked-in action binary is Linux-only.
4. Run a read-only fleet scan first:

```bash
target/debug/landfall fleet scan \
  --owner phrazzld \
  --owner misty-step \
  --active-only \
  --output .landfall/dogfood/<run>/active-scan.json
```

5. Run `--deep-checks` before opening or recommending PRs; default scans intentionally mark secret metadata unavailable.
6. Generate `fleet plan`, then `fleet open-prs --dry-run`; inspect the rendered files before mutating downstream repos.
7. Use the plan's `repository_kind`, `release_surface`, `integration_mode`, and `integration_rationale` fields as the first-pass rollout map. Separate applications, libraries, infrastructure, archived repos, experiments, non-release repos, no-release-tool repos, and already-adopted repos. Do not treat every active repo as an app.
8. For each candidate, inspect existing workflows, releases, tags, `AGENTS.md`, and release notes. Existing Landfall workflows should become manifest/upgrade work, not duplicate workflow installation.
9. Apply downstream integration only when the generated diff is repo-fit and required secrets are present or intentionally provisioned. Use `fleet open-prs --confirm-remote --max-prs 1` only after the dry-run receipt is inspected; never provision secrets for `local`, `generic-ci`, `manifest-only`, `backfill-first`, or skipped non-release modes just to make a GitHub plan look ready.
10. Roll out one repository at a time. After a downstream PR merges, monitor the release run or local/generic CI artifact path named in the receipt before continuing the fleet.
11. Record friction immediately. Use [references/finding-taxonomy.md](references/finding-taxonomy.md) for categories and severity.
12. Fix Landfall itself when the product made the dogfood run unsafe, confusing, or wrong. Add replay coverage for each fixed failure mode.

## Evidence

Every run should leave:

- `active-scan.json` and, when used, `deep-active-scan.json`
- `plan/plan.json` and `plan/README.md`
- dry-run PR artifacts under `plan/prs/`
- `plan/prs/open-prs.json` receipts with branch, commit message, rollback, disposition, evidence directory, monitoring status, and `APPLY.md` for confirmed one-repo rollout packets
- a short `report.md` listing adopted repos, blocked repos, friction, fixes made, and next rollout steps
- exact commands and hosted PR/check URLs for downstream integrations

## Safety

- Prefer env-based token discovery. Avoid passing secrets as CLI arguments, especially through `cargo run`, because Cargo echoes arguments.
- Never print secret values. Search dogfood artifacts for token-like strings before committing.
- Do not set or overwrite repository secrets in bulk until the target set is narrowed to real applications and the token/value source is explicit.
- Do not open duplicate release workflows for repos that already invoke `misty-step/landmark`.
- Do not add GitHub workflows for `local`, `generic-ci`, `manifest-only`, or `backfill-first` plans unless repo-specific review deliberately overrides the generated receipt.
- Treat generated `open-prs` output as a proposal, not authority. The lead agent owns repo-fit.
