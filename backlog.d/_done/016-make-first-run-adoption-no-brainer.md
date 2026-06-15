# Make first-run adoption work in sixty seconds

Priority: P1 · Status: done · Estimate: XL

## Goal
Let a new human or agent get useful Landfall output quickly on macOS, Linux,
GitHub Actions, or arbitrary CI without first solving secrets, triggers, and
binary compatibility.

## Oracle
- [x] A zero-secret local quickstart runs on macOS and Linux and writes a useful release preview/evidence packet from a checkout.
- [x] README clearly separates local CLI, arbitrary CI, GitHub Action full mode, and GitHub Action synthesis-only mode.
- [x] Installation instructions cover source build, checked-in action binary, and any packaged binaries without implying the Linux action binary runs on macOS.
- [x] Setup-generated workflows have one blessed trigger strategy per release-tool family and do not duplicate manual-tag paths.
- [x] Node 24 action-runtime verification is part of CI before relying on GitHub-hosted defaults.
- [x] Docs consistency checks fail on stale command names, nonexistent files, bad model IDs, or mismatched examples.

## Completion Evidence
- `cargo run --locked -- replay-action --scenario first_run_local_preview --evidence-dir .landfall/replay-016-first-run` passed and wrote local evidence/artifacts for a disposable checkout with no secrets.
- `cargo test --locked setup_` passed for generated workflow trigger/runtime regression coverage.
- `cargo run --locked -- check-action-contract` passed with first-run docs, Node 24, model ID, command name, and linked-file checks.
- `bin/gate` passed.

## Children
1. Add `local preview` quickstart using only git history and artifact outputs.
2. Add cross-platform install/dev instructions and make the action binary caveat explicit.
3. Rework README around four integration modes: local CLI, generic CI, GitHub full, GitHub synthesis-only.
4. Fix setup/manual-tag duplicate trigger generation and add regression tests.
5. Add Node 24 runtime verification to CI/action replay.
6. Extend docs contract checks beyond action inputs.

## Notes
- Evidence: `dist/landfall` is a Linux action binary; local macOS users should use Cargo or a local build.
- Evidence: dogfood found missing secrets and trigger ambiguity across downstream repos.
- Why: if first contact is slow or confusing, Landfall will not become the default release system.
