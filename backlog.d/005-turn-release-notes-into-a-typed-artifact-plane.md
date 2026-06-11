# Turn release notes into a typed artifact plane

Priority: P2 · Status: done · Estimate: L

## Goal
Consolidate release-note rendering, notification payloads, and machine-readable diagnostics around typed note artifacts instead of parallel markdown parsers and ad hoc outputs.

## Oracle
- [x] A shared note model can render markdown, plaintext, HTML, Slack Block Kit text, RSS entries, webhook JSON, and stored JSON without duplicated parsing rules.
- [x] Unsafe link handling is implemented once and covered by shared tests.
- [x] Consumers can subscribe to a stable machine-readable synthesis status payload that includes quality, stage, model attempts, and publication destinations.
- [x] Failure issue creation/closure is either modeled as an explicit companion output channel or removed from the core action surface.
- [x] Existing outputs remain backward-compatible or have a documented deprecation path.

## Children
1. Introduce a typed release-note artifact module with sections, bullets, links, quality, source, and rendering adapters.
2. Migrate `write-artifacts.py`, `notify.py`, `notify-slack.py`, and `update-feed.py` to the shared model.
3. Emit a single JSON status artifact for synthesis and distribution outcomes.
4. Reframe `synthesis-failure-issue` as a typed optional destination, or split it into a documented companion action/script.
5. Consolidate GitHub API headers and release URL construction in a shared client helper.
6. Add compatibility tests for current markdown, HTML, plaintext, Slack, RSS, and webhook payloads.
7. Delete duplicated markdown rendering helpers once adapters prove parity.

## Notes
- Evidence: `scripts/notes_render.py` already renders markdown to plaintext and HTML, while `scripts/notify.py` keeps another markdown-to-plaintext/html implementation and `scripts/notify-slack.py` keeps a third Slack-specific parser.
- Evidence: `close-resolved-failures.py`, `report-synthesis-failure.py`, `extract-prs.py`, and `update-release.py` each build GitHub request headers separately despite sharing retry/logging helpers.
- Evidence: `action.yml` opens and closes synthesis failure issues from the core release action, expanding permissions and stateful side effects beyond release-note publishing.
- Evidence: product review found platform consumers need machine-readable synthesis health signals beyond warning text and simple booleans.
- Why: simplification and platform perspectives converge here: Landfall's value is the note artifact, but that artifact is not yet modeled deeply.

## Delivery
- Added `ReleaseNoteArtifact` as the shared Rust model for markdown, plaintext, HTML, Slack, RSS, webhook JSON, stored JSON, sections, bullets, and safe links.
- Added `SynthesisStatus` JSON as the `synthesis-status` action output with quality, failure stage/message, model attempts, and release body/artifact/RSS/webhook/Slack destination outcomes.
- Added `synthesis-failure-issue-action` as the explicit companion output channel for failure issue close/report/failure/skipped states.
- Preserved existing scalar outputs and stored JSON compatibility, including dedupe by both current `tag` and older `version` entries.
- Verification: `bin/gate`; `cargo test --locked` with 6 tests including typed artifact rendering and status JSON coverage; adversarial Grok review: no blockers; Claude QA: pass; `shasum -a 256 -c dist/landfall.sha256`.
