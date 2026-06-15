# Agent Integration Guide

This guide is the cold-start contract for agents integrating Landfall into a
repository or pipeline.

## Discovery

Start with the runtime, not README prose:

```bash
landfall describe --json
```

The document lists supported providers, modes, commands, inputs, output
contracts, preview behavior, schemas, examples, and the failure taxonomy.

stdout carries JSON payloads. stderr carries logs and errors. Pass
`--error-format json` to any command when an agent needs a stable failure
envelope:

```bash
landfall --error-format json run --provider unsupported --dry-run
```

JSON failures include `code`, `stage`, `retryable`, `user_action`, and redacted
`context`.

## Safe First Run

For a zero-secret local preview, run:

```bash
landfall run --provider local --repo-root . --dry-run
```

`--dry-run` computes the release decision, artifact paths, hashes, and
publication plan without writing artifacts or mutating a remote release.

For a GitHub-backed pipeline, keep publication explicit:

```bash
landfall run \
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

- `schemas/landfall-manifest.v1.schema.json` for `.landfall.yml`
- `schemas/synthesis-status.v1.schema.json` for synthesis status output
- `schemas/replay-result.v1.schema.json` for replay-action evidence
- `schemas/fleet-plan.v1.schema.json` for fleet adoption plans
- `schemas/release-entry.v1.schema.json` for release-note JSON entries
- `schemas/run-evidence.v1.schema.json` for `landfall run` evidence packets
- `schemas/failure-envelope.v1.schema.json` for `--error-format json` stderr

Every schema carries an `$id` and `x-landfall-artifact` value that is checked by
`landfall check-action-contract`.

## Validation Oracle

An agent can validate this guide and the runtime contract with:

```bash
landfall replay-action --scenario agent_native_contracts --format json
```

The scenario parses every checked schema, validates `describe --json`, exercises
the JSON failure envelope, and proves `run --dry-run` against a disposable git
repository without writing release artifacts.
