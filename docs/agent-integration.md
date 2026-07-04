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

`--dry-run` computes the release decision, artifact paths, hashes, publication
plan, and embedded `release_kit` without writing artifacts or mutating a remote
release. A normal local run writes the same release-kit artifact to
`.landmark/run/release-kit.json` and records its schema and hash in
`.landmark/run/evidence.json`. The evidence packet also carries the
deterministic version decision and changed-file list for producer adapters that
need text evidence without re-reading the repository. Rust crate repositories
also record `cargo-semver-checks` public API evidence against the previous
release tag when the provider can run; skips and tool failures are explicit
evidence statuses, not silent omissions.

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

For fleet-wide Misty Step adoption, use
[`docs/fleet-integration-playbook.md`](fleet-integration-playbook.md) before
opening consumer-repo branches. It defines the standard manifest, when to choose
full mode versus synthesis-only, and the required verification steps.

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

The remote-service feed adapter is:

```bash
landmark notify-release-feed \
  --evidence-file .landmark/run/evidence.json \
  --release-kit-file .landmark/run/release-kit.json
```

It reads `RELEASE_FEED_URL` and `RELEASE_FEED_SECRET` from the environment
(`LANDMARK_RELEASE_FEED_*` and `RELEASE_KIT_FEED_*` aliases are also accepted),
enriches the kit with produced text-floor artifacts, and POSTs the full
`landmark.release-kit.v1` JSON body to `POST /v1/events`. The request signature
matches Landmark's webhook scheme: `X-Signature-256: sha256=<hmac>` over the raw
body. Missing receiver config is a clean skip.

Typical release-kit artifacts include migration guides, docs patches, blog
drafts, essays, announcement copy, social copy, screenshots, images, GIFs, demo
scripts, and demo videos. Text artifacts may be produced directly by Landmark;
rich media and publication-specific outputs should stay adapter-owned.

## Glossary

| Term | Definition |
|------|-----------|
| synthesis | LLM step that converts a technical changelog into user-facing "What's New" notes |
| full mode | `semantic-release` (compatibility path) + synthesis pipeline |
| synthesis-only mode | Skip `semantic-release`; synthesize notes for an existing tag |
| floating tag | Major-version alias tag (e.g. `v1`) that points to the latest release; Landmark's own release workflow only repoints it once that release's binaries and checksums are confirmed live |
| changelog-source | Where synthesis pulls its input: `auto`, `changelog`, `release-body`, `prs` |
| audience | Built-in prompt variant: `general`, `developer`, `end-user`, `enterprise` |
| synthesis-required | If `true`, fail the action when synthesis or publication policy fails |
| preflight check | Pre-`semantic-release` validation: tag history integrity, config detection (`landmark preflight-tags`) |
| version decision | The output of the shared version-decision engine (`crates/landmark/src/version_decision.rs`): the final bump, commit-derived floor, public API evidence bump, reconciliation status, decisive signals, unknown commits, and any typed waiver state |
| release kit | Typed packet of release facts, recommended outputs, artifact status, provenance, approvals, and producer contracts |
| producer adapter | Explicit local, browser, service, harness, or human boundary that turns release-kit inputs into a rich artifact such as a video, GIF, image, essay, docs patch, or blog draft |
| final-mile artifact | Any output needed to ship the release beyond versioning: docs updates, migration guide, demo script, video, GIF, image, blog post, announcement copy, feed item, or social copy |

## Validation Oracle

An agent can validate this guide and the runtime contract with:

```bash
landmark replay-action --scenario agent_native_contracts --format json
```

The scenario parses every checked schema, validates `describe --json`, exercises
the JSON failure envelope, and proves `run --dry-run` against a disposable git
repository without writing release artifacts.
