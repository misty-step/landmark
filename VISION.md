# Landmark Vision

Status: Canonical root vision for Landmark. Philosophy, principles, and
product direction only — no tickets, no dates, no schedules. Revise when the
release boundary, artifact contract, or supported adoption modes materially
change.

## What Landmark Is

Landmark is automated release intelligence for every software project. Given
a git repository — any language, any stack, any forge, any CI — it decides
the next semantic version, writes the technical changelog, and produces
user-facing release notes in the project's voice. A release stops being a
ritual someone performs and becomes a property the repository has.

Integration must be a no-brainer:

- The first useful result requires zero configuration and zero secrets:
  point Landmark at a checkout and it previews the version, changelog, and
  notes locally.
- Configuration exists only to express taste — product name, audience,
  voice, artifact destinations — never to make the basic path work.
- The GitHub Action is one packaging layer, not the product. The product
  boundary is the portable Rust CLI that local scripts, generic CI, and
  agents call directly.
- Landmark releases itself, visibly. A release tool whose own pipeline is
  stale has no credibility; its self-release is the storefront.

## Principles

**Stack universality.** Git history is the only hard requirement.
Conventional commits are the recommended floor signal; everything else —
ecosystem version evidence, package manifests, release pipelines — is an
optional provider behind a typed seam. No stack is second-class, and nothing
in the core may require a particular language runtime, forge, or CI system.

**One deterministic brain.** Version decisions are deterministic,
explainable, and singular. Every entry point uses the same Rust engine.
Unknown release intent is named and refused — never silently patched by
substring heuristics. Model judgment extends the deterministic floor; it
never replaces it.

**Models at the seams, commodity cost by default.** LLMs classify and
prose-write at explicit seams, with schema-constrained outputs, grounded in
structured commit data, and gated by fabrication checks. Default model policy
runs on commodity-cost models; premium models are an explicit, budgeted
choice, never a hidden default. Provider policy is portable BYOK. Pins are
reviewed on a clock so staleness is impossible to miss.

**Portable artifacts, explicit publication.** Release notes are portable
artifacts — Markdown, HTML, JSON, RSS — that can live anywhere: in the
repository, on a forge, or on a public release-notes surface. Publication to
any public destination is an explicit, approved decision recorded in the
release transaction — never a default. Content derived from a private source
stays private unless that release explicitly opts it in, and ambiguity about
destinations fails closed. Forge release records are one adapter, not the
only home.

**One release transaction.** `publish(candidate) -> completed receipt`.
Release judgment and public release mutation are one deep responsibility:
idempotent, inspect-before-write, resumable after partial failure, failing
closed on contradictions (ADR 0004). Event delivery is a wake-up signal,
never release truth. Artifact construction and deployment stay outside the
boundary.

**Explainable before mutation.** Every mutating path has a local dry-run,
evidence output, or replay oracle. A cold agent can reproduce the CI decision
locally before anything public changes.

**Visible disagreement.** When deterministic signals, model classification,
and publication policy disagree, the disagreement becomes a visible alarm in
the output — not a quiet skip.

**Agents are first-class callers.** `describe --json`, versioned schemas,
typed failure envelopes with `user_action`, and replay scenarios are public
contract, not internal convenience.

**Boring core, sharp boundaries.** Rust owns release truth. Shell, Node, and
YAML exist only at platform seams and must stay thin enough to delete.
Architecture ratchets may stop work; complexity must buy its way in.

**Voice is data.** Release notes speak in the project's voice because voice
is declared in the manifest — not because someone edited a prompt.

## What Landmark Refuses

- Shell orchestration or YAML as the durable product core.
- Keyword soup or substring matching masquerading as release intelligence.
- Premium-model prices for commodity work by default.
- Silent "successful" skips when structured signals, model judgment, or
  publication policy disagree.
- CI-only release behavior that cannot be reproduced locally.
- Declaring release truth from a tag, an event, or partially mutated public
  state.
- Weakening architecture ratchets, schema checks, or gates to ship a feature.
- Monetization-first product shaping. Landmark optimizes for usefulness,
  adoption, and ecosystem defaulting.
- Embedding brand design, demo media production, CMS publication, or
  long-lived creative pipelines inside the release-intelligence runtime.
- Building product executables or owning environment-specific deployment,
  promotion, health, rollback, or convergence policy.

## Direction

Themes Landmark is heading toward, in rough order of leverage. This section
names direction, not schedules or tickets.

1. **Thin the Action to CLI primitives.** The GitHub Action becomes a thin
   assembly of the same commands generic CI calls. No release logic lives
   only in embedded shell.
2. **Complete the receipt.** Emit the unified release-transaction receipt
   ADR 0004 defines so downstream systems follow one authority. Until it
   ships, consumers must not infer receipt authority from tags, events, or
   synthesis-status outputs.
3. **Freshness as a property.** Model pins and dependencies are reviewed on
   a clock and enforced by the gate. Staleness should be structurally
   impossible to miss, not something someone happens to notice.
4. **Diff-grounded semver evidence.** Reconcile the commit-intent bump with
   independent API-diff evidence, with a typed waiver for declared product
   intent that legitimately overrides both.
5. **Hosted public release notes.** Explore a hosted surface that renders
   Landmark's portable release-note artifacts at stable public URLs per
   project and version. Only artifacts explicitly published to it appear
   there; content from a private source reaches it solely through an
   explicit, approved publication decision. The surface is a deterministic
   consumer of the artifacts — never a second source of truth — and
   self-hosting remains fully supported.
6. **Release media as producer contracts.** Screenshots, GIF walkthroughs,
   and demo videos are typed release-kit artifacts produced by explicit
   producer adapters — local, browser, service, harness, or human — never
   bespoke media pipelines embedded in the core runtime. Commodity
   vision-capable models make this direction reachable; it stays behind the
   producer contract until the text pipeline is boring.
7. **Fleet self-healing.** Detect drift between installed consumer
   workflows, manifests, and current templates, and open the fix as a PR
   instead of letting copy-pasted integration rot silently.

## Where The Depth Lives

- `README.md` explains adoption modes, CLI preview, GitHub Action use, and
  agent-native contracts.
- `AGENTS.md` carries repo contracts, product boundaries, architecture
  rules, and gate expectations.
- `action.yml` is the composite GitHub Action wrapper and input/output
  contract.
- `docs/adr/` records boundary decisions; ADR 0002 covers the release-kit
  producer boundary, ADR 0004 the release-transaction authority.
- `docs/agent-integration.md` is the cold-start guide for agent adopters.
- `schemas/` is the checked registry for manifests, release context,
  release-kit, replay, fleet plans, evidence, and failure envelopes.
- `bin/gate` is the closeout gate; `bin/replay-action` is the release
  behavior oracle for action and runtime contract changes.
