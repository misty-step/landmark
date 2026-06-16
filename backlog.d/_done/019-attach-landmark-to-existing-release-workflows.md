# Attach Landmark to existing release workflows without duplicating release jobs

Priority: P0 · Status: done · Estimate: L

## Goal
Make fleet rollout patch existing release-please, changesets, and semantic-release workflows so Landmark runs after the owning release job instead of creating a competing release workflow.

## Oracle
- [x] `fleet open-prs` for release-please repos adds or updates a synthesis job that depends on the existing release-please job and does not add a second release-please job.
- [x] `fleet open-prs` for changesets repos adds or updates a synthesis job that depends on the existing changesets job and does not add a second changesets publish job.
- [x] `fleet open-prs` for semantic-release repos either replaces the existing release job with Landmark full mode or emits a blocked plan requiring explicit operator choice.
- [x] Replay fixtures include repos with existing `release.yml` files and prove no duplicate release jobs are generated.
- [x] Generated workflows follow common YAML lint policies, including single-quoted string inputs where quotes are required.

## Notes
- Evidence: the June 15, 2026 org rollout dry-run generated standalone `Release` workflows for `misty-step/linejam`, `misty-step/chrondle`, `misty-step/thinktank`, and `misty-step/vibe-machine`, which would duplicate existing release jobs.
- Safe rollout currently limited to manual-tag/synthesis-only repos until this is fixed.

## Delivered
- Added fleet workflow metadata and generated workflow patch records so release-please and Changesets repositories update their existing workflow paths instead of adding `.github/workflows/landmark-release.yml`.
- Workflow patches now preserve the existing workflow body and insert or replace `jobs.synthesize`; token-like workflow bodies are blocked instead of serialized into scan/plan artifacts.
- Existing semantic-release workflows now produce a blocked plan requiring explicit operator choice before Landmark full-mode replacement.
- Extended the fleet adoption replay fixture to prove release-please and Changesets dry-runs write existing workflow paths, generate valid YAML, keep one owning release job, and skip existing semantic-release workflows.
- Verified with `cargo test -p landmark fleet_plan_ -- --nocapture` and `cargo run --locked -p landmark -- replay-action --scenario fleet_adoption_planner --format json`.
