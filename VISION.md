# Landmark Vision

Status: Canonical root vision for Landmark. Revise when the release boundary,
artifact contract, or supported adoption modes materially change.

## What Landmark Is

Landmark is public open-source release intelligence for repositories with git
history and, when available, conventional commits. It can run as a GitHub
Action, but the product boundary is the Rust CLI: local scripts, generic CI,
and agents should all be able to ask the same runtime what changed, what
version follows, what evidence exists, which release artifacts should be
produced, and whether the resulting public release transaction is complete.

The job is not merely "make a changelog." Landmark's adopted product boundary
owns release truth:
classification, importance, version decisions, synthesis status, release-kit
plans, provenance, approval state, public release mutation, and
machine-readable evidence. Release judgment and release mutation are one
responsibility: Landmark should decide what becomes a release, reconcile the
public systems that make it a release, and record the completed result.
Judgment belongs at model-native seams: release classification and audience
importance should be evaluated from structured commits, diff statistics, and
release context by a BYOK model policy. Deterministic code still owns parsing,
version math, provider policy, persistence, approvals, mutation safety, and
schema contracts. GitHub, Slack, feeds, docs patches, blog drafts, screenshots,
videos, and other final-mile surfaces are adapters and producers around that
truth.

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

## Release Transaction Boundary

Landmark owns the complete release transaction, not only the decision that
precedes it. Its small public contract should eventually be equivalent to:

```text
publish(candidate release) -> completed release receipt
```

The depth behind that contract includes version and audience judgment,
artifact-policy validation, changelog and release-note production, repository
metadata updates, tags, public release records, stable-channel declarations,
and a durable receipt tying the result to its source revision and immutable
artifact identities. Because those mutations cross systems, publication is a
reconcilable transaction rather than a claim of distributed atomicity: retries
inspect existing state, finish missing compatible mutations, return the same
completed result when already applied, and fail closed on contradictions.

Artifact construction and deployment remain outside this boundary. A product's
build pipeline constructs, signs, and publishes its executable artifacts before
Landmark declares the release stable. Landmark validates the supplied artifact
manifest and records its immutable identities; it does not rebuild a product's
container, package, or binary. Deployment systems consume only completed
release receipts, treat forge or webhook events as wake-up signals, and own
environment-specific desired state, promotion, health verification, rollback,
and convergence. Landmark does not deploy releases or assert that an
environment is current.

**Implementation status (2026-07-13):** this is the accepted target boundary,
not a claim about the current output contract. Today's GitHub Action and CLI
perform several release mutations and emit separate evidence/status outputs,
but they do not yet expose one reconciled cross-system transaction or completed
release receipt. That implementation belongs with the Rust-owned full-release
work tracked in Powder; until it lands, consumers must not infer receipt
authority from an existing tag, event, or synthesis-status output.

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
- Public release mutation is part of release intelligence. Mutating paths must
  be idempotent, inspect-before-write, resumable after partial failure, and
  fail closed when existing public state contradicts the release decision.
- Under the target contract, a candidate is not a stable release until every
  required public mutation is reconciled and Landmark emits its completed
  release receipt. Downstream consumers follow that receipt; event delivery
  alone is not release truth.
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
- Declaring a release complete from an event, tag, or partially mutated public
  state without a reconciled completion receipt.
- Making every consumer repo or agent reinvent changelog, release-note,
  semantic-version, and release-evidence logic.
- Monetization-first product shaping. Landmark may build credibility and
  leverage, but near-term decisions should optimize usefulness, adoption, and
  ecosystem defaulting rather than revenue capture.
- Embedding brand design, demo media production, CMS publication, or long-lived
  creative pipelines inside the release-intelligence runtime. Rich media output
  belongs behind typed producer contracts.
- Building product executables or owning environment-specific deployment,
  promotion, health, rollback, or convergence policy.
- Weakening `bin/check-architecture` or schema checks to ship a feature.

## Current Bets

1. Keep the CLI boundary sharp while the GitHub Action remains the easiest
   adoption path.
2. Make Landmark the default release layer for Misty Step projects before
   asking strangers to trust it.
3. Make `release-kit` the durable artifact graph for richer release workflows:
   docs patches, blog drafts, screenshots, demo GIFs, videos, and other
   intelligent release artifacts.
4. Treat schema registry coverage as part of the product, especially for
   agents.
5. Prefer portable local and generic-CI modes over GitHub-only affordances.
6. Turn every failure class into a stable envelope with a clear `user_action`.
7. Back version decisions with a second, independent signal beyond commit
   intent — diff-grounded semver evidence (e.g. `cargo-semver-checks` for Rust
   consumers) reconciled against the conventional-commit bump, with a typed
   waiver for declared product intent that legitimately overrides both
   (backlog 005/002/009 — the differentiator arc, deliberately staged behind
   the bets below until it can build on settled ground).
8. Thin `action.yml`'s ~1,000 lines of embedded bash into one replayable Rust
   `landmark action-run` command, with `semantic-release` remaining only as an
   explicitly named compatibility path (backlog 007, then 010).
9. Make the fleet self-healing: detect drift between installed consumer
   workflows/manifests and current templates, and open the fix as a PR instead
   of letting copy-pasted integration rot silently (backlog 008).

### Recently landed
Model-native classification over parsed commits and diff statistics (with
conventional commits as the deterministic floor signal), one shared Rust
version-decision engine across every entry point (unknown commits are named,
never silently patched), and published per-target release binaries with the
`dist/` history purge — all three were open bets as of the 2026-07-01 groom;
treat this file, not that snapshot, as current.

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
