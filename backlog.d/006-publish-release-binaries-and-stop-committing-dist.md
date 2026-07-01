# Publish release binaries and stop committing dist

Priority: P1 · Status: pending · Estimate: L

## Goal
Make Landmark self-dogfood real release binaries and stop growing the checked-in
Linux action binary history.

## Oracle
- [ ] Landmark releases publish per-target binary assets and checksums.
- [ ] The GitHub Action bootstraps a pinned binary download or equally small
      packaging shim instead of requiring new `dist/landmark` commits.
- [ ] After no supported path depends on the committed binary, `dist/` is purged
      from all Git history with `git filter-repo` and `master` is force-pushed
      under the operator's explicit 2026-07-01 authorization for this purge
      only.
- [ ] The `v1` tag and any release tags needed by fleet consumers are
      re-created or re-pointed after the rewrite so `misty-step/landmark@v1`
      still resolves.
- [ ] At least one consumer repo's Landmark workflow passes against the
      rewritten action/tag state.
- [ ] Before/after repository pack size is reported; the groom baseline was
      187 MiB.
- [ ] Consumer action parity remains covered by hosted runner artifact
      comparison or its replacement.
- [ ] `bin/gate` passes.

## Children
1. Add release asset production for the supported target matrix.
2. Publish checksums and verify downloads in the self-release flow.
3. Change the action packaging path to consume pinned release assets.
4. Stop adding new `dist/landmark` binary commits.
5. Run the authorized `dist/` history purge: `git filter-repo`, force-push
   `master`, re-point `v1` and release tags, and verify a consumer workflow.
6. Report before/after pack size, using 187 MiB as the known pre-purge baseline.
7. Evaluate crates.io and Homebrew only after release assets are reliable.

## Notes
The groom teardown measured the packed Git history cost of repeatedly committing
the musl binary. Operator directive on 2026-07-01 explicitly authorizes this
single force-push sequence after release binaries and pinned action bootstrap
land: "force push, clean things up, better now than later." Never force-push
anything except this `dist/` purge.
