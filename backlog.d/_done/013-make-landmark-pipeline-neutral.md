# Make Landmark pipeline-neutral with GitHub as one adapter

Priority: P0 · Status: done · Estimate: XL

## Goal
Let any release trigger or CI pipeline run Landmark through provider-neutral
CLI primitives while preserving the GitHub Action as a thin, excellent wrapper.

## Oracle
- [x] A local shell pipeline can run from a git checkout with no GitHub token and produce version decision, technical changelog, public notes, artifacts, RSS/feed updates, and machine-readable status.
- [x] The GitHub Action path calls the same Rust CLI primitives as the local pipeline instead of owning separate release logic in shell.
- [x] GitHub release lookup, release mutation, PR extraction, failure issues, and fleet discovery are isolated behind explicit provider boundaries.
- [x] A checked replay scenario proves both `provider=local` and `provider=github` for the same fixture release.
- [x] README includes a non-GitHub quickstart and the old GitHub Action quickstart still passes contract checks.

## Children
1. Define the release engine data model: release source, version decision, changelog source, synthesized notes, publication destinations, provider status, and evidence packet.
2. Add a `local` provider that reads git tags/commits/changelog files and writes artifacts/status without calling a forge.
3. Extract GitHub-specific REST calls behind a narrow provider adapter instead of direct command-local `curl_json` use.
4. Promote git-range changelog generation from `backfill` into the normal release source path.
5. Split publication sinks into local artifacts, feed, webhook/Slack, and provider release-body mutation.
6. Add `landmark run` or equivalent orchestration command that can be called from shell, GitHub Actions, GitLab CI, Forgejo, or an agent.
7. Update the GitHub Action to invoke the provider-neutral command with `provider=github`.

## Notes
- Evidence: `action.yml` currently makes `github-token` required and owns GitHub-specific shell flow.
- Evidence: `backfill` already has git-tag/range logic that can power tokenless local mode.
- Evidence: provider-specific commands include `fetch-release-body`, `extract-prs`, `update-release`, failure issue lifecycle, and `fleet`.
- Why: the user wants custom triggers and pipelines; the architecture should make that normal rather than a workaround.

## Delivery
- Added `landmark run` with `provider=local` and `provider=github`, local git-range version decisions, technical changelog generation, public notes/artifacts, RSS/feed output, evidence JSON, optional notes-file input, and explicit GitHub publication.
- Routed GitHub release lookup/mutation, PR listing, failure issue lifecycle, and fleet metadata through `GitHubProvider`.
- Updated the composite action so release-body mutation, release artifact writes, and RSS feed writes use `landmark run`; the static action replay rejects drift back to `update-release`, `write-artifacts`, or `update-feed` for those paths.
- Verification: `cargo run --locked -- replay-action --scenario action_static_contract --scenario local_provider_run --scenario github_provider_run --scenario provider_run_parity --evidence-dir .landmark/replay-013-parity`; `bin/gate`.
- Evidence packet: `.landmark/replay/replay-result.json` includes passing `provider_run_parity` with `provider=local` and `provider=github` for `v1.1.0`.
