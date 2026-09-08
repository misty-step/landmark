---
name: landmark
description: |
  Use when an agent needs release intelligence from Landmark: version analysis,
  changelog synthesis, release notes, release-kit planning, GitHub Action
  adoption, fleet rollout, classification, or release artifact evidence.
  Trigger phrases: "Landmark", "release intelligence", "changelog",
  "release notes", "version bump", "release kit".
argument-hint: "[describe|run|synthesize|setup|fleet|doctor]"
---

# Landmark

Landmark combines automated versioning with product-aware LLM synthesis.
Use it to explain changes and user impact from release evidence, rather than
hand-writing release truth from remembered commits.

Use the README, accepted ADRs, and versioned schemas for current contracts.
`VISION.md` is optional product rationale.

## Route

| Need | Surface |
|---|---|
| Describe the current release state | `landmark describe --json` |
| Dry-run release analysis | `landmark run --provider local --dry-run` |
| Generate product-aware release notes with a configured provider | `landmark synthesize --help`; [synthesis configuration](README.md#product-manifest) |
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

## MCP

`crates/landmark-mcp` exposes three inspection commands over stdio JSON-RPC:

- `describe` — the agent-native self-description (`landmark describe --json`).
- `run_dry_run` — the release decision and release-kit plan, forced to
  `--provider local --dry-run`. Optional API-evidence collection may execute
  local tools; this is not a sandbox.
- `doctor` — manifest validation (`landmark doctor --format json`).

LLM synthesis and public mutations stay on the CLI/Action surface, where
provider credentials, cost policy, and publication permissions are explicit.
MCP does not expose `synthesize`, release mutation, or notifications.

Run it: `cargo run --locked -p landmark-mcp` (stdio). Tests:
`cargo test -p landmark-mcp` (also covered by `cargo test --locked`).

## Verification

In the Landmark repo:

```sh
bin/gate
```

For release-orchestration changes, also use the relevant replay path, especially
`bin/replay-action` when touching action behavior.
