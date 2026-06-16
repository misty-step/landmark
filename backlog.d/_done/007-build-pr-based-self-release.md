# Build PR-based self-release under branch protection

Priority: P2 · Status: done · Estimate: L

## Goal
Let Landmark dogfood automatic self-release without requiring direct pushes to
protected `master`.

## Oracle
- [x] A release run can generate `CHANGELOG.md`, `package.json`,
      `crates/landmark/Cargo.toml`, and `Cargo.lock` changes on a release
      branch.
- [x] The generated release branch opens or updates a pull request whose checks
      include `merge-gate`.
- [x] The workflow publishes the GitHub Release only after the generated release
      PR lands.
- [x] A replay command proves the protected-branch path without mutating
      production releases.

## Notes
- Evidence: the `2026-06-11` `Release` run for commit
  `12441ec32afb35156e1c00eb62a3c32514919b8d` failed when
  `@semantic-release/git` tried to push `HEAD:master`; GitHub rejected it
  because `master` requires the `merge-gate` status check.
- Current mitigation: Landmark's own release workflow is manual so normal
  `master` pushes do not run a release job that cannot pass under current branch
  protection.

## Delivery

- Added Rust-owned `prepare-self-release` and `publish-self-release` commands.
  The prepare phase computes the next version from release-worthy conventional
  commits, prepends `CHANGELOG.md`, updates `package.json`,
  `crates/landmark/Cargo.toml`, and `Cargo.lock`, and emits GitHub Action
  outputs for the release PR.
- Replaced the manual-only release workflow with a two-phase protected-branch
  flow: `prepare-release-pr` opens/updates `landmark/self-release` through
  `peter-evans/create-pull-request`, and `publish-landed-release` creates the
  GitHub Release only when landed metadata is ahead of the latest semver tag.
- Added the `self_release_pr_path` replay scenario. It creates a disposable repo,
  proves release PR file generation, commits the release changes as landed PR
  metadata, then publishes through a fake GitHub Releases API.
- Verification: `cargo test --locked`, `cargo run --locked --
  check-action-contract`, `cargo run --locked -- replay-action --evidence-dir
  .landmark/replay`, checked-in musl binary rebuild/checksum, and `bin/gate`.
