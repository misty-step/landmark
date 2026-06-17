# Agent Integration Guide

This guide is the cold-start contract for agents integrating Landmark into a
repository or pipeline.

## Discovery

Start with the runtime, not README prose:

```bash
landmark describe --json
```

The document lists supported providers, modes, commands, inputs, output
contracts, preview behavior, schemas, examples, and the failure taxonomy.

stdout carries JSON payloads. stderr carries logs and errors. Pass
`--error-format json` to any command when an agent needs a stable failure
envelope:

```bash
landmark --error-format json run --provider unsupported --dry-run
```

JSON failures include `code`, `stage`, `retryable`, `user_action`, and redacted
`context`.

## Safe First Run

For a zero-secret local preview, run:

```bash
landmark run --provider local --repo-root . --dry-run
```

`--dry-run` computes the release decision, artifact paths, hashes, and
publication plan without writing artifacts or mutating a remote release.

For a GitHub-backed pipeline, keep publication explicit:

```bash
landmark run \
  --provider github \
  --repo-root . \
  --repository owner/repo \
  --release-tag v1.2.3 \
  --github-token "$GH_RELEASE_TOKEN" \
  --publish-release-body
```

Without `--publish-release-body`, the GitHub provider still produces local
evidence and artifacts without mutating the release body.

## Schemas

The checked schema registry is:

- `schemas/landmark-manifest.v1.schema.json` for `.landmark.yml`
- `schemas/synthesis-status.v1.schema.json` for synthesis status output
- `schemas/release-context.v1.schema.json` for deterministic release context packets
- `schemas/release-kit.v1.schema.json` for final-mile release kit plans and producer contracts
- `schemas/replay-result.v1.schema.json` for replay-action evidence
- `schemas/fleet-plan.v1.schema.json` for fleet adoption plans
- `schemas/release-entry.v1.schema.json` for release-note JSON entries
- `schemas/run-evidence.v1.schema.json` for `landmark run` evidence packets
- `schemas/failure-envelope.v1.schema.json` for `--error-format json` stderr

Every schema carries an `$id` and `x-landmark-artifact` value that is checked by
`landmark check-action-contract`.

## Release Kit Boundary

Use the release kit when the release needs more than a version, changelog, and
release notes. The kit is the machine-readable contract between Landmark's
release intelligence and downstream producers.

Landmark should own:

- release facts, classification, audience fit, and importance
- the list of required or recommended final-mile artifacts
- artifact dependencies, acceptance checks, and destination paths
- provenance, approval state, blockers, and waivers
- producer contracts that describe what an adapter must produce

Landmark should not embed bespoke media production, brand design, CMS publishing,
or browser-capture pipelines in the core runtime. Route those through producer
adapters such as local CLIs, browser automation, external services, harness
skills, or human approval. Producers consume release-kit inputs and return
artifact paths, hashes, evidence, and status.

Typical release-kit artifacts include migration guides, docs patches, blog
drafts, essays, announcement copy, social copy, screenshots, images, GIFs, demo
scripts, and demo videos. Text artifacts may be produced directly by Landmark;
rich media and publication-specific outputs should stay adapter-owned.

## Validation Oracle

An agent can validate this guide and the runtime contract with:

```bash
landmark replay-action --scenario agent_native_contracts --format json
```

The scenario parses every checked schema, validates `describe --json`, exercises
the JSON failure envelope, and proves `run --dry-run` against a disposable git
repository without writing release artifacts.
