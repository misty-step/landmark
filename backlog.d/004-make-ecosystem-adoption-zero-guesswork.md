# Make ecosystem adoption zero-guesswork

Priority: P2 · Status: pending · Estimate: XL

## Goal
Let a repository adopt Landfall by answering observable setup questions instead of hand-selecting release modes, triggers, secrets, and changelog sources.

## Oracle
- [ ] A dry-run setup command or workflow diagnoses the repo's release tool, branch, tag format, required secrets, conventional-commit readiness, and best Landfall mode.
- [ ] The tool emits a ready-to-commit workflow for semantic-release, release-please, changesets, and manual-tag repos.
- [ ] Monorepo and multi-package repos get first-class outputs instead of commented example snippets.
- [ ] The generated workflow includes a healthcheck and explains required permissions before the first release attempt.
- [ ] `scripts/backfill.py` is either documented as a first-class repair CLI or removed from the maintenance surface.

## Children
1. Add a read-only setup analyzer that inspects repository files, tags, existing release workflows, and package metadata.
2. Generate candidate workflow files for full mode and synthesis-only mode with a machine-readable rationale.
3. Promote monorepo/multi-package synthesis from a commented `examples/changesets.yml` sketch to a supported scenario.
4. Add PR preview or dry-run note synthesis so teams can inspect output before wiring production releases.
5. Graduate `scripts/backfill.py` into the setup/repair workflow, or delete it with tests and docs adjusted.
6. Feed analyzer findings back into README onboarding and examples.

## Notes
- Evidence: `action.yml` requires explicit `mode`; examples split `release-please`, `changesets`, and manual tags into separate files.
- Evidence: `examples/changesets.yml` keeps the multi-package variant as a commented matrix sketch.
- Evidence: `examples/healthcheck.yml` downloads scripts with `curl` and installs unpinned `requests`, so even the diagnostic example has reliability and supply-chain gaps.
- Evidence: `scripts/backfill.py` is a large standalone repair surface, but current project context does not mention it as an adoption or operations workflow.
- Why: product/adoption review found Landfall is powerful but still asks consumers to know too much about GitHub Actions and their release topology.
