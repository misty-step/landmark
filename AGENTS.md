# Landmark Agent Contract

*A reusable GitHub Action that handles the complete release pipeline: analyze
conventional commits to determine version bump, generate a technical
changelog (CHANGELOG.md), push the version bump + changelog to the repo,
create a GitHub Release, LLM-synthesize user-facing release notes from the
technical changelog, and update the GitHub Release body with those notes.*

## Product Boundary
Landmark is a portable release-intelligence runtime. The GitHub Action is one
packaging layer, not the product boundary. Keep release analysis, synthesis,
artifact planning, artifact writing, feed generation, notifications, evidence,
and provider policy in the Rust CLI. Keep GitHub-specific behavior behind
explicit adapter seams.

Landmark owns release truth, audience/importance classification, release-kit
plans, provenance, approval state, and producer contracts for final-mile output.
It does not own bespoke media production, brand design, CMS publishing, or
long-running creative pipelines. Demo videos, GIFs, images, blog posts, essays,
and docs updates should be represented as typed planned/produced artifacts and
delegated to explicit local, browser, service, harness, or human producer
adapters.

Read `VISION.md` before changing release boundaries, adoption modes,
agent-native contracts, or release-kit producer responsibilities.

## Architecture

### Runtime Structure
- `crates/landmark/src/main.rs` is the Rust binary facade: parse CLI, dispatch,
  and render top-level errors. Runtime responsibilities should live in focused
  modules under `crates/landmark/src/`.
- `bin/check-architecture` ratchets the facade and extracted module sizes; if
  a module needs to grow past its current budget, split ownership first or
  update the ratchet with an explicit architecture reason.
- `action.yml` is a composite GitHub Action wrapper around a bootstrap-
  downloaded Landmark release binary plus `semantic-release` for full GitHub
  release mode. The bootstrap step downloads and checksum-verifies the
  release binary matching the runner's OS/arch from the GitHub Release for
  the action's own pinned version; there is no checked-in binary.
- Release binaries are built for `x86_64-unknown-linux-musl`,
  `aarch64-unknown-linux-musl`, `aarch64-apple-darwin`, and
  `x86_64-apple-darwin` and published with `checksums.txt` as GitHub Release
  assets by `.github/workflows/release.yml`. For local development use
  `cargo run --locked -p landmark -- ...` or a locally built
  `target/debug/landmark`.
- Node is only for `semantic-release` in full mode. Do not add new Node or
  shell orchestration unless the platform boundary requires it.
- Python is not part of the active runtime. Do not reintroduce Python scripts
  for release behavior.

### Pipeline Steps
Composite GitHub Action with these steps:
- `semantic-release` handles steps 1-4 (analyze commits, generate changelog,
  push version bump + changelog, create GitHub Release) — proven,
  battle-tested.
- A bootstrap-downloaded Rust runtime handles step 5-6 (LLM-synthesize
  user-facing release notes, update the GitHub Release body) plus policy,
  artifacts, notifications, and replay.

## Key Design Decisions
- **Unix philosophy**: This does ONE thing — releases. Not code review, not monitoring.
- **Wraps semantic-release**: Don't reinvent the wheel. Extend it.
- **LLM synthesis is the value-add**: Technical changelogs exist. User-facing notes don't.
- **OpenRouter by default**: Supports provider choice and model fallback chains.
- **Reusable Action**: Any repo can opt in with a simple workflow file.

## Portability Direction
- A non-GitHub caller must be able to drive Landmark through CLI commands,
  manifest files, JSON artifacts, and local git state.
- `synthesis-only`, `backfill --mode artifacts-only`, `write-artifacts`,
  `update-feed`, and webhook/Slack notification paths are the portable core.
- `release-kit` artifacts are the planning/evidence boundary for richer
  final-mile output; prefer extending the typed kit contract over embedding a
  producer in the core runtime.
- GitHub operations such as release-body mutation, PR extraction, issue
  lifecycle, fleet scan, and Action outputs must be treated as adapter-specific.
- Prefer adding a provider interface or local artifact sink over broadening
  GitHub assumptions.

## Repo Gates
- Run `bin/gate` before closeout for code or contract changes.
- `bin/gate` includes `bin/check-architecture`; do not weaken the ratchet to
  land feature work.
- For action contract changes, also ensure `check-action-contract` coverage
  remains green through the gate.
- Use `bin/replay-action` when touching release orchestration, synthesis,
  artifact outputs, release-body mutation, notifications, feeds, or failure
  lifecycle behavior.

## action.yml Safety Patterns
- Never interpolate an `inputs.*` or `secrets.*` value directly into a `run:`
  shell block; pass it through an `env:` block and reference the shell
  variable instead. Direct interpolation is a shell-injection vector (a repo
  name, commit subject, or synthesized note containing shell metacharacters
  becomes code). Every `run:` step in `action.yml` follows this today; keep it
  that way when adding steps.
- Non-blocking pipeline stages (synthesis, artifact writes, RSS/webhook/Slack
  notifications) must `exit 0` on failure and record `succeeded`/
  `failure_stage`/`failure_message` outputs instead. A release must still
  publish when a best-effort stage fails; only `synthesis-required: "true"`
  turns a synthesis/publication failure into a hard blocker (see "Enforce
  synthesis required" in `action.yml`).

## File Structure
```
landmark/
├── action.yml              # Reusable GitHub Action (called by repos)
├── crates/
│   └── landmark/           # Rust runtime
├── templates/
│   └── synthesis-prompt.md # Prompt template for LLM
├── configs/
│   └── .releaserc.json    # Default semantic-release config
├── README.md
├── AGENTS.md               # Canonical agent contract (CLAUDE.md is a symlink to this file)
└── package.json            # For semantic-release deps
```

## How Repos Use It
```yaml
name: Release
on:
  push:
    branches: [master, main]
jobs:
  release:
    runs-on: ubuntu-latest
    permissions:
      contents: write
      issues: write
      pull-requests: write
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
          persist-credentials: false
      - uses: actions/create-github-app-token@v2
        id: release-token
        with:
          app-id: ${{ secrets.LANDMARK_RELEASER_APP_ID }}
          private-key: ${{ secrets.LANDMARK_RELEASER_PRIVATE_KEY }}
      - uses: misty-step/landmark@v0
        with:
          github-token: ${{ steps.release-token.outputs.token }}
          llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
          # Optional:
          # llm-model: anthropic/claude-sonnet-4
          # llm-fallback-models: "google/gemini-2.5-flash,openai/gpt-4o-mini"
```

## Requirements
- Node.js 22+
- Rust stable
- A GitHub App installed on the repo with Contents: read/write, and its App
  ID + private key as `LANDMARK_RELEASER_APP_ID` / `LANDMARK_RELEASER_PRIVATE_KEY`
  secrets (see README's "Why a GitHub App, not a PAT"). The default
  `${{ github.token }}` covers landmark's own action invocation but its tags
  and releases do not trigger further workflow runs the way an App
  installation token does — use the App when downstream automation depends
  on that trigger.
- `OPENROUTER_API_KEY` secret (or another compatible provider API key)

## Backlog And Docs
- Active work lives in `backlog.d/<nnn>-<slug>.md`; completed items are archived
  under `backlog.d/_done/`.
- Strategic groom reports live under `.groom/`.
- Keep README, `action.yml`, examples, and this file aligned. Stale agent-facing
  prose is a release risk because agents use it as an operating contract.

## Git
Prefer `jj` for local status and commits when it is available; fall back to
non-destructive `git` commands when an agent environment does not provide it.
Preserve user changes and avoid destructive git commands.
