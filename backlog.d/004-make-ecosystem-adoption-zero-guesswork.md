# Make ecosystem adoption zero-guesswork

Priority: P2 · Status: done · Estimate: XL

## Goal
Let a repository adopt Landfall by answering observable setup questions instead of hand-selecting release modes, triggers, secrets, and changelog sources.

## Oracle
- [x] A dry-run setup command or workflow diagnoses the repo's release tool, branch, tag format, required secrets, conventional-commit readiness, and best Landfall mode.
- [x] The tool emits a ready-to-commit workflow for semantic-release, release-please, changesets, and manual-tag repos.
- [x] Monorepo and multi-package repos get first-class outputs instead of commented example snippets.
- [x] The generated workflow includes a healthcheck and explains required permissions before the first release attempt.
- [x] `scripts/backfill.py` is either documented as a first-class repair CLI or removed from the maintenance surface.

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

## Delivery
- Added `landfall setup --repo-root . --output-dir .landfall/setup` as the dry-run adoption analyzer and workflow generator.
- The analyzer reports detected release tool, default branch, tag format, required secrets, permissions, conventional-commit readiness, package topology, monorepo status, recommendation, and backfill retirement.
- Generated workflow candidates cover semantic-release full mode, release-please synthesis-only mode, changesets synthesis-only mode, changesets monorepo matrix mode, and manual-tag synthesis-only mode.
- Updated the healthcheck example to use Landfall directly instead of removed Python scripts.
- Added `examples/changesets-monorepo.yml` as a first-class multi-package workflow.
- Verification: `bin/gate`; `cargo test --locked` with 9 tests including setup detection and generated-workflow YAML parsing; `cargo run --locked -- setup --repo-root . --output-dir .landfall/setup`; `shasum -a 256 -c dist/landfall.sha256`.
