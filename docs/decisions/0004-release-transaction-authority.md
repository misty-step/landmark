# 0004: Release judgment and public mutation share one authority

- **Status:** accepted
- **Date:** 2026-07-13
- **Primitives:** release transaction, completed release receipt, mutation adapters
- **Powder:** none recorded

## Context
Landmark decides versions and release notes and mutates public releases across
adapters. Separating judgment from mutation creates competing meanings of
“released” when tags, release records, channel pointers, and artifact evidence
are only partly applied. Deployment would couple the portable release tool
to environment-specific infrastructure.

## Decision
Landmark owns release judgment and the complete public release mutation
transaction: validate supplied artifact identities, reconcile required mutations,
and emit a durable completed release receipt. It does not rebuild product
artifacts or deploy releases; build pipelines own construction, signing, and
publication, [producer adapters](0002-release-kit-boundary.md) own rich outputs,
and deployment systems consume receipts and own promotion and convergence.

## Consequences
The [portable core](../../crates/landmark/) must share one transaction and receipt
model across adapters. Retries inspect state, finish compatible partial mutations,
and return the same completed result; contradictory tags, versions, identities,
or release records fail closed pending explicit repair or waiver. A candidate
is not stable until required mutations are reconciled and a completed receipt
exists. Evidence distinguishes candidates, incomplete transactions, and completed
releases; forge events are wake-up signals, not authority. Environment topology,
credentials, promotion policy, and product build logic stay outside the core.

Revisit if: an adapter needs an independent definition of completion or deployment
requirements would bring environment-specific responsibilities into the core.
