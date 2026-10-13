# Decisions

A steering veto list. Open it before proposing a new primitive, surface, or
storage home. Write a record after deciding *not* to build something the next
agent will otherwise re-propose. Do not write one per PR. Start from
[0000-template.md](0000-template.md).

## Steering

- [0001](0001-rust-runtime-boundary.md): Rust owns Landmark runtime behavior, not Python; binary distribution is superseded by 0003.
- [0002](0002-release-kit-boundary.md): Landmark owns release kits, not bespoke production studios or CMS publishing.
- [0003](0003-published-release-binaries.md): Publish verified release binaries as assets; do not check binaries into Git.
- [0004](0004-release-transaction-authority.md): Release judgment and public mutation share one transaction and receipt authority; Landmark does not build or deploy.

## Historical

None. All four former ADRs remain steering decisions; their numbers stay reserved.
