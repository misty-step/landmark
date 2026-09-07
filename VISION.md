# Landmark context

Optional product rationale, not a canonical lock, roadmap, or required reading
step. The [README](README.md) is the current adoption and command entrypoint;
accepted [ADRs](docs/adr/) and versioned [schemas](schemas/) define technical
contracts. Linear owns selected current non-R90 work and prioritization.
An operator request authorizes work, not this file or an old ticket; automatic
intake remains retired and R90 continues to use Habitat.

## Product aim

Landmark makes releasing a property of a repository rather than a manual
ritual: explain the semantic version, write the technical changelog, and
produce user-facing notes in the project's voice. The first useful local
preview needs no configuration or secrets. Configuration expresses product
context, audience, voice, and approved artifact destinations.

The portable Rust CLI is the product boundary. The GitHub Action, local
scripts, generic CI, and agent callers use the same engine; no stack, forge,
or CI system should be required by the core. Landmark's self-release is a
visible consumer, not a separate definition of release completion.

## Design principles

- **One deterministic decision engine.** Conventional commits provide the
  floor; typed ecosystem and API-diff evidence can refine it. Unknown intent
  and disagreement are visible, never patched over with substring heuristics.
- **Models at explicit seams.** Schema-constrained classification and prose
  stay grounded in structured evidence and fabrication checks. Commodity
  cost is the default; premium models are an explicit budget choice.
  Provider policy is portable BYOK, and pin freshness should be visible.
- **Voice is data.** The manifest supplies product voice, rather than
  repository-specific prompt edits.
- **Portable artifacts, explicit publication.** Markdown, HTML, JSON, and RSS
  can live in a repo, forge, or another approved destination. A private
  source stays private unless the release explicitly approves publication;
  destination ambiguity fails closed. A notes-hosting surface is a consumer
  of those artifacts, never a second source of release truth.
- **One release transaction.** Judgment, public mutation, reconciliation,
  and the completed receipt are one responsibility: inspect before writing,
  make retries idempotent, resume compatible partial state, and fail closed
  on contradictions. Tags and events alone are not completion authority.
  See [ADR 0004](docs/adr/0004-release-transaction-authority.md).
- **Explain before mutating.** Dry-run, evidence, and replay surfaces let a
  cold caller reproduce the decision locally. Versioned schemas, typed
  failures with `user_action`, and `describe --json` are public contracts.
- **Boring core, thin adapters.** Rust owns release behavior. Shell, Node,
  and YAML belong at platform seams; an adapter cannot invent its own
  meaning of "published."

## Boundaries

Product builds construct and sign executables. Landmark validates supplied
artifacts and binds their identities to the release; deployers consume the
completed receipt and own placement, promotion, health, rollback, and
convergence. Runtime transaction state and retained release artifacts keep
their native authority. Linear may link a receipt, not replace it.

Release-kit schemas coordinate explicit local, browser, service, harness, or
human producers. Bespoke media production, brand design, CMS publication, and
long-lived creative pipelines stay outside the runtime. See
[ADR 0002](docs/adr/0002-release-kit-boundary.md).

Use the README's
[receipt command and scope note](README.md#commit-the-transaction-and-emit-the-receipt)
for supported behavior and the self-release limitation. Aspirations are not
feature-status claims or ordered implementation work. Landmark optimizes for
usefulness and adoption, not monetization-first scope or weakening a safety
gate to ship.
