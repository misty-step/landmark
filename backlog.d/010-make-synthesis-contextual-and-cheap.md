# Make synthesis contextual and cheap

Priority: P1 · Status: done · Estimate: XL

## Goal
Generate richer public release notes without making every release pay for large diff context or a frontier model call.

## Oracle
- [x] `dist/landfall synthesize --dry-run-cost ...` reports estimated input tokens, output tokens, model tier, skip decision, and cost before any LLM call.
- [x] A deterministic release classifier identifies docs-only, chore-only, dependency-only, internal tooling, user-visible, breaking, security, and migration-heavy releases from commits, PRs, labels, paths, and manifest policy.
- [x] Model selection supports `cheap`, `balanced`, `rich`, and `off` policies with replay coverage for skip, cheap model, fallback, and rich escalation paths.
- [x] `changelog-source: prs` is evaluated against real or fixture PR metadata before any durable per-PR cache is introduced.
- [x] The `synthesis-status` output includes cost metadata and the final context sources used.
- [x] `bin/gate` proves cost policy without network calls through fake LLM and fake GitHub endpoints.

## Children
1. Add token and cost estimation for changelog, release-body, PR metadata, manifest context, and prompt template inputs.
2. Build a deterministic release significance classifier that can skip or downshift synthesis for low-value releases.
3. Introduce model policy fields in `.landfall.yml` and action inputs only where the manifest cannot cover the use case.
4. Improve PR metadata extraction and measure whether PR summaries already provide enough context before adding durable per-PR caching.
5. Add a structured context packet passed to the LLM: product manifest, technical changelog, PR summaries, breaking-change candidates, changed public surfaces, and artifact targets.
6. Emit cost and context telemetry in `synthesis-status` and replay evidence.

## Notes
- Evidence: current defaults use `anthropic/claude-sonnet-4` with fallback models, and synthesis runs whenever `synthesis: "true"` and a release exists.
- Evidence: the action already has `changelog-source: auto|changelog|release-body|prs`, typed `synthesis-status`, and fake endpoint replay, which are enough to add cost policy without new infrastructure.
- Risk: per-PR summary caching adds durable state and invalidation complexity on ephemeral GitHub runners; defer it until PR-source measurement shows release-time context is insufficient.
- Why: the user explicitly wants high-context notes without paying frontier-model costs on every changelog, and both critics warned against unbounded per-release context stuffing.
