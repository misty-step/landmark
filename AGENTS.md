# Landfall Agent Contract

## Product Boundary
Landfall is a portable release-intelligence runtime. The GitHub Action is one
packaging layer, not the product boundary. Keep release analysis, synthesis,
artifact writing, feed generation, notifications, and provider policy in the
Rust CLI. Keep GitHub-specific behavior behind explicit adapter seams.

## Architecture
- `crates/landfall/src/main.rs` owns the Rust runtime and current CLI surface.
- `action.yml` is a composite GitHub Action wrapper around `dist/landfall` plus
  `semantic-release` for full GitHub release mode.
- `dist/landfall` is the checked-in Linux x86_64 musl binary consumed by the
  action. On macOS, ARM64 Linux, or any other non-Linux-x86_64 platform, use
  `cargo run --locked -p landfall -- ...` or a locally built
  `target/debug/landfall`; do not execute `dist/landfall` locally.
- Node is only for `semantic-release` in full mode. Do not add new Node or
  shell orchestration unless the platform boundary requires it.
- Python is not part of the active runtime. Do not reintroduce Python scripts
  for release behavior.

## Portability Direction
- A non-GitHub caller must be able to drive Landfall through CLI commands,
  manifest files, JSON artifacts, and local git state.
- `synthesis-only`, `backfill --mode artifacts-only`, `write-artifacts`,
  `update-feed`, and webhook/Slack notification paths are the portable core.
- GitHub operations such as release-body mutation, PR extraction, issue
  lifecycle, fleet scan, and Action outputs must be treated as adapter-specific.
- Prefer adding a provider interface or local artifact sink over broadening
  GitHub assumptions.

## Repo Gates
- Run `bin/gate` before closeout for code or contract changes.
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
