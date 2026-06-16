# ADR 0001: Rust Runtime Boundary

Status: accepted
Date: 2026-06-11

## Context

Landmark is a composite GitHub Action that intentionally delegates version
analysis, changelog generation, and GitHub Release creation to
`semantic-release`. Before this decision, Landmark-owned behavior lived in
Python scripts and shell fragments: synthesis, release-body updates, artifact
rendering, RSS/webhook/Slack publication, policy evaluation, failure issue
lifecycle, metadata checks, and replay verification.

The repo doctrine is Rust by default, and consumer action runs should not
bootstrap Python or pip for Landmark-owned behavior.

## Decision

Landmark-owned runtime behavior is implemented as a Rust CLI in
`crates/landmark` and distributed to action consumers as the checked-in Linux
binary `dist/landmark`.

The composite action remains the public GitHub Action wrapper. It still uses
GitHub expressions, shell, and `semantic-release` where those are the right
boundaries:

- Node remains only for `semantic-release` in full mode.
- Rust owns healthcheck, changelog source selection, synthesis, validation,
  release-body mutation, artifacts, RSS, notifications, failure issues,
  floating-tag parsing, metadata checks, action contract checks, and replay.
- `bin/gate` is the local verifier for fmt, clippy, tests, action contract,
  version sync, replay, and binary checksum.
- `bin/replay-action` is the local consumer replay entrypoint.

The Linux binary is built from source with the `x86_64-unknown-linux-musl`
target and `rust-lld`; `dist/landmark.sha256` is committed with it.

## Consequences

Consumers no longer need Python or pip when using Landmark. Synthesis-only mode
does not require Node setup. Full mode still installs Node dependencies because
`semantic-release` remains the external release engine.

The action contract stays stable, but the implementation is now a deep Rust
module behind one CLI surface instead of many Python scripts. Backfill is
retired from the core surface; historical repairs should use synthesis-only
runs so repair behavior matches normal release-note synthesis.

## Rollback

Rollback is a normal Git revert of the Rust migration PR. Because the public
inputs and outputs are preserved and `semantic-release` was not replaced, the
rollback boundary is the action wrapper and `dist/landmark` invocation points.

If the checked-in binary is stale or invalid, `bin/gate` fails checksum
verification. If runtime behavior drifts, replay evidence under `.landmark/`
and CI artifacts are the first debugging source.
