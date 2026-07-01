# Publish release binaries and stop committing dist

Priority: P1 · Status: pending · Estimate: L

## Goal
Make Landmark self-dogfood real release binaries and stop growing the checked-in
Linux action binary history.

## Oracle
- [ ] Landmark releases publish per-target binary assets and checksums.
- [ ] The GitHub Action bootstraps a pinned binary download or equally small
      packaging shim instead of requiring new `dist/landmark` commits.
- [ ] `dist/landmark` history disposition is documented as an explicit operator
      decision; no history rewrite occurs without direct authorization.
- [ ] Consumer action parity remains covered by hosted runner artifact
      comparison or its replacement.
- [ ] `bin/gate` passes.

## Children
1. Add release asset production for the supported target matrix.
2. Publish checksums and verify downloads in the self-release flow.
3. Change the action packaging path to consume pinned release assets.
4. Stop adding new `dist/landmark` binary commits.
5. Document the operator decision needed for any repository history rewrite.
6. Evaluate crates.io and Homebrew only after release assets are reliable.

## Notes
The groom teardown measured the packed Git history cost of repeatedly committing
the musl binary. That cost is real, but the rewrite decision is intentionally
not in scope for autonomous execution.
