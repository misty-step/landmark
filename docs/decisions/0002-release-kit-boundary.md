# 0002: Landmark owns release kits, not production studios

- **Status:** accepted
- **Date:** 2026-07-08 <!-- First committed date; original ADR had no date. -->
- **Primitives:** release-intelligence core, release kit, producer adapters
- **Powder:** none recorded

## Context
Release work extends beyond notes and versioning to migration guides, docs
patches, launch copy, screenshots, images, GIFs, and demo videos. Owning every
creative and publishing workflow would pull media tooling, design systems,
CMS APIs, and human approval queues into the Rust runtime; ignoring those
outputs would leave the final-mile release job underserved.

## Decision
Landmark owns release intelligence and the typed release-kit contract: facts,
audiences, artifact plans, dependencies, acceptance checks, destinations,
status, provenance, approvals, blockers, and waivers. Bespoke media production,
brand design, CMS publishing, and long-running creative pipelines belong behind
explicit local, browser, service, harness, or human producer adapters, not in
the core runtime.

## Consequences
The [Rust CLI](../../crates/landmark/) remains the source of truth for release
context, plans, schemas, and evidence. It may produce simple text and data
artifacts close to release truth, including notes, feeds, docs patch suggestions,
and announcement drafts; rich producers remain replaceable. Completion can
require planned artifacts to be produced, verified, waived, or explicitly
blocked. Extend typed artifacts and run evidence before adding a media-specific
renderer.

Revisit if: a proposed output can stay close to release truth as a simple text
or data artifact rather than requiring a bespoke production workflow.
