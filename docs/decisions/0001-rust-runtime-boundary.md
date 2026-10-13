# 0001: Rust owns Landmark runtime behavior, not Python

- **Status:** accepted
- **Date:** 2026-06-11
- **Primitives:** Rust CLI, GitHub Action wrapper
- **Powder:** none recorded

## Context
Landmark delegated version analysis, changelog generation, and GitHub Release
creation to `semantic-release`, while its own synthesis, artifacts, policy,
notifications, and release-body updates lived in Python scripts and shell
fragments. The repository uses Rust by default; consumers should not need
Python or pip for Landmark-owned behavior.

## Decision
Landmark-owned runtime behavior belongs in the [Rust CLI](../../crates/landmark/),
not Python scripts. The [composite Action](../../action.yml) remains the GitHub
wrapper, with Node only for `semantic-release` in full mode; the original
checked-in binary distribution choice is superseded by [0003](0003-published-release-binaries.md).

## Consequences
Consumers need no Python or pip, and synthesis-only mode needs no Node setup.
One CLI owns synthesis, validation, release-body mutation, artifacts, feeds,
notifications, policy, failure issues, metadata checks, and replay rather than
many scripts. The public Action contract and external release engine stay stable;
[gate](../../bin/gate) and [replay](../../bin/replay-action) verify that boundary.

Rollback: a normal Git revert of the Rust migration PR at the Action wrapper
and its binary invocation points. The original checked-in `dist/landmark`
invocation is superseded by [0003](0003-published-release-binaries.md).

Revisit if: not stated in the original decision.
