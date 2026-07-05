# Landmark DESIGN.md

This file is the product's public-site brand contract. Keep it short and exact:
agents and humans should be able to update `site/` from this file without
inventing a second design system.

## Brand Voice

- Plain-spoken, concrete, and operator-facing — match the tone of Landmark's
  own README: no marketing fog, no mascot language.
- Lead with the mechanism (what the CLI actually computes and writes), then
  the proof (real commit ranges, real hashes, real evidence files).
- State refusals plainly. Landmark's fabrication-refusal behavior is a trust
  feature, not a caveat — say so directly instead of softening it.

## Pitch One-Liner

`Landmark turns a repo's conventional commits into a version decision, a changelog, and public release notes it can prove — and refuses to write a section it can't ground in a real commit.`

## Lucide Mark

- Icon: `signpost`
- Reason: a signpost marks a waypoint and a direction at a fork in the road —
  that is exactly what Landmark computes at each release: the next version
  number, decided from real signals, marking where the repo's history stands.
  Checked `docs/`, `README.md`, `action.yml`, and the CLI output for an
  existing mark first; found none, so this is a fresh pick, not a reuse.

## Palette Hooks

Pinned `data-ae-theme="ember"` — deliberately distinct from Powder's
ultramarine blue, since both sites can be viewed side by side in the fleet.
Ember's warm orange reads as "release signal fired," which fits a tool whose
entire job is deciding when to ship.

```css
:root {
  --ae-accent: #c2410c;
  --ae-accent-dark: #ff8a5c;
}
```

No additional categorical project tokens were needed; the theme preset covers
the site's needs.

## Screenshot Inventory

No live UI exists to screenshot — Landmark's product boundary is a Rust CLI,
so the gallery uses real terminal-output captures instead of screenshots, per
the showcase contract for CLI products.

| File                                      | Surface                        | State                                                                          | Caption                                                                                                    |
| ------------------------------------------ | ------------------------------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `site/assets/screenshots/01-overview.svg` | Terminal: `landmark run --dry-run` | Real dry-run against this repo's own history (`v1.27.0..HEAD`, 8 commits)      | The version decision: minor bump to v1.28.0, decisive commit, and why — computed live, not staged.        |
| `site/assets/screenshots/02-workflow.svg` | Terminal: real run output       | Real (non-dry-run) execution that wrote `docs/releases/v1.28.0.md` and friends | The release-kit plan: which artifacts are Landmark-owned vs. pending operator review, with real approval gates. |
| `site/assets/screenshots/03-release.svg`  | Generated file: `v1.28.0.md`     | Actual markdown this run wrote to `docs/releases/`                             | The grounded public release note itself — six bullets, each tied to a real commit in this repo.            |

All three source files are checked into this branch under
`site/assets/screenshots/*.svg` as text-rendered terminal/file captures (not
literal screenshots) built from the actual command output captured during
site build; see the campaign receipt for the raw transcripts.

## Footer Links

- Misty Step: `https://mistystep.io`
- GitHub: `https://github.com/misty-step/landmark` (repo is public)
- Weave: omitted — Landmark is a standalone Misty Step fleet product, not a
  Weave-family surface; no evidence in the repo ties it to Weave.

## Release Notes Rule

`site/changelog.html` is user-facing. Write entries as product outcomes, not
commit logs. Each entry needs a date, a version or release label, and one or
two plain-language bullets. The current entry is sourced directly from a real
`landmark run --provider local` execution against this repo (v1.27.0 →
v1.28.0), not invented.
