# Build PR-based self-release under branch protection

Priority: P2 · Status: pending · Estimate: L

## Goal
Let Landfall dogfood automatic self-release without requiring direct pushes to
protected `master`.

## Oracle
- [ ] A release run can generate `CHANGELOG.md`, `package.json`, and
      `pyproject.toml` changes on a release branch.
- [ ] The generated release branch opens or updates a pull request whose checks
      include `merge-gate`.
- [ ] The workflow publishes the GitHub Release only after the generated release
      PR lands.
- [ ] A replay command proves the protected-branch path without mutating
      production releases.

## Notes
- Evidence: the `2026-06-11` `Release` run for commit
  `12441ec32afb35156e1c00eb62a3c32514919b8d` failed when
  `@semantic-release/git` tried to push `HEAD:master`; GitHub rejected it
  because `master` requires the `merge-gate` status check.
- Current mitigation: Landfall's own release workflow is manual so normal
  `master` pushes do not run a release job that cannot pass under current branch
  protection.
