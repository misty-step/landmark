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

Landmark owns release intelligence. Use live git and the CLI before
hand-writing release truth from memory.

The [README](README.md), accepted [ADRs](docs/adr/), and versioned
[schemas](schemas/) define supported behavior. [VISION.md](VISION.md) is
optional rationale, not required reading.

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

## Operating Rules

- Keep release analysis, synthesis, artifact planning, feed generation,
  evidence, approval state, and provider policy in the Rust CLI.
- GitHub is an adapter. Non-GitHub callers use CLI commands, JSON artifacts,
  local git state, and manifest files.
- User-facing notes are a model-native product surface with evidence and
  replay paths, not static prose.
- Release-kit artifacts are the planning/evidence boundary. Do not embed
  bespoke media production in the core runtime.

## MCP

`crates/landmark-mcp` is a read-only stdio JSON-RPC wrap over the Landmark
binary (`--json` / `--error-format json`):

- `describe` — `landmark describe --json`
- `run_dry_run` — always `--provider local --dry-run` (no GitHub, no writes)
- `doctor` — `landmark doctor --format json`

`synthesize` is excluded: it spends an API key. Mutating verbs
(`run --provider github`, `update-release`, `notify-webhook`,
`fleet open-prs`, self-release) stay CLI/action-only.
