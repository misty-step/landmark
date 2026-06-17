# Build the release-kit artifact graph

Priority: P1 · Status: pending · Estimate: L

## Goal
Make Landmark emit a typed release-kit plan that recommends and tracks final-mile
release artifacts beyond changelogs, release notes, and version numbers while
keeping rich production behind explicit producer adapters.

## Oracle
- [ ] `landmark run --dry-run` can write or print a `release-kit` artifact that
      validates against `schemas/release-kit.v1.schema.json`.
- [ ] The kit distinguishes Landmark-owned outputs from adapter-owned outputs
      such as docs patches, blog drafts, images, GIFs, and demo videos.
- [ ] Each planned artifact includes audience, owner, status, acceptance checks,
      provenance, and approval/blocker state.
- [ ] Producer contracts name their adapter kind, inputs, outputs, mutation
      policy, command or handoff path, and evidence path.
- [ ] Replay coverage proves a high-importance release plans richer artifacts
      while a low-importance/internal release keeps the kit small.

## Notes
- Landmark owns release truth, artifact planning, provenance, approvals, and
  evidence. It should not become a media renderer, brand studio, or CMS engine.
- Start from the current release context packet and run evidence model. Extend
  typed artifacts before adding any producer-specific integration.
- Producer adapters may be local CLIs, browser automation, remote services,
  harness skills, or human approval lanes.
