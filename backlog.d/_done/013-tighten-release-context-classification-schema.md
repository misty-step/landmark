# Tighten release-context.v1.schema.json's classification/cost objects and add drift detection

Priority: P1 · Status: done · Estimate: M

## Goal
`schemas/release-context.v1.schema.json`'s `classification` and `cost`
sub-objects are declared as bare `{ "type": "object" }` with no `properties`,
even though the Rust structs they describe (`ReleaseClassification`,
`CostEstimate` in `crates/landmark/src/manifest.rs:176-201`) are stable and
richer than that today (including the newer `deterministic_signals` and
`disagreements` fields). Add real `properties`/`required` to both, and wire a
drift check so the schema can't silently fall out of sync with the struct
again, following the pattern the repo already uses for the manifest schema.

## Oracle
- [x] `schemas/release-context.v1.schema.json`'s `classification` property
      lists `categories`, `significance`, `user_visible`, `breaking`,
      `security`, `migration_heavy`, `source`, `model`,
      `deterministic_signals`, `disagreements`, `reasons` with correct types.
- [x] `schemas/release-context.v1.schema.json`'s `cost` property lists
      `input_tokens`, `output_tokens`, `model_tier`, `model`, `estimated_usd`,
      `skip`, `skip_reason` with correct types.
- [x] A key-set drift check exists for both objects, added to
      `manifest_schema_key_contracts()`-style table (or a sibling function)
      and wired into `validate_agent_native_contracts`
      (`crates/landmark/src/release_ops/contracts.rs:186`), so
      `bin/check-action-contract` fails if the schema's declared keys and the
      Rust struct's serialized field names diverge.
- [x] `bin/gate` passes.

## Evidence
- Generalized `validate_manifest_schema_alignment(path)` into
  `validate_schema_key_alignment(path, contracts)` (contracts now passed in,
  not hardcoded), and added `release_context_schema_key_contracts()` in
  `manifest.rs` alongside its manifest sibling. Wired both the manifest and
  new release-context checks into `validate_agent_native_contracts`.
- Both `classification` and `cost` also got `additionalProperties: false`,
  matching the convention already used throughout
  `landmark-manifest.v1.schema.json` and the `version_decision` object added
  to `run-evidence.v1.schema.json` earlier tonight — the top-level
  `release-context.v1.schema.json` stays deliberately loose
  (`additionalProperties: true`), only these two high-stakes sub-objects
  tightened.
- Verified the drift check is a real, non-tautological guard: temporarily
  deleted `disagreements` from the schema's `classification.properties` and
  `required` list and confirmed `check-action-contract` fails with
  `release-context.classification schema keys drifted from runtime
  validation: expected {...11 keys...}, got {...10 keys...}`, then restored.

## Notes
Verified live: `schemas/release-context.v1.schema.json`'s `classification` and
`cost` properties are `{"type": "object"}` with no nested `properties` today
(the top-level schema is deliberately loose via `additionalProperties: true`,
but these two objects carry the highest-stakes agent-facing fields — the exact
fields a disagreement-alarm-reading agent needs). The repo already has the
right pattern for this: `manifest_schema_key_contracts()`
(`crates/landmark/src/manifest.rs:417-465`) declares expected property-key
sets per schema pointer, and `validate_manifest_schema_alignment`
(`crates/landmark/src/release_ops/contracts.rs:439-453`) diffs them against
the actual schema file using `schema.pointer(pointer)` — no external jsonschema crate needed. Reuse that
exact mechanism, pointed at `release-context.v1.schema.json`'s
`/properties/classification/properties` and `/properties/cost/properties`.
VISION.md: "Agent-native contracts are first-class... `schemas/` ... are
public surfaces, not internal conveniences" — this ticket makes that true for
the two objects that changed shape most recently (tonight's classification
work added `deterministic_signals`/`disagreements`).

**Why:** confirmed no JSON-schema-validation-against-real-output exists
anywhere in the test suite (`grep -rn "jsonschema\|schemars" crates/*/Cargo.toml`
returns nothing); the closest existing check
(`validate_agent_native_contracts`) only verifies `$id`/`x-landmark-artifact`
and README/guide doc-token presence, not structural conformance.
