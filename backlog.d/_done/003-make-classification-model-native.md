# Make release classification model-native

Priority: P0 · Status: done · Estimate: L

## Goal
Replace keyword release classification with a BYOK model call over structured
parsed commits and diff statistics, while retaining conventional commits as the
deterministic floor signal.

## Oracle
- [x] The v1.25.0 Landmark regression fixture classifies as user-visible and
      synthesis-worthy despite semantic-release's `### Features` changelog
      rendering.
- [x] Classification input includes parsed commit subjects, conventional types,
      breaking signals, commit bodies where available, and diff statistics; the
      rendered changelog is context only.
- [x] A cheap OpenRouter-compatible model path performs classification under the
      existing provider policy, BYOK, fallback, redaction, and dry-run
      constraints.
- [x] If deterministic `feat`, `fix`, `perf`, or breaking signals disagree with
      model classification or skip policy, the disagreement is recorded in
      evidence and surfaced in release output instead of becoming a silent skip.
- [x] Misfire fixtures cover v1.25.0, workflow/manifest/cli substring
      landmines, reverts, squash bodies, non-conventional commits, `perf`, empty
      ranges, and rename/bootstrap ranges.
- [x] `bin/replay-action` covers the skip-vs-misfire path and `bin/gate`
      passes.

## Children
1. Extract a structured classification input from parsed commits and diff stats
   in the Rust runtime.
2. Define the deterministic floor signal from conventional commits without
   treating it as the final classifier.
3. Add a model-native classifier using the existing OpenRouter-compatible
   provider policy, with schema-checked output and redacted diagnostics.
4. Add disagreement alarms to evidence, release-body output, and failure-issue
   paths where appropriate.
5. Build the regression corpus, led by Landmark v1.25.0's own changelog
   misfire.
6. Split or ratchet modules only with an explicit architecture reason if
   `synthesis.rs` crosses its budget.

## Verification System
- Claim: Landmark no longer decides release importance from rendered changelog
  substring heuristics.
- Falsifier: v1.25.0 or a fixture with `feat`/`fix` commits can still be skipped
  without a visible disagreement alarm.
- Driver: targeted cargo tests, replay scenario for the misfire, and
  `bin/replay-action`.
- Grader: evidence JSON and release-body fixture inspection.
- Evidence packet: replay evidence directory plus regression fixture outputs.
- Cadence: run on every classifier or synthesis-policy change.

## Notes
Operator decision: this is Landmark's top priority. Deterministic parsing stays
as a floor signal; judgment belongs in the model-native classifier.

## Delivered
- PR #161 extracted structured deterministic release classification, included
  commit bodies and diff statistics in context, and covered the v1.25.0
  user-visible regression fixture.
- PR #163 added the OpenAI-compatible model classifier preflight with provider
  policy, fallback handling, schema-checked JSON, deterministic floor behavior,
  disagreement evidence, and dry-run/no-network coverage.
- PR #164 surfaced deterministic/model classification disagreements in generated
  release notes and replay evidence.
- PR #165 and PR #166 added the misfire regression corpus for workflow,
  manifest, CLI substring, squash-body, non-conventional, revert, `perf`, empty
  range, and rename/bootstrap edge cases.
- Verification: `bin/gate` passed locally for each implementation PR, and
  hosted `local-gate`, `merge-gate`, and secret-scan checks were green before
  merge.
