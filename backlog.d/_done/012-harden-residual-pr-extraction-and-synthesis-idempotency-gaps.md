# Harden residual PR-extraction and synthesis-idempotency gaps

Priority: P2 · Status: done · Estimate: S

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
- [x] `closed_pull_requests` (or its caller) paginates until either the
      fetched PRs span past the release's `since` bound or GitHub returns no
      more pages, so in-range PRs are never dropped solely because they sat
      past page 1.
- [x] The canary duplicate-section symptom is either root-caused (retry
      without idempotency, confirmed via the consuming workflow) and fixed,
      or explicitly ruled out with evidence and closed as a non-issue.
- [x] `bin/gate` passes.

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

## Delivered
- Confirmed the duplicate-section mechanism directly from `gh release view`
  on canary v1.6.0 and v1.7.1: not a retry. Canary wires two separate GitHub
  Actions workflows to synthesize notes for the same tag —
  `release.yml` (full mode, triggered by CI completing on master) creates the
  release and synthesizes it in one step; that release creation fires a
  `release: published` webhook that separately triggers
  `landmark-release.yml` (synthesis-only mode), which synthesizes the same
  tag again. Two independently-worded syntheses land on one release.
- Root cause in Landmark itself: `compose_release_body`/`strip_existing_whats_new`
  found the end of a prior "What's New" section by scanning for the next
  `## ` heading — but synthesized notes routinely carry their own `## Bug
  Fixes`/`## Features` subheading, so a second compose mistook the first
  run's subheading for the section boundary and left the rest of that run's
  content behind for the new notes to stack on top of.
- Fixed by bounding the synthesized block with explicit `<!--
  landmark:whats-new:start/end -->` sentinel comments (invisible on GitHub)
  instead of heading-boundary heuristics, so a second compose replaces the
  entire prior block exactly regardless of what the notes contain. Bodies
  composed before this fix ships fall back to the old heading heuristic once,
  then self-heal to the marker-bounded form. Moved `update_release`,
  `compose_release_body`, and `strip_existing_whats_new` into a new
  `release_body.rs` module (kept `synthesis.rs` under the 1200-line
  architecture ratchet).
- `closed_pull_requests` now paginates past `per_page=100`, stopping once a
  page's oldest PR (by `created_at`, GitHub's default sort) falls before the
  release's `since` bound or a page comes back short — capped at 10 pages.
  `extract_prs` threads its already-computed `since` through.
- Verified with two new subprocess-level replay scenarios (fake-GitHub-server
  convention from `pr_scoping.rs`):
  `release_body_idempotent_across_reruns` (two `update-release` calls on one
  release converge on the latest notes, footer preserved) and
  `pr_fetch_paginates_past_first_page` (an in-range PR seeded onto page 2
  survives). Both fail against the pre-fix code and pass after; confirmed by
  temporarily stashing the fix and rerunning `replay-action`.
- `cargo test`, `cargo clippy -D warnings`, `cargo fmt --check`,
  `bin/check-architecture`, and `bin/gate` all green.
