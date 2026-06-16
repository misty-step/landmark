# Migrate the Landmark runtime to Rust

Priority: P1 · Status: done · Estimate: XL

## Goal
Move Landmark's owned runtime from Python and bash orchestration to a tested Rust binary while preserving the public GitHub Action contract and the semantic-release engine boundary.

## Oracle
- [x] `bin/gate` exits 0 and runs Rust fmt, clippy, tests, action metadata validation, contract validation, and the consumer replay harness.
- [x] `cargo test --locked` and `cargo clippy --locked --all-targets -- -D warnings` exit 0.
- [x] `bin/replay-action --scenario synthesis-only-success --scenario full-semantic-release --scenario degraded-required-fails --scenario release-body-fallback` exits 0 and leaves an evidence packet.
- [x] `! rg -n "setup-python|python -m pip|\\$\\{GITHUB_ACTION_PATH\\}/scripts/.*\\.py|scripts/.*\\.py" action.yml .github/workflows README.md` exits 0, except historical changelog text.
- [x] `test -z "$(find scripts tests -name '*.py' -print)"` exits 0 after the final removal milestone.
- [x] `npm ci --no-fund --no-audit` still exits 0 because semantic-release remains the external release engine for full mode.

## PRD Summary
- User: maintainers shipping Landmark and consumer-repo developers who need predictable release automation.
- Problem: Landmark's owned behavior is split across an 815-line composite action, 17 Python scripts, Python package bootstrap, repeated shell snippets, and duplicated markdown/GitHub helpers.
- Goal: one Rust-owned runtime carries Landmark policy, HTTP, rendering, synthesis, diagnostics, and artifact behavior behind the existing action inputs and outputs.
- Why now: the post-clearance groom found release-integrity, live-replay, and typed-artifact work that all become cheaper if the runtime boundary is made deep before more behavior accretes.
- UX enabled: consumers get the same action contract with fewer runtime dependencies, bounded failure semantics, and clearer evidence packets when releases partially fail.
- Deliverable type: migration plus harness primitive.
- Success signal: a release replay packet proves Rust and the legacy runtime agree before Python is deleted, then `action.yml` no longer installs or invokes Python.

## Product Requirements
- P0: Preserve all documented `action.yml` inputs and outputs unless a separate contract-change ticket deprecates them first.
- P0: Preserve full mode's semantic-release behavior; Rust may configure, guard, and inspect semantic-release, but must not reimplement conventional-commit release analysis in this migration.
- P0: Preserve synthesis-only mode without Node dependency.
- P0: Preserve graceful-degradation defaults while making strict policy explicit and testable.
- P0: Remove Python runtime bootstrap from consumer action runs by the final milestone.
- P1: Ship a typed note artifact model that backs markdown, plaintext, HTML, RSS, Slack, webhook JSON, and status diagnostics.
- P1: Keep a stable CLI surface usable by local tests and action wrapper steps.
- Non-goals: replacing semantic-release, changing public release-note voice, adding new notification channels, supporting non-Ubuntu runners, or redesigning GitHub issue failure reporting beyond moving it behind the Rust runtime.

## Context Packet: Rust Runtime Migration

## Constraints
- Rust owns durable Landmark behavior; Node remains only because semantic-release is a deliberate external engine.
- GitHub Action consumers must not need Python or pip after the migration.
- Secrets must stay in environment variables or process arguments with existing redaction behavior; no new log leakage.
- `synthesis-required`, `synthesis-strict`, `synthesis-quality`, failure-issue handling, floating tags, RSS commits, and webhook/Slack behavior must keep compatibility until separately deprecated.
- Migration must be parity-gated. Do not delete a Python script until Rust behavior has fixture parity or the ticket explicitly accepts a contract change.

## Repo Anchors
- `AGENTS.md` — Rust-by-default project constraint and release-pipeline purpose.
- `action.yml` — public input/output contract and current orchestration boundary.
- `package.json` / `package-lock.json` — semantic-release dependency boundary that stays Node-owned.
- `scripts/synthesize.py` — largest current policy surface: prompt rendering, changelog selection, model fallback, validation, and quality states.
- `scripts/shared.py` — retry/logging conventions to preserve in Rust.
- `scripts/update-release.py` / `scripts/extract-prs.py` / `scripts/report-synthesis-failure.py` / `scripts/close-resolved-failures.py` — duplicated GitHub API behavior to consolidate.
- `scripts/notes_render.py`, `scripts/notify.py`, `scripts/notify-slack.py`, `scripts/update-feed.py`, `scripts/write-artifacts.py` — note rendering and distribution surfaces.
- `tests/conftest.py` and `tests/test_synthesize.py` — current fake-session and synthesis behavior patterns to port into Rust tests.
- `backlog.d/002-harden-release-integrity-policy.md`, `backlog.d/003-build-a-live-consumer-replay-harness.md`, `backlog.d/005-turn-release-notes-into-a-typed-artifact-plane.md` — adjacent epics this migration should absorb or unblock, not duplicate.

