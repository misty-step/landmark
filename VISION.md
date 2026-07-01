# Landmark Vision

Status: Canonical root vision for Landmark. Revise when the release boundary,
artifact contract, or supported adoption modes materially change.

## What Landmark Is

Landmark is public open-source release intelligence for repositories with git
history and, when available, conventional commits. It can run as a GitHub
Action, but the product boundary is the Rust CLI: local scripts, generic CI,
and agents should all be able to ask the same runtime what changed, what
version follows, what evidence exists, and which release artifacts should be
produced.

The job is not merely "make a changelog." Landmark owns release truth:
classification, importance, version decisions, synthesis status, release-kit
plans, provenance, approval state, and machine-readable evidence. Judgment
belongs at model-native seams: release classification and audience importance
should be evaluated from structured commits, diff statistics, and release
context by a BYOK model policy. Deterministic code still owns parsing,
version math, provider policy, persistence, approvals, and schema contracts.
GitHub, Slack, feeds, docs patches, blog drafts, screenshots, videos, and
other final-mile surfaces are adapters and producers around that truth.

Landmark is also a Misty Step ecosystem primitive. Every active Misty Step
project should be able to adopt it so good release hygiene becomes automatic:
semantic versioning, technical changelogs, user-facing release notes, release
evidence, and richer release artifacts should not be reimagined by every agent
in every repository.

## North Star

A maintainer or cold agent can preview a release locally, inspect the evidence,
replay the action contract, and then run the same decision in CI without
discovering hidden GitHub-only behavior. Changelogs, release notes, semantic
versioning, release evidence, feeds, and artifact plans should feel obvious and
programmatic. A release should be explainable before it mutates anything
remote, and disagreements between deterministic floor signals, model
classification, and publication policy should become visible alarms rather
than quiet skips.

## What Must Stay True

- The Rust CLI is the source of truth. `action.yml`, shell, and Node exist only
  at platform boundaries.
- Model-native classification is release intelligence, not a garnish. Parsed
  commits and diff statistics feed the classifier; rendered changelog prose is
  context, not the source of truth.
- Conventional-commit parsing is a deterministic floor signal. It may drive
  version math and catch obvious feature/fix/breaking signals, but it must not
  silently replace the product brain with substring heuristics.
- Version decisions must be deterministic, explainable, and singular. All
  entry points should use one Rust engine, and unknown release intent should
  refuse, warn, or require an explicit waiver rather than silently patching.
- Local preview is not optional. Every mutating path needs a dry-run,
  evidence, replay, or schema oracle that can be exercised before publication.
- Open-source adoption and internal dogfooding reinforce each other. Landmark
  should serve external maintainers while staying simple enough to install
  across Misty Step repos by default.
- Release-kit artifacts are the boundary between Landmark's release intelligence
  and downstream producers. Extend typed producer contracts before embedding a
  bespoke media or CMS workflow in the core runtime.
- Agent-native contracts are first-class. `landmark describe --json`,
  `--error-format json`, `schemas/`, and replay scenarios are public surfaces,
  not internal conveniences.
- Architecture ratchets are allowed to stop work. When module boundaries blur,
  split ownership or update the ratchet with an explicit architecture reason.
- Synthesis failures must be visible and actionable. Weak notes, stale model
  keys, and provider failures should produce evidence and operator action, not
  quiet degradation.

## What Landmark Refuses

- Shell orchestration as the durable product core.
- CI-only release behavior that cannot be reproduced locally.
- Keyword soup or substring matching masquerading as release intelligence.
- Opaque version bumps, hidden provider policy, or untyped artifact side
  effects.
- Silent "successful" skips when structured feature, fix, breaking, or model
  signals disagree with the skip decision.
- Making every consumer repo or agent reinvent changelog, release-note,
  semantic-version, and release-evidence logic.
- Monetization-first product shaping. Landmark may build credibility and
  leverage, but near-term decisions should optimize usefulness, adoption, and
  ecosystem defaulting rather than revenue capture.
- Embedding brand design, demo media production, CMS publication, or long-lived
  creative pipelines inside the release-intelligence runtime. Rich media output
  belongs behind typed producer contracts.
- Weakening `bin/check-architecture` or schema checks to ship a feature.

## Current Bets

1. Replace keyword classification with model-native classification over parsed
   commits and diff statistics, with conventional commits as the floor signal.
2. Collapse the three version-decision paths into one Rust engine before adding
   more release-intent policy.
3. Publish real release binaries and stop growing the checked-in `dist/`
   artifact practice; history rewrite remains an explicit operator decision.
4. Keep the CLI boundary sharp while the GitHub Action remains the easiest
   adoption path.
5. Make Landmark the default release layer for Misty Step projects before
   asking strangers to trust it.
6. Make `release-kit` the durable artifact graph for richer release workflows:
   docs patches, blog drafts, screenshots, demo GIFs, videos, and other
   intelligent release artifacts.
7. Treat schema registry coverage as part of the product, especially for
   agents.
8. Prefer portable local and generic-CI modes over GitHub-only affordances.
9. Turn every failure class into a stable envelope with a clear `user_action`.

## Where The Depth Lives

- `README.md` explains adoption modes, CLI preview, GitHub Action use, and
  agent-native contracts.
- `AGENTS.md` carries repo contracts, product boundaries, architecture rules,
  and gate expectations.
- `action.yml` is the composite GitHub Action wrapper and input/output contract.
- `docs/agent-integration.md` is the cold-start guide for agent adopters.
- `schemas/` is the checked registry for manifests, release context,
  release-kit, replay, fleet plans, evidence, and failure envelopes.
- `bin/gate` is the closeout gate; `bin/replay-action` is the release behavior
  oracle for action and runtime contract changes.
