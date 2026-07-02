# Harden residual PR-extraction and synthesis-idempotency gaps

Priority: P2 · Status: pending · Estimate: S

## Goal
Close two smaller gaps the 2026-07-02 model usage audit surfaced alongside the
two P0 grounding bugs (`extract_prs` range scoping, `extract_release_section`
silent fallback), neither severe enough to block those fixes but both real
residual risk.

## Context
`~/.factory-lanes/wave1/landmark-model-audit.md`:

1. **`closed_pull_requests` has no pagination.** It fetches a single page
   (`per_page=100`) of closed PRs, now filtered client-side to the release's
   tag range by `filter_prs_by_range`. For a release with more than 100
   closed PRs merged since the previous tag, in-range PRs merged earlier than
   GitHub's first 100 (sorted by default, which is creation order — not
   necessarily merge order) could still be silently dropped from the
   changelog, understating the release rather than fabricating content. Lower
   severity than the unfiltered-fetch bug already fixed, but real for
   high-throughput repos.
2. **canary's duplicate classification-notice sections.** The audit observed
   "Bug Fixes" and the classification-notice blockquote appearing twice,
   worded slightly differently, in the same release body for both v1.6.0 and
   v1.7.0. The audit's working theory is `synthesize`/`update_release` ran
   twice without body-replacement idempotency (a retry without a check for
   "did this already run"). Not confirmed — needs an actual look at the
   consuming workflow's retry behavior, not just the symptom.

## Oracle
- [ ] `closed_pull_requests` (or its caller) paginates until either the
      fetched PRs span past the release's `since` bound or GitHub returns no
      more pages, so in-range PRs are never dropped solely because they sat
      past page 1.
- [ ] The canary duplicate-section symptom is either root-caused (retry
      without idempotency, confirmed via the consuming workflow) and fixed,
      or explicitly ruled out with evidence and closed as a non-issue.
- [ ] `bin/gate` passes.

## Children
1. Add pagination to `closed_pull_requests`, bounded by the same
   `previous_tag` commit-date lower bound `extract_prs` already computes —
   stop paginating once a page's oldest PR is older than `since`.
2. Investigate canary's actual release workflow run history (or a synthetic
   repro) for a `synthesize`-then-`update_release` retry path that doesn't
   check whether the release body already contains synthesized notes before
   appending again.
3. If confirmed, make `update_release_body`/the consuming workflow idempotent
   (e.g. detect and replace rather than append on a rerun).

## Notes
Both items are secondary to `backlog.d/011-unify-classification-and-synthesis-grounding.md`
— that ticket addresses the structural cause; this one mops up two smaller,
independent hardening gaps the same audit pass turned up.
