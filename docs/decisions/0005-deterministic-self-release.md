# 0005: Keep native self-releases; do not require synthesis

- **Status:** accepted
- **Date:** 2026-10-15
- **Primitives:** native self-release generator, generated changelog, release assets

## Context
Landmark already prepares release PRs and reconciles landed releases through
its Rust runtime. Replacing that engine would introduce a second version
authority. Requiring model-written notes before publication also made a paid
provider's availability a prerequisite for distributing a release.

## Decision
Keep one native generator for Landmark's CLI/Action release unit, following
the [shared release convention](https://github.com/misty-step/harness/blob/master/docs/release-convention.md)
and [Foundations](https://foundations.mistystep.io/). Publish the generated
`CHANGELOG.md` section unchanged; do not synthesize or rewrite self-release
notes in CI. The CLI retains explicit optional enrichment for callers that
request it, without changing the consumer Action's synthesis contract.

The existing baseline is immutable `v0.28.9`, commit
`0be932f9fd775f826427eb5ada2f24efb9a393d8`, version `0.28.9`. Preserve the
existing pre-1.0 policy in `version_decision::apply_stability`: features and
fixes increment patch; breaking changes increment minor. Stable versions use
ordinary SemVer bumps. This decision does not authorize retagging history or
promoting to 1.0 by hand.

## Consequences
Generated metadata still lands through a release PR. The same landed notes
reach GitHub Releases without an LLM credential. All four target binaries
and checksums remain release assets; only after upload does the native
`floating-tag` command select the major alias to advance. No client notes,
new service, or competing release generator is introduced.

Revisit if the native generator no longer meets the shared convention or the
product deliberately changes its pre-1.0 policy.
