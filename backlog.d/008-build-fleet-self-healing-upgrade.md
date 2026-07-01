# Build fleet self-healing upgrades

Priority: P2 · Status: pending · Estimate: M

## Goal
Let Landmark re-render drifted fleet workflows and manifests, open focused PRs,
and reduce copy-paste release integration risk across Misty Step repos.

## Oracle
- [ ] A `fleet upgrade` or equivalent command detects Landmark workflow and
      manifest drift against the current templates.
- [ ] The command renders patch records and PR bodies without reading or
      exposing secret values.
- [ ] Existing release-please, changesets, semantic-release, synthesis-only,
      and manifest-only modes keep their adapter-specific rules.
- [ ] Prompt templates vary release-note voice enough to avoid the repeated
      "You can now" fingerprint in generated output.
- [ ] `bin/gate` passes.

## Children
1. Add drift detection for installed Landmark workflow and manifest surfaces.
2. Reuse existing fleet PR rendering and safety blockers for upgrade PRs.
3. Add replay scenarios for representative consumer modes.
4. Adjust synthesis prompting to vary sentence openings by audience and content.
5. Dogfood on the factory repos before expanding to broader org scans.

## Notes
The fleet integration footprint is intentionally small; this ticket hardens the
template-update path without turning Landmark into the fleet orchestrator.
