# Probe downstream impact after semver evidence exists

Priority: P3 · Status: blocked · Estimate: M

## Goal
Use Landmark's future changed-symbol evidence to answer what a dependency
release breaks for a specific downstream consumer.

## Oracle
- [ ] Given a Rust dependency release range and a consumer repo, Landmark can
      intersect changed public symbols with consumer usage and emit a scoped
      impact report.
- [ ] The report distinguishes confirmed usage, possible textual matches, and
      unsupported languages.
- [ ] The command refuses to run until the changed-symbol evidence contract from
      `005-build-diff-grounded-semver-evidence.md` exists.
- [ ] `bin/gate` passes.

## Children
1. Define the impact report schema only after changed-symbol evidence lands.
2. Start with Rust-only symbol usage and explicit unsupported-language output.
3. Add a local CLI command such as `landmark impact --dep <repo> --from <tag>
   --to <tag> --consumer <path>`.
4. Decide later whether this belongs in release-kit evidence, PR preview, or a
   separate operator command.

## Notes
Blocked by `005`. The operator decision parks risk-scoring and impact ambitions
behind the classifier, version-engine, and binary distribution work.
