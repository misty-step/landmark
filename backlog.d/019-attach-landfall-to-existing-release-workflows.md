# Attach Landfall to existing release workflows without duplicating release jobs

Priority: P0 · Status: pending · Estimate: L

## Goal
Make fleet rollout patch existing release-please, changesets, and semantic-release workflows so Landfall runs after the owning release job instead of creating a competing release workflow.

## Oracle
- [ ] `fleet open-prs` for release-please repos adds or updates a synthesis job that depends on the existing release-please job and does not add a second release-please job.
- [ ] `fleet open-prs` for changesets repos adds or updates a synthesis job that depends on the existing changesets job and does not add a second changesets publish job.
- [ ] `fleet open-prs` for semantic-release repos either replaces the existing release job with Landfall full mode or emits a blocked plan requiring explicit operator choice.
- [ ] Replay fixtures include repos with existing `release.yml` files and prove no duplicate release jobs are generated.
- [ ] Generated workflows follow common YAML lint policies, including single-quoted string inputs where quotes are required.

## Notes
- Evidence: the June 15, 2026 org rollout dry-run generated standalone `Release` workflows for `misty-step/linejam`, `misty-step/chrondle`, `misty-step/thinktank`, and `misty-step/vibe-machine`, which would duplicate existing release jobs.
- Safe rollout currently limited to manual-tag/synthesis-only repos until this is fixed.
