# Consolidate version decisions into one Rust engine

Priority: P0 · Status: done · Estimate: L

## Goal
Make every Landmark entry point use one deterministic Rust version engine that
explains its inputs and refuses unknown release intent instead of silently
patching.

## Oracle
- [x] `landmark run`, `prepare-self-release`, and GitHub full mode agree on the
      same bump for the same commit range.
- [x] Unknown or non-conventional commits refuse, warn with an explicit
      required policy, or consume a typed waiver; they never silently become
      `patch`.
- [x] Evidence names the commit or policy signal that drove the bump, not only
      aggregate commit counts.
- [x] Existing `release_bump` and `decide_version_bump` tests are folded into
      one corpus covering breaking markers, `feat`, `fix`, `perf`, reverts,
      squash bodies, non-conventional commits, empty ranges, and bootstrap
      ranges.
- [x] Semantic-release remains only as an explicitly named compatibility path
      until full-mode v2 can publish from Rust.
- [x] `bin/gate` passes.

## Children
1. [x] Introduce one shared version-decision type and implementation in Rust
   (`crates/landmark/src/version_decision.rs`: `classify_commit` + `decide_version`).
2. [x] Migrate `landmark run` and `prepare-self-release` onto that implementation.
3. [x] Emit driver-level evidence that names the decisive commit, unknown commit, or
   waiver (`decisive_commit`/`unknown_commits` on both `RunVersionDecision` and
   `SelfReleasePlan`).
4. Revisit full GitHub mode so Rust owns analyze, version, changelog, tag, and
   publish while semantic-release is compatibility only (parked — README now
   names semantic-release as an explicit compatibility path; the full Rust
   full-mode v2 rewrite is separate, larger future work).
5. After the compatibility window, remove the Node semantic-release stack from
   the gate and action contract (parked behind child 4).

## Evidence
- New shared classifier (`classify_commit`) and reducer (`decide_version`)
  replace the three divergent implementations: `run.rs`'s `decide_version_bump`
  (silently defaulted unrecognized commits to `patch`), `self_release.rs`'s
  `classify_release_commit`/`release_bump` (silently dropped unrecognized
  commits with no record they existed), and treated `revert:`/`Revert "..."`
  commits inconsistently (self-release ignored them entirely; the shared
  engine now treats them as patch-worthy, matching semantic-release's angular
  preset default).
- Real bug fixed: a commit range containing only non-conventional commits
  (e.g., a repo mid-adoption of conventional commits, or messy bootstrap
  history) previously **silently bumped patch** in `landmark run`. It now
  resolves to no bump and names every unrecognized commit in
  `unknown_commits`, while a range with real feat/fix/perf/breaking signal
  still releases even if other unrelated commits in the same range are
  unrecognized — those are named but never block a release the recognized
  commits already justified.
- `crates/landmark/src/version_decision_tests.rs`: one corpus (14 tests)
  covering breaking bang + both `BREAKING CHANGE`/`BREAKING-CHANGE` footer
  spellings, feat/fix/perf, reverts in both conventional and git-default
  format, squash-merge bodies (embedded fake headers must not be reparsed),
  every non-release conventional type (chore/docs/test/ci/build/style/refactor),
  non-conventional commits, mixed known+unknown signal, empty ranges, and
  bootstrap ranges (with and without real signal amid messy history).
- `schemas/run-evidence.v1.schema.json` updated (`decisive_commit`,
  `unknown_commits` added to `version_decision`, both required).
- `bin/gate` green locally (fmt, clippy -D warnings, 61 unit tests, all 24
  replay scenarios, check-version-sync, check-action-contract, setup,
  actionlint, git diff --check).

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
in `release_ops/run.rs`, and `release_bump` in `self_release.rs`. Children 4/5
(full Rust GitHub full-mode v2, removing semantic-release entirely) are
explicitly parked as separate future work, per the oracle's own compatibility
clause — filed as a follow-up rather than scope-crept into this ticket.
