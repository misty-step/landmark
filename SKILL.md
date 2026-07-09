---
name: landmark
description: |
  Use when an agent needs release intelligence from Landmark: version analysis,
  changelog synthesis, release notes, release-kit planning, GitHub Action
  adoption, fleet rollout, classification, or release artifact evidence.
  Trigger phrases: "Landmark", "release intelligence", "changelog",
  "release notes", "version bump", "release kit".
argument-hint: "[describe|run|setup|fleet|release-kit|classify]"
---

# Landmark

Landmark owns release intelligence. Use it before hand-writing release truth
from git memory or ad-hoc commit summaries.

Read `VISION.md` before changing release boundaries, adoption modes,
agent-native contracts, or release-kit producer responsibilities.

## Route

| Need | Surface |
|---|---|
| Describe the current release state | `landmark describe --json` |
| Dry-run release analysis | `landmark run --provider local --dry-run` |
| Install in a repo | `landmark setup` |
| Fleet adoption | `fleet scan`, `fleet plan`, `fleet open-prs` |
| GitHub Action use | `misty-step/landmark@v0` |
| Local development | `cargo run --locked -p landmark -- ...` |
| Query core verbs over MCP | `cargo run --locked -p landmark-mcp` (stdio) |
| Full repo gate | `bin/gate` |

## Operating Rules

- Start with live git and Landmark's CLI/action surfaces, not remembered
  release state.
- Keep release analysis, synthesis, artifact planning, feed generation,
  evidence, approval state, and provider policy in the Rust CLI.
- GitHub is an adapter. Non-GitHub callers must be able to use CLI commands,
  JSON artifacts, local git state, and manifest files.
- Treat user-facing release notes as a model-native product surface with
  evidence and replay paths, not as static prose.
- Release-kit artifacts are the planning/evidence boundary for richer final
  output. Do not embed bespoke media production in the core runtime.

## MCP (landmark-920)

`crates/landmark-mcp` is a thin stdio JSON-RPC wrap (mirrors `bastion-mcp`'s
pattern: shell out to the real binary, pass its `--json`/`--error-format
json` output straight through) over three read-only, side-effect-free
verbs:

- `describe` — the agent-native self-description (`landmark describe --json`).
- `run_dry_run` — the release decision + release-kit plan, always forced to
  `--provider local --dry-run` (no GitHub calls, no files written).
- `doctor` — manifest validation (`landmark doctor --format json`).

`synthesize` (the LLM-calling changelog step) is a deliberate exclusion, not
an oversight: it needs an API key and spends real money per call, which does
not belong behind an MCP tool argument any agent can invoke. Anything that
mutates a release (`run --provider github`, `update-release`,
`notify-webhook`, `fleet open-prs`, self-release) stays CLI/action-only —
this server has no path to reach them.

**API face: waived.** Landmark's CLI + GitHub Action already cover every
non-GitHub caller (`--json`, structured error envelopes, local git/manifest
state); a REST API would be a second transport for verbs the CLI/MCP pair
already exposes, with no new consumer it would unblock.

Run it: `cargo run --locked -p landmark-mcp` (stdio). Tests:
`cargo test -p landmark-mcp` (also covered by `cargo test --locked`).

## Verification

In the Landmark repo:

```sh
bin/gate
```

For release-orchestration changes, also use the relevant replay path, especially
`bin/replay-action` when touching action behavior.
