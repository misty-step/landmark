# Build contextual release intelligence without runaway cost

Priority: P1 · Status: pending · Estimate: L

## Goal
Make Landfall produce highly contextual internal changelogs and public release
notes while spending LLM budget only where it changes the output.

## Oracle
- [ ] Landfall builds a deterministic release context packet from commits, tags, changed files, manifests, PR metadata when available, docs, package metadata, and prior releases.
- [ ] Conventional commits produce a useful technical changelog without an LLM.
- [ ] LLM calls are gated by release significance, audience, missing context, and budget policy.
- [ ] The synthesis status explains why a model was skipped, used, escalated, or degraded.
- [ ] Internal technical changelog and public release notes are separate artifacts with different audiences and schemas.
- [ ] Replay fixtures prove cheap, balanced, rich, off, and provider-failure paths.

## Children
1. Define the release context packet schema and deterministic builders.
2. Separate internal technical changelog output from public-facing release notes.
3. Add significance scoring that can skip or downshift LLM usage for routine releases.
4. Cache or reuse context summaries across backfills and repeated preview runs.
5. Add explainable cost/status metadata to the evidence packet.
6. Add fixtures for conventional-commit-only, PR-rich, docs-only, breaking-change, security, migration, and dependency releases.

## Notes
- Evidence: Landfall already has model policy, cost dry-run, classification, and context-source reporting.
- Evidence: the user wants contextual notes without paying an LLM tax on every changelog.
- Why: contextual release intelligence is the enduring value-add once provider portability is solved.
