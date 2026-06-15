# Make first-run adoption work in sixty seconds

Priority: P1 · Status: pending · Estimate: XL

## Goal
Let a new human or agent get useful Landfall output quickly on macOS, Linux,
GitHub Actions, or arbitrary CI without first solving secrets, triggers, and
binary compatibility.

## Oracle
- [ ] A zero-secret local quickstart runs on macOS and Linux and writes a useful release preview/evidence packet from a checkout.
- [ ] README clearly separates local CLI, arbitrary CI, GitHub Action full mode, and GitHub Action synthesis-only mode.
- [ ] Installation instructions cover source build, checked-in action binary, and any packaged binaries without implying the Linux action binary runs on macOS.
- [ ] Setup-generated workflows have one blessed trigger strategy per release-tool family and do not duplicate manual-tag paths.
- [ ] Node 24 action-runtime verification is part of CI before relying on GitHub-hosted defaults.
- [ ] Docs consistency checks fail on stale command names, nonexistent files, bad model IDs, or mismatched examples.

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
