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
- [0005](0005-deterministic-self-release.md): Keep the native self-release generator; publish its canonical changelog without requiring synthesis.

## Historical

None. The former ADR numbers stay reserved.
