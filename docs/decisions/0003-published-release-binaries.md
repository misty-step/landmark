# 0003: Publish release binaries; do not check them into Git

- **Status:** accepted
- **Date:** 2026-07-01
- **Primitives:** release binary assets, GitHub Action bootstrap
- **Powder:** none recorded

## Context
[0001](0001-rust-runtime-boundary.md) originally distributed a checked-in Linux
binary refreshed on every release. Each multi-megabyte binary grew Git history,
and non-Linux CLI consumers had no packaged binary.

## Decision
The [release workflow](../../.github/workflows/release.yml) builds and publishes
Linux musl and macOS binaries for x86_64 and arm64 with `checksums.txt` as
GitHub Release assets; no binary is checked into Git. The [Action](../../action.yml)
bootstraps the asset for its own pinned version and runner platform, verifies
SHA-256, and fails closed on missing assets, unsupported platforms, or checksum
mismatch; floating tags become current only after assets are uploaded.

## Consequences
All four supported platforms get downloadable binaries and release commits
stop adding binary history weight. Self-release preparation and publication
no longer build or refresh binaries. Bootstrap introduces a hard network
dependency on GitHub Releases, accepted because the Action already depends
on GitHub API reachability. The earlier history-rewrite proposal is not
authorization to rewrite Git history or delete retained evidence.

Revisit if: the GitHub Releases network dependency is no longer acceptable.
Rollback would require restoring the checked-in binary and its refresh path,
not merely reverting application logic.
