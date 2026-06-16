# Close self-release binary drift

Priority: P0 · Status: done · Estimate: M

## Goal
Ensure Landmark self-release cannot publish version metadata that leaves the checked-in Linux action binary behind the Rust source.

## Oracle
- [x] The self-release PR path updates `dist/landmark` and `dist/landmark.sha256` whenever `crates/landmark/Cargo.toml` changes.
- [x] CI compares `target/x86_64-unknown-linux-musl/release/landmark` to `dist/landmark`, matching local `bin/gate`.
- [x] A replay or unit test proves a generated self-release PR includes the binary artifact or fails with an actionable error.
- [x] `bin/gate` exits 0 immediately after a self-release version bump.

## Children
1. Decide whether `prepare-self-release` should build the Linux binary directly or fail with a release-blocking instruction when the artifact is stale.
2. Add the binary `cmp` check to `.github/workflows/ci.yml` so hosted Quality Checks catch the same drift as `bin/gate`.
3. Extend `self_release_pr_path` replay evidence to assert binary/checksum handling.
4. Backfill the current `v1.18.1` binary/checksum alignment as a mechanical artifact update.

## Notes
- Evidence: after `chore(release): 1.18.1`, `bin/gate` failed at `cmp target/x86_64-unknown-linux-musl/release/landmark dist/landmark` while hosted Quality Checks had passed.
- Evidence: `.github/workflows/ci.yml` currently runs `cargo build`, `shasum -a 256 -c dist/landmark.sha256`, and `dist/landmark --help`, but does not compare the fresh build to the checked-in binary.
- Why: the groom harness audit found a gap between local and hosted verification on the artifact that consumers actually execute.

## Delivery Evidence
- Red replay: `cargo run --locked -- replay-action --evidence-dir .landmark/replay-012-red --scenario self_release_pr_path` failed with `prepare-self-release did not refresh dist/landmark`.
- Focused replay: `cargo run --locked -- replay-action --evidence-dir .landmark/replay-012-green --scenario self_release_pr_path` passed and recorded `dist/landmark` plus `dist/landmark.sha256` in the generated release plan evidence.
- Hosted failure: PR #118 Quality Checks initially failed because floating hosted `stable` used `rustc 1.96.0` while the local checked-in binary had been built with `rustc 1.94.0`; the new `cmp` gate caught the compiler-version drift.
- Toolchain fix: `rust-toolchain.toml` pins Rust `1.96.0`, and CI/release workflows install that version explicitly before building or preparing self-release artifacts.
- Hosted parity fix: local macOS cross-builds still differed from Ubuntu Linux builds, so `bin/build-linux-action` now refreshes or checks `dist/landmark` inside the pinned Linux builder on non-Linux hosts, and `prepare-self-release` refuses to produce the production Linux artifact from non-Linux hosts.
- Hosted evidence fix: Quality Checks now uploads the freshly built Linux action binary before byte comparison, so any future mismatch leaves a recoverable artifact for correction instead of only a failed log line.
- Artifact backfill: downloaded `landmark-linux-action-binary` from PR #118 run `27445100772`, replaced `dist/landmark`, and regenerated `dist/landmark.sha256` to `ff9be02556f7cd9b4a58f872c58e8a6e56d467295c622a3184fdd28929fd8f4c`.
- Full gate: `bin/gate` passed under Rust `1.96.0`, including unit tests, action replay, executable/checksum verification, and Linux byte-compare on Linux hosts; hosted Quality Checks remains authoritative for byte-for-byte Ubuntu parity.
