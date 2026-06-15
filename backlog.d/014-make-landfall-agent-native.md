# Make Landfall self-describing and agent-native

Priority: P0 · Status: pending · Estimate: XL

## Goal
Give agents stable machine contracts for configuring, running, verifying, and
debugging Landfall without reverse-engineering README prose or Rust internals.

## Oracle
- [ ] Versioned JSON Schemas exist for `.landfall.yml`, synthesis status, replay result, fleet plan, artifact feed entries, and release evidence packets.
- [ ] `landfall describe --json` exposes commands, modes, providers, inputs, outputs, schemas, and examples in one machine-readable document.
- [ ] Mutating commands support preview/dry-run or explain why preview is impossible.
- [ ] Commands that agents call support deterministic JSON output, with payloads on stdout and logs/errors on stderr.
- [ ] Failure output includes a stable code, stage, retryability, user action, and redacted context.
- [ ] A cold-agent integration guide can be validated by a script or replay scenario.

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
