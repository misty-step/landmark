# Unify classification and synthesis grounding

Priority: P1 · Status: pending · Estimate: M

## Goal
Classification and synthesis currently derive "what happened in this release"
from two independent paths that can silently diverge. Collapse them into one
grounding source so a bug in one can't feed confidently wrong content through
the other.

## Context
`~/.factory-lanes/wave1/landmark-model-audit.md` (2026-07-02 model usage audit)
found that classification (`classify_release_context_with_deterministic`)
consumes `deterministic.commits`/`diff_stats` — already correctly scoped to
`previous_tag..target` via `context_git_range`. Synthesis instead resolves its
technical changelog independently via `resolve_technical_changelog`, which
tries `CHANGELOG.md` section extraction, then a fetched GitHub release body,
then extracted PRs — none of which are cross-checked against the commit range
classification already computed. Two production incidents traced directly to
this divergence:

- bitterblossom v1.79.0: no `CHANGELOG.md`, `extract-prs` fetched an unfiltered
  "last 100 closed PRs" page unrelated to the release, and synthesis
  faithfully turned it into ~19 fabricated feature bullets for a one-commit
  release.
- canary v1.6.0/v1.7.0: `CHANGELOG.md` existed but was stale;
  `extract_release_section` silently fell back to the wrong (most recent)
  section, and synthesis produced fluent, confidently wrong notes describing
  neither shipped feature.

The immediate bugs are fixed (PRs scoped to the release tag range;
`extract_release_section` fails closed instead of guessing), but the
structural gap — synthesis and classification trusting different "what
happened" sources — remains and can recur in a new form.

## Oracle
- [ ] Synthesis's technical-changelog resolution and classification's
      deterministic context are demonstrably sourced from the same
      range-scoped evidence (or synthesis explicitly cross-checks its
      resolved source against `deterministic.commits` and fails/degrades on
      material mismatch).
- [ ] A regression fixture proves that if `resolve_technical_changelog`'s
      chosen source disagrees with `deterministic.commits` (e.g. commit count
      or subject keywords), synthesis does not silently proceed with the
      mismatched source.
- [ ] `bin/gate` passes.

## Children
1. Decide the unification shape: either (a) synthesis derives its technical
   changelog text directly from `deterministic.commits` when no explicit
   `changelog-source` override is set, or (b) synthesis keeps its existing
   source order but validates the resolved text against
   `deterministic.commits` (e.g. commit-count/keyword sanity check) before
   using it, degrading loudly on mismatch.
2. Land whichever shape as the default for `changelog-source: auto`; keep
   explicit `changelog`/`release-body`/`prs` overrides working for repos that
   deliberately want one source.
3. Fold in the audit's cheap belt-and-suspenders mitigation: explicit
   grounding language in the synthesis prompt telling the model to prefer the
   commit list if the supplied changelog text looks inconsistent with it.
4. Cross-link with `backlog.d/005-build-diff-grounded-semver-evidence.md`,
   which names the same "ground the model in the real diff" principle for
   version-bump decisions.

## Notes
Filed from the 2026-07-02 model usage audit alongside the P0 fixes for
`extract_prs` range scoping and `extract_release_section`'s silent fallback.
Those fixes close the two specific incidents; this ticket is the structural
follow-up the audit recommended as the highest-leverage remaining risk.
