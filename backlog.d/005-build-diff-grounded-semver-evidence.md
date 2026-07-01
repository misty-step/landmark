# Build diff-grounded semver evidence

Priority: P1 · Status: pending · Estimate: L

## Goal
Back version decisions with changed-public-API evidence and typed waivers so
Landmark can distinguish inferred intent, declared intent, and actual breakage.

## Oracle
- [ ] Rust repositories can attach `cargo-semver-checks` or equivalent public
      API evidence to run evidence.
- [ ] Commit-derived bump, diff-derived bump, and optional policy override are
      reconciled with explicit agreement, disagreement, or waiver status.
- [ ] `backlog.d/002-add-version-intent-policy-override.md` lands as the typed
      waiver half of the reconciliation story.
- [ ] `landmark preview` or an equivalent PR-comment mode shows the would-be
      version, decisive evidence, and draft note before publication.
- [ ] `bin/gate` passes.

## Children
1. Add changed-public-symbol evidence for Rust crates through
   `cargo-semver-checks` or a documented adapter seam.
2. Reconcile conventional intent, diff evidence, and explicit version-intent
   policy in run evidence.
3. Land the version-intent override ticket as a typed waiver.
4. Add a preview surface suitable for PR comments before any release mutation.
5. Keep cross-language symbol extraction parked until the Rust path proves the
   evidence contract.

## Notes
This is the differentiator arc, but it stays behind `003` and `004` so it does
not build trust claims on top of keyword classification or split version truth.
