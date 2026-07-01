# Thin the GitHub Action into a Rust action-run command

Priority: P2 · Status: pending · Estimate: L

## Goal
Move action orchestration policy out of YAML bash and into a replayable Rust
`landmark action-run` command while keeping GitHub-specific behavior behind an
adapter seam.

## Oracle
- [ ] `action.yml` mostly sets up the environment and invokes one Rust command
      for the post-release Landmark pipeline.
- [ ] Input validation, release-context resolution, publication policy,
      artifact writes, notifications, feeds, and failure lifecycle behavior are
      covered in Rust tests or replay scenarios.
- [ ] GitHub-specific environment parsing remains isolated from portable CLI
      release logic.
- [ ] Existing action contract checks catch any un-replayed subcommand or input
      drift.
- [ ] `bin/gate` passes.

## Children
1. Define `landmark action-run` inputs and GitHub environment adapter.
2. Port bash validation and output policy into focused Rust modules.
3. Collapse the action steps that only call Landmark subcommands.
4. Extend replay coverage for the migrated behavior.
5. Delete YAML bash only after the Rust path proves parity.

## Notes
Do this after version truth and classifier trust improve; otherwise the action
gets thinner around the wrong decision core.
