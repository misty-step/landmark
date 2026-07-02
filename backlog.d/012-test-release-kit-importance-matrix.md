# Pin release_kit's importance/audience/rich-artifact decision matrix with unit tests

Priority: P1 · Status: ready · Estimate: S

## Goal
`crates/landmark/src/release_kit.rs` has zero `#[test]` functions covering
`release_kit_importance`, `release_kit_audiences`,
`release_kit_needs_rich_artifacts`, or `release_kit_importance_reason` — the
functions that decide which final-mile artifacts (migration guides, docs
patches, blog drafts, demo videos) get planned for a release. Add direct unit
tests pinning the existing decision table so it can't silently drift.

## Oracle
- [ ] `release_kit_importance` (`release_kit.rs:460`, line numbers shift as
      011 and this ticket land — grep for the function name, don't trust the
      line number) has a test per branch: `security` (classification.security),
      `migration` (major bump, or classification.breaking, or
      migration_heavy), `high` (significance == "high"), `launch` (empty
      latest_tag + bump != "none"), `low` (significance == "low"), and the
      `medium` fallback — constructed directly from
      `ReleaseClassification`/`RunVersionDecision` values, not through the
      full `plan()` pipeline.
- [ ] `release_kit_audiences` (`release_kit.rs:480`) has a test proving
      `release-operator`/`docs-owner` are added only when
      `release_kit_needs_rich_artifacts(importance)` is true, and the primary
      audience plus `developer-operator` are always present.
- [ ] `release_kit_needs_rich_artifacts` (`release_kit.rs:491`) has a test
      naming every importance value it treats as needing rich artifacts vs not.
- [ ] `cargo test --locked` and `bin/gate` pass.

## Split ownership before adding, not after
011 landed release_kit.rs at 1012 lines (a deliberate, explicitly-reasoned
`bin/check-architecture` bump to 1050 — see that script's comment — not a
blank check). Adding this ticket's tests without splitting the file risks
hitting that cap again immediately. `assert_contract` and its supporting
JSON-schema-shape validators (`assert_json_eq`, `assert_schema_contract`,
`assert_supported_schema_keywords`, `collect_unsupported_schema_keywords`,
`supported_schema_keyword`, `validate_contract_schema_node`,
`validate_object_schema_node`, `validate_array_schema_node`, `json_type` —
roughly release_kit.rs:670 to end) is a self-contained, separable concern
(validating release-kit JSON shape against its schema) from the
plan/build/artifact-decision logic this ticket is testing. Split that block
into its own module (e.g. `release_kit_contract.rs`) as part of this ticket,
before adding new tests, rather than growing the combined file further.

## Notes
Verified live: `grep -rn "fn.*release_kit\|#\[test\]" crates/landmark/src/release_kit.rs`
shows no test functions, and `cargo test --locked release_kit` runs 0 tests.
The only existing coverage is `release_kit::assert_contract` inside replay
scenarios, which checks JSON *shape*, not that `importance`/`audiences` are
*correct* for a given classification/decision input. This ticket is
independent of `011-fix-release-kit-classifier-call-site.md` — it unit-tests
the pure decision functions directly with constructed inputs, so it can land
before, after, or alongside 011 without conflict.

**Why:** teardown report flagged release-kit as the highest-leverage untested
surface once classification correctness is addressed; this closes the "zero
unit tests on a 990-line decision module" gap independently confirmed by
running `cargo test --locked release_kit` (0 tests) during this groom pass.