## Alternatives
- Keep Python and only harden it: lowest short-term risk, but leaves pip bootstrap, duplicated runtime helpers, and an exception to the repo's Rust default as the permanent architecture. Verdict: rejected as end state; acceptable only for emergency fixes before migration lands.
- Big-bang Rust rewrite including semantic-release replacement: satisfies the phrase "full Rust" most literally, but violates the project decision to wrap semantic-release and would rebuild version analysis, changelog generation, GitHub Release creation, and ecosystem edge cases without evidence. Verdict: rejected.
- TypeScript/Node action rewrite: removes Python but deepens the non-Rust surface and ties synthesis-only mode to Node. Verdict: rejected.
- Rust-owned Landmark binary with semantic-release retained as a narrow subprocess boundary: removes Python, centralizes policy and artifact code, preserves proven release mechanics, and lets the action wrapper become shallow. Verdict: recommended.

## Design
- Add a Cargo workspace with `crates/landmark-core` for pure behavior and `crates/landmark-cli` for the action-facing binary.
- Model the release as typed state: resolved release, technical source, synthesized notes, quality, update result, distribution results, and final policy verdict.
- Implement Rust modules in this order: shared logging/retry/GitHub client, note model/renderers, prompt/changelog selection, synthesis HTTP/fallback/validation, release body update, artifact/feed writers, notifications, failure issue lifecycle, preflight/floating-tag/version helpers, backfill or deletion.
- Keep `action.yml` as a thin composite wrapper: setup Node only in full mode, invoke semantic-release only in full mode, then call the Rust binary for Landmark-owned behavior.
- Final consumer distribution uses a checked-in Linux release binary under `dist/landmark` plus a CI check that rebuilds and verifies its checksum. Source-built `cargo run` is allowed for local development and interim dogfood, but not the final consumer path.
- Add `bin/gate` as the one-command local loop. It must create/use a local tool environment without depending on global Python packages.
- The replay harness from backlog item 003 is the migration gate. If it does not exist yet, the first migration child builds the minimum harness before porting behavior.
- ADR required: yes. The ADR records the selected runtime boundary, semantic-release retention, binary distribution decision, and rollback strategy. Escalate if implementation requires replacing semantic-release or changing runner support.

## Children
1. Build `bin/gate` and the minimum replay harness needed to compare legacy and Rust behavior.
2. Add the Cargo workspace, Rust CLI skeleton, structured logging, retry policy, and GitHub client.
3. Port the note artifact model and renderers; prove parity with existing markdown/plaintext/HTML/Slack/RSS/webhook fixtures.
4. Port synthesis input selection, prompt rendering, model fallback, output validation, and quality-state writing.
5. Port release-body update, PR extraction, preflight tags, floating tags, version metadata, feed/artifact writers, notifications, and failure issue lifecycle.
6. Thin `action.yml` so shell only wires inputs/outputs and invokes semantic-release or the Rust binary.
7. Add checked-in Linux binary distribution and checksum verification.
8. Delete migrated Python scripts/tests and remove Python setup/install from docs, CI, and action runtime.
9. Run the replay matrix against the final Rust action and update `README.md`, `project.md`, and backlog references.

## Oracle (Definition of Done)
Commands that must all exit 0 after the final milestone:
- `bin/gate`
- `cargo test --locked`
- `cargo clippy --locked --all-targets -- -D warnings`
- `cargo fmt --check`
- `npm ci --no-fund --no-audit`
- `bin/replay-action --scenario synthesis-only-success --scenario full-semantic-release --scenario degraded-required-fails --scenario release-body-fallback`
- `git diff --exit-code -- dist/landmark dist/landmark.sha256`
- `! rg -n "setup-python|python -m pip|\\$\\{GITHUB_ACTION_PATH\\}/scripts/.*\\.py|scripts/.*\\.py" action.yml .github/workflows README.md`
- `test -z "$(find scripts tests -name '*.py' -print)"`

Evidence artifacts:
- Replay packet with action outputs, release body before/after, notes artifacts, tags, and structured logs.
- Parity fixtures for legacy Python behavior accepted or explicitly retired.
- ADR documenting the Rust runtime boundary and semantic-release retention.

