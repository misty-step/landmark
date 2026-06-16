# Add a Landmark product manifest

Priority: P0 · Status: done · Estimate: L

## Goal
Make each repository's release context, audience, artifact policy, and cost posture durable in `.landmark.yml` instead of scattering it through workflow inputs.

## Oracle
- [x] `dist/landmark init --repo-root . --output .landmark.yml --dry-run` emits a valid manifest seeded from observable repository metadata.
- [x] `dist/landmark setup --repo-root . --output-dir .landmark/setup` reads `.landmark.yml` and reflects manifest values in generated workflows.
- [x] `dist/landmark synthesize ...` uses manifest `product`, `audience`, `voice`, artifact, and model policy defaults when equivalent action inputs are absent.
- [x] `cargo run --locked -- check-action-contract` fails when README, action inputs, or manifest schema docs drift.
- [x] `bin/gate` exits 0 and replay evidence covers manifest defaults plus action-input override precedence.

## Children
1. Define `.landmark.yml` schema for product name, audience, description, voice guide, changelog source, artifact outputs, release profile, model policy, and budget hints.
2. Add `landmark init` to infer a first manifest from README, package metadata, repository name, release tool, and existing Landmark inputs.
3. Teach setup generation and synthesis to load the manifest, with explicit action inputs retaining precedence.
4. Add `landmark doctor` checks for missing, stale, contradictory, or overbroad manifest fields.
5. Document manifest-first adoption and update the contract checker to validate examples and schema snippets.

## Notes
- Evidence: `action.yml` already exposes `audience`, `product-description`, `voice-guide`, `prompt-template-path`, `changelog-source`, and artifact outputs, but consumers must repeat those choices in workflow YAML.
- Evidence: `landmark setup` currently diagnoses release tooling and emits workflows, but it does not write repo-native product context artifacts.
- Why: both product/adoption and technical/economics critics identified the manifest as the keystone that makes fleet rollout, cheap synthesis, previews, and backfill composable.

## Closure Evidence
- Implemented typed `.landmark.yml` schema, `init`, `doctor`, and `manifest-defaults` in the Rust CLI.
- Action precedence is now explicit: non-empty action inputs override manifest values, then built-in defaults apply.
- `setup` projects manifest defaults into generated workflows; `synthesize` uses manifest product, audience, voice, changelog, model, and fallback defaults.
- Contract validation now checks manifest schema documentation tokens in `README.md`.
- Contract validation now checks manifest schema enum docs and the action's manifest-default precedence expressions.
- `model.policy` is consumed when `model.primary` is absent: `cheap` selects `openai/gpt-4o-mini`; `balanced` and `rich` select `anthropic/claude-sonnet-4`.
- `release.profile` is validated and feeds setup recommendation mode/rationale.
- Verification:
  - `cargo test --locked` passed: 19 tests.
  - `cargo run --locked -- check-action-contract` passed.
  - `cargo run --locked -- init --repo-root . --output .landmark.yml --dry-run` emitted the seeded Landmark manifest.
  - `cargo run --locked -- replay-action --evidence-dir .landmark/replay-008-manifest --scenario manifest_defaults_and_overrides` passed.
  - `cargo run --locked -- replay-action --evidence-dir .landmark/replay-008-action-defaults --scenario action_manifest_defaults_precedence` passed.
  - `bin/gate` passed and wrote `.landmark/replay/replay-result.json`; canonical replay includes `action_manifest_defaults_precedence` and `manifest_defaults_and_overrides`.
  - Local macOS cannot execute the checked-in Linux `dist/landmark`; `bin/gate` verifies checksum locally and defers byte-for-byte source-to-dist parity to hosted Linux CI.
