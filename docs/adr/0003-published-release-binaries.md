# ADR 0003: Published Release Binaries, Not a Checked-In Binary

Status: accepted
Date: 2026-07-01

## Context

ADR 0001 distributed the Rust runtime to action consumers as a checked-in
Linux binary, `dist/landmark`, refreshed and committed on every release. That
approach worked but grew the Git history cost of the repository every release
(a multi-megabyte musl binary committed each time) and only ever covered one
platform; non-Linux consumers of the CLI had no packaged binary at all.

## Decision

Landmark release binaries are built for a supported target matrix
(`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`,
`aarch64-apple-darwin`, `x86_64-apple-darwin`) and published as GitHub Release
assets, alongside a `checksums.txt`, by `.github/workflows/release.yml`'s
`build-release-assets` / `publish-release-assets` jobs. No binary is checked
into Git.

`action.yml` gains a `Bootstrap Landmark binary` step that runs before any
other step. It reads the action's own pinned version from `package.json`,
maps the runner's `uname -s`/`uname -m` to a target, downloads the matching
asset and `checksums.txt` from
`https://github.com/misty-step/landmark/releases/download/v<version>/`,
verifies the SHA-256 checksum, and exports `LANDMARK_BIN` for every later step
to invoke. The step fails closed (non-zero exit) on missing assets, an
unsupported platform, or a checksum mismatch.

The floating major tag (`v1`) and any release's assets are only made current
after `publish-release-assets` uploads binaries for that tag, so a consumer
resolving `@v1` never lands on a tag whose binary is not yet downloadable.

`prepare-self-release` and `publish-self-release` no longer build or refresh
any binary; the self-release PR path only touches `CHANGELOG.md`,
`package.json`, `crates/landmark/Cargo.toml`, and `Cargo.lock`.

## Consequences

Every consumer platform (Linux x86_64/arm64, macOS x86_64/arm64) gets a real
packaged binary; local CLI users can download one instead of requiring Cargo.
Release commits are small permanently; the repository stops accumulating a
committed binary's history weight release over release. `dist/` and its
history are removed from the repository in a follow-up, separately authorized
`git filter-repo` history rewrite (see backlog 006), since past commits
already carry the binary's weight.

The action now has a hard network dependency at the very first step: if
`github.com/misty-step/landmark/releases` is unreachable, no Landmark step can
run. This is judged an acceptable tradeoff because the action already depends
on GitHub API reachability for every other step.

## Rollback

Revert the workflow/action-boundary PR. Because `dist/landmark` is removed
from Git going forward (not merely stopped), rollback would need to
reintroduce the checked-in binary and its refresh path from before this
change, not just revert application logic.
