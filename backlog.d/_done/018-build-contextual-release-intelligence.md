# Build contextual release intelligence without runaway cost

Priority: P1 · Status: done · Estimate: L

## Goal
Make Landmark produce highly contextual internal changelogs and public release
notes while spending LLM budget only where it changes the output.

## Oracle
- [x] Landmark builds a deterministic release context packet from commits, tags, changed files, manifests, PR metadata when available, docs, package metadata, and prior releases.
- [x] Conventional commits produce a useful technical changelog without an LLM.
- [x] LLM calls are gated by release significance, audience, missing context, and budget policy.
- [x] The synthesis status explains why a model was skipped, used, escalated, or degraded.
- [x] Internal technical changelog and public release notes are separate artifacts with different audiences and schemas.
- [x] Replay fixtures prove cheap, balanced, rich, off, and provider-failure paths.

## Children
1. Define the release context packet schema and deterministic builders.
2. Separate internal technical changelog output from public-facing release notes.
3. Add significance scoring that can skip or downshift LLM usage for routine releases.
4. Cache or reuse context summaries across backfills and repeated preview runs.
5. Add explainable cost/status metadata to the evidence packet.
6. Add fixtures for conventional-commit-only, PR-rich, docs-only, breaking-change, security, migration, and dependency releases.

## Notes
- Evidence: Landmark already has model policy, cost dry-run, classification, and context-source reporting.
- Evidence: the user wants contextual notes without paying an LLM tax on every changelog.
- Why: contextual release intelligence is the enduring value-add once provider portability is solved.

## Delivered
- Added `schemas/release-context.v1.schema.json` and registered it in the schema descriptor list.
- Extended synthesis context metadata with deterministic commits, tags, changed files, manifest summary, docs, package metadata, prior release headings, optional PR/release-body metadata, classification, cost, and explicit decision status.
- Recorded separate internal technical changelog and public release-note audiences/schemas in run evidence without making the new fields required in the v1 schema.
- Extended replay coverage for balanced skip, cheap use, balanced rich escalation, direct rich use, off policy skip, fallback, provider failure, and local run artifact audience/schema separation.
- Verified with `cargo run --locked -p landmark -- replay-action --scenario synthesis_cost_policy --scenario local_provider_run --scenario agent_native_contracts --evidence-dir .landmark/replay-018-context`, `cargo run --locked -p landmark -- check-action-contract`, and `bin/gate`.
- Reviewed with Pi/Gemini and OpenCode/DeepSeek fresh-context adversarial lanes; both returned no blocking findings. Advisory: deterministic context collection uses fixed bounds and may later deserve manifest configuration.
