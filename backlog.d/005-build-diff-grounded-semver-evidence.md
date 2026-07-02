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

**Scope widened 2026-07-02** (model usage audit,
`~/.factory-lanes/wave1/landmark-model-audit.md`): the "ground the model in the
real diff, not commit-subject text" principle this ticket names for version
*decisions* applies equally to synthesis *content*. Classification already
consumes range-scoped `deterministic.commits`/`diff_stats`; synthesis sources
its "what happened" facts from a separate, independently-resolved changelog
string (`resolve_technical_changelog`) that can diverge from what
classification saw. Two production incidents this audit found
(bitterblossom v1.79.0 unfiltered-PR leak, canary v1.6.0/v1.7.0 stale
changelog-section fallback) both stemmed from that divergence. The immediate
grounding bugs are fixed (`extract_prs` now scopes to the release tag range;
`extract_release_section` no longer silently falls back to the wrong
section), but the two independent resolution paths still exist. See
`backlog.d/011-unify-classification-and-synthesis-grounding.md` for the
sibling ticket that tracks collapsing them into one.
