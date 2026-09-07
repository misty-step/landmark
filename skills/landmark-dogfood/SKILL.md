---
name: landmark-dogfood
description: Dogfood Landmark adoption and release-note automation for explicitly selected GitHub repositories; evaluate adoption friction and return release-pipeline evidence and scoped findings.
---

# Landmark Dogfood

Use Landmark against real repositories before claiming adoption quality. Work
only within the current operator request. Return an evidence packet, approved
downstream PRs where in scope, and unresolved findings with source pointers
and proposal status. Linear owns selected current non-R90 work; do not create
tickets automatically or maintain a second fleet backlog. R90 continues to
use Habitat.

## Workflow

1. Start from the local Landmark checkout and read `AGENTS.md`. Select owners
   and repositories from the current request, not a historical dogfood report.
2. Stage evidence under `.landmark/dogfood/<date-or-run>/`. Keep downstream
   clones outside this workspace to avoid accidental Cargo workspace membership.
3. Build/run the Rust binary locally with `cargo run --locked -- ...` or `target/debug/landmark`. The action itself bootstrap-downloads a published release binary; there is no checked-in binary to run locally.
4. Run a read-only fleet scan first:

```bash
target/debug/landmark fleet scan \
  --owner <requested-owner> \
  --active-only \
  --output .landmark/dogfood/<run>/active-scan.json
```

5. Run `--deep-checks` before opening or recommending PRs; default scans intentionally mark secret metadata unavailable.
6. Generate `fleet plan`, then `fleet open-prs --dry-run`; inspect the rendered files before mutating downstream repos.
7. Use the plan's `repository_kind`, `release_surface`, `integration_mode`, and `integration_rationale` fields as the first-pass rollout map. Separate applications, libraries, infrastructure, archived repos, experiments, non-release repos, no-release-tool repos, and already-adopted repos. Do not treat every active repo as an app.
8. For each candidate, inspect existing workflows, releases, tags, `AGENTS.md`, and release notes. Existing Landmark workflows should become manifest/upgrade work, not duplicate workflow installation.
9. Apply downstream integration only when the generated diff is repo-fit and required secrets are present or intentionally provisioned. Use `fleet open-prs --confirm-remote --max-prs 1` only after the dry-run receipt is inspected; never provision secrets for `local`, `generic-ci`, `manifest-only`, `backfill-first`, or skipped non-release modes just to make a GitHub plan look ready.
10. Roll out one repository at a time. After a downstream PR merges, monitor the release run or local/generic CI artifact path named in the receipt before continuing the fleet.
11. Record friction immediately. Use [references/finding-taxonomy.md](references/finding-taxonomy.md) for categories and severity.
12. Fix Landmark itself only when the finding is within the current request;
    otherwise return the proposed fix and evidence for selection. Keep a
    regression case when it protects the observed failure, not merely to
    accompany a report.

## Evidence

Every run should leave:

- `active-scan.json` and, when used, `deep-active-scan.json`
- `plan/plan.json` and `plan/README.md`
- dry-run PR artifacts under `plan/prs/`
- `plan/prs/open-prs.json` receipts with branch, commit message, rollback, disposition, evidence directory, monitoring status, and `APPLY.md` for confirmed one-repo rollout packets
- a short report listing observed adoption outcomes, friction, in-scope fixes,
  and selected unresolved proposals; not a standing rollout order
- exact commands and hosted PR/check URLs for downstream integrations

Local ignored output is staging, not a shared retention guarantee. Keep raw,
large, or sensitive packets in approved retained artifact storage. Share
sanitized summaries and exact retained locators with run/revision identity.
Git keeps reusable procedures, contracts, and privacy-reviewed fixtures.

## Safety

- Prefer env-based token discovery. Avoid passing secrets as CLI arguments, especially through `cargo run`, because Cargo echoes arguments.
- Never print secret values or commit raw dogfood artifacts. Redact shared
  summaries and use access-controlled retention for the underlying proof.
- Do not set or overwrite repository secrets in bulk until the target set is narrowed to real applications and the token/value source is explicit.
- Do not open duplicate release workflows for repos that already invoke `misty-step/landmark`.
- Do not add GitHub workflows for `local`, `generic-ci`, `manifest-only`, or `backfill-first` plans unless repo-specific review deliberately overrides the generated receipt.
- Treat generated `open-prs` output as a proposal, not authority. The lead agent owns repo-fit.
