# Landmark Agent Contract

## Product Boundary
Landmark is a portable release-intelligence runtime. The GitHub Action is one
packaging layer, not the product boundary. Keep release analysis, synthesis,
artifact planning, artifact writing, feed generation, notifications, evidence,
and provider policy in the Rust CLI. Keep GitHub-specific behavior behind
explicit adapter seams.

Landmark owns release truth, audience/importance classification, release-kit
plans, provenance, approval state, and producer contracts for final-mile output.
It does not own bespoke media production, brand design, CMS publishing, or
long-running creative pipelines. Demo videos, GIFs, images, blog posts, essays,
and docs updates should be represented as typed planned/produced artifacts and
delegated to explicit local, browser, service, harness, or human producer
adapters.

## Architecture
- `crates/landmark/src/main.rs` is the Rust binary facade: parse CLI, dispatch,
  and render top-level errors. Runtime responsibilities should live in focused
  modules under `crates/landmark/src/`.
- `bin/check-architecture` ratchets the facade and extracted module sizes; if
  a module needs to grow past its current budget, split ownership first or
  update the ratchet with an explicit architecture reason.
- `action.yml` is a composite GitHub Action wrapper around `dist/landmark` plus
  `semantic-release` for full GitHub release mode.
- `dist/landmark` is the checked-in Linux x86_64 musl binary consumed by the
  action. On macOS, ARM64 Linux, or any other non-Linux-x86_64 platform, use
  `cargo run --locked -p landmark -- ...` or a locally built
  `target/debug/landmark`; do not execute `dist/landmark` locally.
- Node is only for `semantic-release` in full mode. Do not add new Node or
  shell orchestration unless the platform boundary requires it.
- Python is not part of the active runtime. Do not reintroduce Python scripts
  for release behavior.

## Portability Direction
- A non-GitHub caller must be able to drive Landmark through CLI commands,
  manifest files, JSON artifacts, and local git state.
- `synthesis-only`, `backfill --mode artifacts-only`, `write-artifacts`,
  `update-feed`, and webhook/Slack notification paths are the portable core.
- `release-kit` artifacts are the planning/evidence boundary for richer
  final-mile output; prefer extending the typed kit contract over embedding a
  producer in the core runtime.
- GitHub operations such as release-body mutation, PR extraction, issue
  lifecycle, fleet scan, and Action outputs must be treated as adapter-specific.
- Prefer adding a provider interface or local artifact sink over broadening
  GitHub assumptions.

## Repo Gates
- Run `bin/gate` before closeout for code or contract changes.
- `bin/gate` includes `bin/check-architecture`; do not weaken the ratchet to
  land feature work.
- For action contract changes, also ensure `check-action-contract` coverage
  remains green through the gate.
- Use `bin/replay-action` when touching release orchestration, synthesis,
  artifact outputs, release-body mutation, notifications, feeds, or failure
  lifecycle behavior.

## Backlog And Docs
- Active work lives in `backlog.d/<nnn>-<slug>.md`; completed items are archived
  under `backlog.d/_done/`.
- Strategic groom reports live under `.groom/`.
- Keep README, `action.yml`, examples, and this file aligned. Stale agent-facing
  prose is a release risk because agents use it as an operating contract.

## Git
Prefer `jj` for local status and commits when it is available; fall back to
non-destructive `git` commands when an agent environment does not provide it.
Preserve user changes and avoid destructive git commands.
