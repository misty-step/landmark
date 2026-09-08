---
name: landmark-dogfood
description: Dogfood Landmark adoption and release-note automation for explicitly selected GitHub repositories; evaluate adoption friction and return release-pipeline evidence and scoped findings.
---

# Landmark Dogfood

Use Landmark against real repositories to assess adoption friction and release
quality. Follow the [fleet integration playbook](../../docs/fleet-integration-playbook.md)
for commands, integration modes, credential handling, and retained evidence.
Use `setup` for one repository; use the fleet planner for a selected group.

## Local entrypoint

From the Landmark checkout, use `cargo run --locked -p landmark -- ...` or
`target/debug/landmark`. The GitHub Action downloads a published binary.

## Integration hazards

- Keep downstream clones outside this Cargo workspace.
- Inspect existing release ownership; upgrade an existing Landmark integration
  instead of installing a duplicate workflow.
- Run deep checks before recommending GitHub adoption. Default scans do not
  establish secret readiness, and artifact-only modes do not need release tokens.
- Inspect `fleet open-prs --dry-run` before approved remote changes. Apply one
  repository at a time and verify its actual release or artifact path before
  continuing; opening a PR alone is not adoption proof.
- Keep tokens out of command arguments and shared output. Retain sanitized
  evidence with the source revision; ignored local output is only staging.

## Findings

Record observed behavior, impact, exact commands, PR/check URLs, and remaining
uncertainty using the [finding taxonomy](references/finding-taxonomy.md).
Distinguish verified fixes from proposed work. Follow the repository's work
authority rules rather than turning the report into another rollout queue.
