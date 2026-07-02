# Full GitHub mode v2: Rust owns analyze/version/changelog/tag/publish

Priority: P2 · Status: pending · Estimate: L

## Goal
Retire `semantic-release` as the engine behind GitHub Action full mode by
having Rust own the whole pipeline it currently runs: commit analysis,
version decision, changelog generation, tag creation, and release publish.

## Oracle
- [ ] Full mode can complete an entire release (analyze, version, changelog,
      tag, GitHub Release) without invoking `npx semantic-release` or
      installing Node, for repos that opt into the Rust-owned path.
- [ ] The existing `semantic-release` path remains available and unchanged as
      an explicitly named compatibility mode for repos that depend on its
      plugin ecosystem (e.g. `@semantic-release/exec` custom steps).
- [ ] The Rust path reuses `crates/landmark/src/version_decision.rs`
      (`classify_commit`/`decide_version`) as its version-decision engine —
      no third implementation.
- [ ] Replay coverage exists for the new Rust-owned full-mode path at parity
      with the existing `consumer_full_mode_success` semantic-release scenario.
- [ ] `bin/gate` passes.

## Children
1. Design the Rust-owned changelog/tag/publish sequence (what
   `@semantic-release/changelog`, `@semantic-release/git`, and
   `@semantic-release/github` currently do) as adapter-seamed Rust code.
2. Add a `full-mode` selector (e.g. manifest `release.engine: rust|semantic-release`)
   defaulting to the existing `semantic-release` path so no consumer breaks.
3. Port the release commit / tag creation flow using the GitHub provider
   already used elsewhere in the runtime.
4. Add replay coverage for the Rust-owned path.
5. Once the Rust path has fleet parity, revisit whether `semantic-release`
   stays as a documented compatibility mode indefinitely or gets a
   deprecation timeline (operator decision, not this ticket's call).

## Notes
This is backlog 004's children 4/5, split out as its own ticket once 004
landed the shared version-decision engine both paths must agree with.
Related to `007-thin-github-action-into-rust-action-run.md` (that ticket
thins the *post-release* synthesis/publication pipeline into Rust; this one
is the *pre-release* semantic-release job). Do this only after 007 lands, so
the action isn't getting thinned around two different decision cores at once.
