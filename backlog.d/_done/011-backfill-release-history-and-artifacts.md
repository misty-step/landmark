# Backfill release history and artifacts

Priority: P1 · Status: done · Estimate: L

## Goal
Let mature repositories adopt Landmark with useful historical changelog and release-note artifacts instead of starting from a blank release feed.

## Oracle
- [x] `dist/landmark backfill --repo-root . --since <tag> --dry-run` plans historical release artifacts from existing tags, GitHub Releases, changelog entries, and PR metadata without mutating releases.
- [x] `dist/landmark backfill --mode artifacts-only` writes markdown, plaintext, HTML, JSON, and RSS-compatible artifacts for prior releases through the same typed artifact model used by normal synthesis.
- [x] `dist/landmark backfill --mode release-body --dry-run` previews GitHub Release body updates and refuses ambiguous or duplicate release mappings.
- [x] Replay fixtures cover missing releases, missing tags, prereleases, monorepo package tags, private repositories, and already-Landmark-managed releases.
- [x] Backfill output includes a resumable manifest of processed tags, skipped tags, estimated cost, and remaining work.

## Children
1. Reintroduce backfill as a Rust-owned command, not a standalone Python maintenance surface.
2. Resolve historical release sources in priority order: existing GitHub Release body, `CHANGELOG.md`, tag ranges, merged PRs, and manifest-provided product context.
3. Support artifact-only backfill as the default safe path, with release-body mutation gated behind preview and explicit confirmation.
4. Add cost-aware batching, resume files, and per-tag skip reasons.
5. Integrate backfill recommendations into `landmark setup` and `landmark fleet plan`.

## Notes
- Evidence: README currently says the old Python backfill script is retired and repair should use one synthesis-only run per tag.
- Evidence: typed markdown/plaintext/HTML/JSON/RSS artifacts already exist, so historical adoption can reuse the artifact plane instead of creating a new format.
- Why: fleet adoption will include mature repositories with existing tags and releases; a no-brainer release system needs to migrate history, not just future releases.