## Premise Source
- `sha256:ff21539be7ff9a6bb685593d50ba1d7bee771158804a6567ed69f439e102e859 AGENTS.md` — Rust-by-default and Landmark action doctrine.
- `sha256:ac6869f0908513a0ae971b6bed63b27d40545ca064d24ce4d4324d0affc7c63e backlog.d/002-harden-release-integrity-policy.md` — release integrity and shell/Python hardening gaps.
- `sha256:9176fa24759e2f9f5c2e60b31b2b48f1c2730af4f8b74f299aaf4bbc8b44cbfb backlog.d/003-build-a-live-consumer-replay-harness.md` — live replay harness requirement.
- `sha256:7a8a9bb83860299d9c4070cdfb7ebde0cfec098b96ac52f4f1ab2de0375bbda7 backlog.d/005-turn-release-notes-into-a-typed-artifact-plane.md` — typed artifact plane requirement.
- Waiver: the explicit user request to shape "a full migration to Rust" lives in this chat, not a repo artifact. Residual risk is that "full" could mean replacing semantic-release; this packet intentionally rejects that interpretation based on current repo doctrine.

## Lead Repo Read
- Read `action.yml`: public input/output contract, Python setup, semantic-release boundary, synthesis orchestration, artifact writers, notification steps, and policy summary.
- Read `scripts/synthesize.py`: changelog source resolution, prompt rendering, model fallback, validation, and quality behavior.
- Read `scripts/shared.py` and `scripts/update-release.py`: structured logging, retry behavior, GitHub API headers, 404 handling, and release-body composition.
- Read `scripts/notes_render.py` plus `tests/test_notes_render.py`: current renderer subset and unsafe link behavior.
- Read `package.json` and `pyproject.toml`: Node semantic-release dependency and Python test/runtime dependency declarations.
- Read `backlog.d/001` through `005`: current post-groom backlog and adjacent epics.
- Commands run: `rg --files`, `wc -l scripts/*.py tests/*.py action.yml README.md`, `shasum -a 256 ...`, `git status --short --branch --untracked-files=all`.

## Alignment Questions
- none; assumptions accepted. The only contested interpretation is semantic-release replacement, and current repo doctrine already says Landmark wraps semantic-release rather than reinventing it.

## Risks + Rollout
- Risk: Rust port subtly changes release-note quality or markdown rendering. Mitigation: parity fixtures and replay packets before deleting Python.
- Risk: checked-in binary distribution becomes stale. Mitigation: checksum verification in `bin/gate` and CI; release commit must update binary and checksum together.
- Risk: action runtime becomes slower if it source-builds Rust. Mitigation: source-build only during interim dogfood; final consumer path uses `dist/landmark`.
- Risk: semantic-release subprocess errors are harder to diagnose through a Rust wrapper. Mitigation: keep semantic-release invocation explicit in `action.yml` until replay harness proves wrapper diagnostics.
- Rollback: keep Python scripts until the final removal milestone; if a Rust milestone fails, action can continue invoking the legacy Python path while parity gaps are fixed.

## Stop Conditions
- Stop and reshape if replacing semantic-release becomes necessary to satisfy "full Rust".
- Stop and reshape if GitHub Action distribution cannot use a checked-in Linux binary and source-building is too slow for consumers.
- Stop and return to product direction if preserving `synthesis-strict` or failure-issue behavior conflicts with the new release integrity policy.
- Stop if replay fixtures show behavior drift that is not an explicitly approved contract change.

## Delivery
- Added the Rust workspace and `landmark` CLI in `crates/landmark`, with subcommands for synthesis, release-body mutation, policy, artifacts, feeds, notifications, failure issues, metadata checks, action-contract checks, floating tags, and replay.
- Replaced Python setup and script invocation in `action.yml` and CI with the Rust runtime while retaining Node only for `semantic-release`.
- Added `dist/landmark` as a static x86-64 Linux binary, `dist/landmark.sha256`, `.cargo/config.toml` for musl `rust-lld` builds, `bin/replay-action`, and Rust-owned `bin/gate`.
- Deleted the Python scripts, pytest suite, and `pyproject.toml`.
- Added ADR `docs/adr/0001-rust-runtime-boundary.md`.
- Evidence: `bin/gate`; `bin/replay-action --scenario synthesis-only-success --scenario full-semantic-release --scenario degraded-required-fails --scenario release-body-fallback --evidence-dir .landmark/replay-ticket-006`; `file dist/landmark`; `shasum -a 256 -c dist/landmark.sha256`; no Python script references in action/CI/README; no `scripts` or `tests` Python files remain.
