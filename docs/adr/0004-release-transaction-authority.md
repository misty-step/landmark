# ADR 0004: Release Judgment and Public Mutation Share One Authority

Status: accepted
Date: 2026-07-13

## Context

Landmark already decides versions, produces changelogs and release notes, and
mutates public release surfaces through GitHub and semantic-release adapters.
Its documentation nevertheless described mutation as adjacent to release truth
without naming who owns completion when tags, release records, stable-channel
pointers, and artifact evidence span multiple systems.

Splitting release judgment from public release mutation creates two competing
definitions of "released." A decision service can say a release should exist
while a separate publisher only partially applies it, leaving downstream
deployers to infer authority from whichever event or public surface arrived
first. Folding deployment into either component would instead couple a public,
self-hostable release tool to environment-specific infrastructure.

## Decision

Landmark owns both release judgment and the complete public release mutation
transaction. The target contract is:

```text
publish(candidate release) -> completed release receipt
```

Landmark decides the version and release policy, validates the supplied
artifact manifest, applies every required public mutation, reconciles partial
state, and emits a durable receipt that binds the completed release to its
source revision, public references, and immutable artifact identities.

The transaction is idempotent and resumable, not fictionally atomic across
providers. A retry inspects existing state, completes missing compatible
mutations, and returns the same completed result when the transaction already
succeeded. Contradictory tags, versions, artifact identities, or public release
records fail closed and require explicit repair or waiver. A candidate is not
stable until all required mutations have been reconciled and the completed
receipt exists. Webhooks and forge events may wake consumers, but they are not
the authority.

The adjacent responsibilities remain separate:

- The product build pipeline constructs, signs, and publishes executable
  artifacts before stable release publication. Landmark validates their
  manifest and identity; it does not rebuild them.
- Producer adapters may render rich final-mile artifacts under the release-kit
  contract, as established by ADR 0002.
- Deployment systems consume completed release receipts and own
  environment-specific desired state, promotion, health verification,
  rollback, incident policy, and convergence.

## Consequences

- Landmark's portable core must eventually expose one release transaction and
  receipt model across local, generic-CI, GitHub, and future forge adapters.
- Mutation adapters remain provider-specific implementations, but none may
  invent an independent definition of release completion.
- Release evidence must distinguish a candidate, an in-progress or failed
  transaction, and a completed stable release.
- Partial publication is visible and recoverable instead of being reported as
  either an unexplained workflow failure or a successful release.
- Downstream deployment automation can follow a single authoritative receipt
  containing an immutable artifact identity instead of rebuilding, resolving a
  mutable tag, or treating event delivery as truth.
- Landmark remains public and self-hostable because no environment topology,
  infrastructure credential, promotion policy, or product-specific build logic
  enters its core contract.

## Rejected

- **Separate release publisher.** This makes the decision and its mutation two
  shallow modules with competing completion semantics and a larger failure
  surface between them.
- **Let each product workflow publish directly.** This duplicates transaction,
  reconciliation, and receipt behavior across every repository.
- **Let deployment infer the latest release from tags or events.** Delivery is
  not authority, and partially applied release state becomes indistinguishable
  from a stable release.
- **Make Landmark deploy the release.** This couples public release intelligence
  to private environment topology and gives Landmark responsibility for runtime
  convergence it cannot portably verify.
