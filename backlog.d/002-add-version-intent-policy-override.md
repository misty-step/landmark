# Add version-intent policy override

Priority: P2 · Status: pending · Estimate: S

## Goal
Let a repo or release run declare product version intent so Landmark does not
under-bump initial, rename, or bootstrap releases when the commit history lacks
`feat!` or `BREAKING CHANGE` markers.

## Oracle
- [ ] Manifest or CLI policy can request an explicit version intent such as
      `initial`, `major`, `minor`, or `patch` without weakening conventional
      commit defaults.
- [ ] The version decision evidence records both the commit-derived bump and the
      policy override reason.
- [ ] A product rename/bootstrap fixture proves Landmark can select `v1.0.0` or
      a major bump from policy while preserving normal semver behavior for
      ordinary commits.
- [ ] `bin/gate` passes after implementation.

## Notes
Quality sanity on Threshold showed the issue: the explicit `v1.0.0` release was
product-correct, but commit-derived semver saw the rename range as `patch`.
