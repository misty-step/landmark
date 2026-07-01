# Consolidate version decisions into one Rust engine

Priority: P0 · Status: ready · Estimate: L

## Goal
Make every Landmark entry point use one deterministic Rust version engine that
explains its inputs and refuses unknown release intent instead of silently
patching.

## Oracle
- [ ] `landmark run`, `prepare-self-release`, and GitHub full mode agree on the
      same bump for the same commit range.
- [ ] Unknown or non-conventional commits refuse, warn with an explicit
      required policy, or consume a typed waiver; they never silently become
      `patch`.
- [ ] Evidence names the commit or policy signal that drove the bump, not only
      aggregate commit counts.
- [ ] Existing `release_bump` and `decide_version_bump` tests are folded into
      one corpus covering breaking markers, `feat`, `fix`, `perf`, reverts,
      squash bodies, non-conventional commits, empty ranges, and bootstrap
      ranges.
- [ ] Semantic-release remains only as an explicitly named compatibility path
      until full-mode v2 can publish from Rust.
- [ ] `bin/gate` passes.

## Children
1. Introduce one shared version-decision type and implementation in Rust.
2. Migrate `landmark run` and `prepare-self-release` onto that implementation.
3. Emit driver-level evidence that names the decisive commit, unknown commit, or
   waiver.
4. Revisit full GitHub mode so Rust owns analyze, version, changelog, tag, and
   publish while semantic-release is compatibility only.
5. After the compatibility window, remove the Node semantic-release stack from
   the gate and action contract.

## Verification System
- Claim: release version truth is singular and deterministic.
- Falsifier: any entry point can compute a different bump for the same range, or
  unknown commits publish as patch without explicit policy.
- Driver: shared cargo test corpus, self-release replay scenario, provider run
  replay scenarios, and `bin/gate`.
- Grader: version evidence JSON and generated release plan comparison.
- Evidence packet: test fixture logs and replay evidence directories.
- Cadence: run before changing release-operation code.

## Notes
This supersedes the current split between semantic-release, `decide_version_bump`
in `release_ops/run.rs`, and `release_bump` in `self_release.rs`.
