# Contributing to Landfall

## Prerequisites

- Rust stable
- Node.js 22+
- npm (ships with Node)

## Setup

Clone and install dependencies:

```bash
git clone https://github.com/misty-step/landmark.git
cd landfall

# Node dependencies (semantic-release and plugins)
npm ci --no-fund --no-audit
```

## Linting

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

## Testing

```bash
cargo test --locked
bin/replay-action --evidence-dir .landfall/replay
```

Unit tests live with the Rust runtime. The replay harness creates disposable consumer repositories and fake local GitHub/LLM endpoints.

## Validating action.yml

The public action contract is validated against `action.yml` and README/examples:

```bash
cargo run --locked -- check-action-contract
```

## CI

All of the above run automatically on PRs and pushes to `master`. See `.github/workflows/ci.yml`.
CI also runs `cargo run --locked -- check-version-sync`, which ensures `package.json` and `crates/landfall/Cargo.toml` match the latest semver git tag.

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org/). semantic-release uses these to determine version bumps:

- `fix:` — patch release
- `feat:` — minor release
- `feat!:` or `BREAKING CHANGE:` — major release
- `chore:`, `docs:`, `ci:` — no release

## Security

- Don't pass secrets via CLI args in action steps (shows up in process listings). Use env vars.
- For provider-specific webhooks (ex: Slack), validate hostname allowlists (defense-in-depth).

## Release

Releases are fully automated. Merging to `master` triggers:

1. `semantic-release` analyzes commits, bumps version, updates `CHANGELOG.md`, creates a GitHub Release
2. LLM synthesis generates user-facing "What's New" notes and prepends them to the release body

No manual version bumping or tagging required.
Metadata versions are updated automatically during release prepare via `./dist/landfall update-version-metadata`.

## Architecture

```
landfall/
├── action.yml                # Composite GitHub Action entry point
├── .releaserc.json           # Repo-local semantic-release config (metadata sync + release commit assets)
├── configs/
│   └── .releaserc.json       # semantic-release config
├── crates/
│   └── landfall/             # Rust runtime and tests
├── dist/
│   ├── landfall              # Checked-in Linux action binary
│   └── landfall.sha256       # Runtime checksum
├── bin/
│   ├── gate                  # Local verification gate
│   └── replay-action         # Local consumer replay wrapper
├── templates/
│   └── synthesis-prompt.md   # LLM prompt template
└── package.json              # Node deps for semantic-release
```

The action runs as a composite GitHub Action (`action.yml`). Node handles `semantic-release`; the Rust runtime handles synthesis, API calls, artifact writing, policy, and replay.
