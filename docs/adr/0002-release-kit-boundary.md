# ADR 0002: Landmark Owns Release Kits, Not Production Studios

## Status
Accepted

## Context
Landmark already owns version decisions, technical changelogs, public release
notes, artifact writing, feeds, notifications, replay evidence, and fleet
adoption planning. Final-mile release work is broader than those outputs:
important releases may also need migration guides, docs patches, blog posts,
essays, launch copy, screenshots, images, GIFs, and produced demo videos.

That breadth creates a product-boundary risk. If Landmark tries to directly
own every creative and publishing workflow, the Rust runtime becomes a shallow
orchestration pile around browser capture, ffmpeg, design systems, CMS APIs,
brand guidelines, and human approval queues. If Landmark ignores those outputs,
it under-serves the actual release job: helping the right audience understand
and adopt what shipped.

## Decision
Landmark owns the release-intelligence core and the release-kit contract:

- release facts, source context, classification, and audience fit
- recommended final-mile artifacts and why each artifact is needed
- artifact dependencies, acceptance checks, destinations, and status
- provenance, approvals, blockers, and waivers
- producer contracts for outputs Landmark should not directly render

Landmark may directly produce simple/default text and data artifacts that stay
close to release truth: technical changelogs, public release notes, migration
notes, markdown/plaintext/HTML/JSON entries, feeds, documentation patch
suggestions, and announcement/blog drafts.

Landmark does not directly own bespoke media production, brand design, CMS
publishing, or long-running creative pipelines. Demo videos, GIFs, images,
screenshots, blog publication, and highly branded assets belong behind explicit
producer adapters: local CLIs, browser automation, external services, harness
skills, or human production/approval lanes.

The runtime boundary is:

```text
release facts -> release intelligence -> release kit -> producers -> evidence
```

## Consequences
- The Rust CLI remains the source of truth for release context, plans,
  schemas, and evidence.
- Rich producers are replaceable. A repo can use Remotion, ffmpeg, a browser
  harness, a design service, a CMS adapter, or a human without changing
  Landmark's core release model.
- A release is not "complete" merely because notes were generated; completion
  can require planned artifacts to be produced, verified, waived, or explicitly
  blocked.
- The schema registry needs a `release-kit` artifact so agents and adapters can
  coordinate without scraping prose.
- Future implementation should extend typed artifacts and run evidence before
  adding any media-specific renderer.

## Rejected
- **Make Landmark a production studio.** This would centralize every output but
  would import fragile media, design, CMS, and approval concerns into the core.
- **Keep Landmark limited to notes and versioning.** This preserves simplicity
  but misses the real final-mile release job and leaves agents without a
  contract for richer launch artifacts.
- **One plugin per artifact type in core.** This looks modular but still forces
  the core runtime to own producer lifecycle and domain-specific quality bars.
