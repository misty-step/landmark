# Make Landfall self-describing and agent-native

Priority: P0 · Status: done · Estimate: XL

## Goal
Give agents stable machine contracts for configuring, running, verifying, and
debugging Landfall without reverse-engineering README prose or Rust internals.

## Oracle
- [x] Versioned JSON Schemas exist for `.landfall.yml`, synthesis status, replay result, fleet plan, artifact feed entries, and release evidence packets.
- [x] `landfall describe --json` exposes commands, modes, providers, inputs, outputs, schemas, and examples in one machine-readable document.
- [x] Mutating commands support preview/dry-run or explain why preview is impossible.
- [x] Commands that agents call support deterministic JSON output, with payloads on stdout and logs/errors on stderr.
- [x] Failure output includes a stable code, stage, retryability, user action, and redacted context.
- [x] A cold-agent integration guide can be validated by a script or replay scenario.

## Children
1. Publish schemas under `schemas/` and add schema validation to `doctor` and the gate.
2. Add `describe --json` and keep it generated from the actual clap command surface where practical.
3. Introduce `--format text|json` for agent-facing commands and document stdout/stderr discipline.
4. Define a failure taxonomy that covers provider auth, provider outage, invalid changelog source, budget skip, synthesis degradation, artifact write failure, feed failure, and publication mutation failure.
5. Add examples and sample evidence packets for local, GitHub Action, and synthesis-only runs.
6. Extend contract checks so README, examples, `AGENTS.md`, and schemas cannot drift from the runtime.

## Notes
- Evidence: the repo currently has no `*.schema.json` files for the primary artifacts.
- Evidence: `AGENTS.md` was stale after the Rust migration, proving prose drift is a real agent risk.
- Why: the primary users are often agents; agents need contracts, not vibes.

## Delivery
- Added a checked schema registry under `schemas/` for the manifest, synthesis status, replay result, fleet plan, release-entry artifact, run evidence, and JSON failure envelope.
- Added `landfall describe --json`, generated from Clap command metadata plus explicit runtime contracts for schemas, providers, modes, preview policy, stdout/stderr discipline, examples, and failure taxonomy.
- Added `--error-format json`, `run --dry-run`, `doctor --format json`, `replay-action --format json`, and JSON stdout options for fleet scan/plan/open-prs.
- Added `setup --dry-run`, manifest shape validation, schema-vs-runtime key alignment checks, and command-contract coverage checks against the live Clap command tree.
- Added `docs/agent-integration.md` and README agent-native contract documentation.
- Added `agent_native_contracts` replay coverage and wired schema/guide validation into `check-action-contract`; replay now exercises `run --dry-run`, `backfill --dry-run`, and fleet scan/plan/open-prs JSON stdout paths.
- Verification: `cargo test --locked manifest_shape_rejects_unknown_keys`; `cargo test --locked failure_classifier_emits_stable_codes_and_redacts_tokens`; `cargo run --locked -- check-action-contract`; `cargo run --locked -- replay-action --scenario agent_native_contracts --format json --evidence-dir .landfall/replay-014-agent`; `bin/gate`.
- Evidence packet: `.landfall/replay-014-agent/replay-result.json` records the passing cold-agent contract replay with 7 schemas, JSON error code `invalid_input`, no-write `run --dry-run` release `v1.1.0`, `backfill_dry_run=true`, and fleet JSON path coverage.
