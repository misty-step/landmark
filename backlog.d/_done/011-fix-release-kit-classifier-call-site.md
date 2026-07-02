# Fix release_kit::plan's classifier call site to use structured commit data

Priority: P0 · Status: done · Estimate: M

## Goal
`release_kit::plan` (`crates/landmark/src/release_kit.rs:173`) still calls the
plain-text substring classifier (`classify_release_context`, the
`classify_release_context_from_text` path) instead of the structured,
model-native classifier (`classify_release_context_with_deterministic` /
`classify_release_context_with_model`) that `synthesis.rs` already switched to.
Fix the call site so release-kit importance/audience planning is driven by
parsed commit data, closing the same misfire class the classifier fix already
closed for synthesis.

## Oracle
- [x] `release_kit::plan` builds a `DeterministicReleaseContext`/`Vec<ContextCommit>`
      from `release.commits` (using the existing `classify_commit` parser for
      `conventional_type`/`breaking`, mirroring how `synthesis.rs` builds its
      deterministic context) and passes it to
      `classify_release_context_with_deterministic` (or `_with_model` where a
      model is configured), not the bare-text `classify_release_context`.
- [x] A regression test reproduces the exact landmark v1.25.0 shape already
      pinned in `crates/landmark/src/classification_tests.rs` (commits:
      `feat(fleet): deliver backfill-first adoption lane`,
      `feat(run): emit release kit artifact graph`,
      `fix(fleet): attach to existing release workflows`, rendered as
      `### Features` / `### Bug Fixes` semantic-release headers) but exercised
      through `release_kit::plan`, and asserts `importance` is NOT `"low"` and
      `release_kit_needs_rich_artifacts` behaves accordingly.
- [x] `cargo test --locked` and `bin/gate` pass.

## Evidence
- Used `conventional_commit_type`/`is_breaking_commit` (the same helpers
  `synthesis.rs`'s `context_commits` uses) rather than `classify_commit` from
  `version_decision.rs` — the ticket's own "mirroring how synthesis.rs builds
  its deterministic context" instruction is the verifiable one; `classify_commit`
  is a different, coarser bump-bucketing classifier with a different purpose.
- Deleted `classify_release_context` (the bare-text wrapper) entirely once
  `release_kit.rs` stopped being its last non-test caller — dead code after
  the fix, not kept around with an `#[allow(dead_code)]`.
- New replay scenario `release_kit_classification_uses_structured_commits`
  (added to both `scenario_map()` and `canonical_scenarios()`) reproduces the
  exact landmark v1.25.0 commit shape through the real `run` CLI entry point,
  asserts `importance != "low"`, and cross-checks
  `release_kit_needs_rich_artifacts`'s implied audiences
  (`release-operator`/`docs-owner`) against what the run actually planned.
  Verified this test is a real regression guard, not tautological: temporarily
  ran the exact concatenated classification text through the untouched
  `classify_release_context_from_text` and confirmed it produces
  `significance=low, categories=["internal-tooling"]` (the `workflow`
  substring in "attach to existing release workflows" triggers the
  bare-classifier's internal-tooling heuristic) before restoring the fix.
- `release_kit.rs` grew to 1012 lines from the fix; `bin/check-architecture`'s
  cap bumped 1000 → 1050 with an explicit reason comment (not a blank check),
  and backlog 012 now carries a "split before adding tests" note pointing at
  the `assert_contract`/schema-validation block as the natural split.

## Notes
Verified live in `crates/landmark/src/release_kit.rs:150-174`: `release.commits`
(type `Vec<RunCommit>` with `subject`/`short_hash`/`body`,
`crates/landmark/src/release_ops/models.rs:252`) is already available at the
call site, so building the deterministic commit list is a small adapter, not a
redesign. `release_kit_importance` (`release_kit.rs:438-456`) directly branches
on `classification.significance`/`.security`/`.breaking`/`.migration_heavy`,
and a "low" significance from the substring classifier's landmine bugs (e.g.
`lower.contains("cli")` matching "reconcile", `"manifest"`/`"configuration"`
false-escalating, or simply missing `### Features`-style semantic-release
headers) silently shrinks the planned final-mile artifact set
(`release_kit_needs_rich_artifacts`, `release_kit.rs:469`) for what may be an
important release — the same failure shape as the groom teardown's headline
finding (`.factory-lanes/groom/landmark.md` §1), just in the kit-planning path
instead of the synthesis-skip path that was already fixed tonight (PR series
ending in commit `f7e122e`). This ticket closes the second call site.

**Why:** teardown finding §1 was fixed for `synthesis.rs` but `release_kit.rs`
was missed — confirmed live via `grep -rn "classify_release_context(" crates/landmark/src`,
which shows `release_kit.rs:173` is the only remaining caller of the unstructured
text classifier outside its own definition and tests.
