# Publish release binaries and stop committing dist

Priority: P1 · Status: done · Estimate: L

## Goal
Make Landmark self-dogfood real release binaries and stop growing the checked-in
Linux action binary history.

## Oracle
- [x] Landmark releases publish per-target binary assets and checksums.
- [x] The GitHub Action bootstraps a pinned binary download or equally small
      packaging shim instead of requiring new `dist/landmark` commits.
- [x] After no supported path depends on the committed binary, `dist/` is purged
      from all Git history with `git filter-repo` and `master` is force-pushed
      under the operator's explicit 2026-07-01 authorization for this purge
      only.
- [x] The `v1` tag and any release tags needed by fleet consumers are
      re-created or re-pointed after the rewrite so `misty-step/landmark@v1`
      still resolves.
- [x] At least one consumer repo's Landmark workflow passes against the
      rewritten action/tag state.
- [x] Before/after repository pack size is reported; the groom baseline was
      187 MiB.
- [x] Consumer action parity remains covered by hosted runner artifact
      comparison or its replacement (release-time checksum-verified download
      replaces the old committed-binary byte-compare).
- [x] `bin/gate` passes.

## Children
1. [x] Add release asset production for the supported target matrix
   (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`,
   `aarch64-apple-darwin`, `x86_64-apple-darwin`) — `.github/workflows/release.yml`.
2. [x] Publish checksums and verify downloads in the self-release flow
   (`checksums.txt` release asset + action.yml bootstrap checksum verification).
3. [x] Change the action packaging path to consume pinned release assets
   (`action.yml` "Bootstrap Landmark binary" step).
4. [x] Stop adding new `dist/landmark` binary commits (`prepare-self-release`
   no longer builds/refreshes `dist/`; `dist/landmark` and `dist/landmark.sha256`
   removed from the tree and added to `.gitignore`).
5. [x] Run the authorized `dist/` history purge: `git filter-repo`, force-push
   `master`, re-point `v1` and release tags, and verify a consumer workflow.
6. [x] Report before/after pack size, using 187 MiB as the known pre-purge baseline.
7. Evaluate crates.io and Homebrew only after release assets are reliable
   (not done in this pass — filed as backlog follow-up, not a blocker).

## Evidence
- Real release `v1.26.0` shipped through `.github/workflows/release.yml`'s new
  `build-release-assets`/`publish-release-assets` jobs: all 4 target binaries
  plus `checksums.txt` are live GitHub Release assets, downloaded and
  checksum-verified successfully by `action.yml`'s bootstrap step in a real
  consumer run (misty-step/canary `landmark-release.yml` run 28555674024,
  both before and after the purge below).
- `git filter-repo --path dist --invert-paths --force` ran against a fresh
  clone; verified byte-identical file trees/content at HEAD and a mid-history
  commit (minus `dist/`) before pushing. Pushed with a temporary, fully
  restored branch-protection window (saved and diffed the protection JSON
  before/after — identical) using `--force-with-lease` with explicit pinned
  expected SHAs for `master` and all 44 tags (never a bare `--force`).
- Pack size (fresh clone, `.git` packed size): 38.52 MiB before → 1.14 MiB
  after (a working copy with accumulated local/reflog cruft measured 172.74
  MiB before — the groom's 187 MiB baseline is in that same ballpark; the
  fresh-clone number is the comparable, real-world "what a new clone costs"
  figure).
- Re-verified misty-step/canary's real `landmark-release.yml` a second time
  after the force-push; `misty-step/landmark@v1` still resolved and completed
  successfully post-rewrite.
- Found and fixed a real fallout risk before it could bite: misty-step/threshold
  pinned Landmark by exact SHA (`1379e56f...`), a commit that touches
  `dist/landmark` and would not have survived the rewrite. Re-pinned to the
  floating `@v1` tag (misty-step/threshold#29, merged) before running the purge.

## Notes
The groom teardown measured the packed Git history cost of repeatedly committing
the musl binary. Operator directive on 2026-07-01 explicitly authorizes this
single force-push sequence after release binaries and pinned action bootstrap
land: "force push, clean things up, better now than later." Never force-push
anything except this `dist/` purge.
