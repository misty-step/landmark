# Contributing to Landfall

## Prerequisites

- Python 3.12+
- Node.js 22+
- npm (ships with Node)

## Setup

Clone and install dependencies:

```bash
git clone https://github.com/misty-step/landfall.git
cd landfall

# Node dependencies (semantic-release and plugins)
npm ci --no-fund --no-audit

# Python dependencies (synthesis scripts + dev tooling)
python -m pip install requests pytest ruff check-jsonschema
```

## Linting

```bash
python -m ruff check scripts/ tests/
```

Ruff is configured in `pyproject.toml` — targets Python 3.12, 120-char line length, E/F/W rule sets.

## Testing

```bash
python -m pytest -q tests/
```

Tests live in `tests/` and import from `scripts/` via the `pythonpath` setting in `pyproject.toml`.

## Validating action.yml

The action metadata is validated against the official JSON schema:

```bash
python -m check_jsonschema --schemafile https://json.schemastore.org/github-action.json action.yml
```

## CI

All of the above run automatically on PRs and pushes to `master`. See `.github/workflows/ci.yml`.
CI also runs `python scripts/check-version-sync.py`, which ensures `package.json` and `pyproject.toml` match the latest semver git tag.

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org/). semantic-release uses these to determine version bumps:

- `fix:` — patch release
- `feat:` — minor release
- `feat!:` or `BREAKING CHANGE:` — major release
- `chore:`, `docs:`, `ci:` — no release

## Release

Releases are fully automated. Merging to `master` triggers:

1. `semantic-release` analyzes commits, bumps version, updates `CHANGELOG.md`, creates a GitHub Release
2. LLM synthesis generates user-facing "What's New" notes and prepends them to the release body

No manual version bumping or tagging required.
Metadata versions are updated automatically during release prepare via `scripts/update-version-metadata.py`.

## Architecture

```
landfall/
├── action.yml                # Composite GitHub Action entry point
├── .releaserc.json           # Repo-local semantic-release config (metadata sync + release commit assets)
├── configs/
│   └── .releaserc.json       # semantic-release config
├── scripts/
│   ├── shared.py             # Common utilities
│   ├── check-version-sync.py # CI drift detection against latest semver tag
│   ├── update-version-metadata.py # Release-time metadata version synchronizer
│   ├── synthesize.py         # LLM synthesis of user-facing notes
│   ├── update-release.py     # Prepends notes to GitHub Release body
│   ├── write-artifacts.py    # Writes notes to file/JSON outputs
│   ├── report-synthesis-failure.py  # Creates issue on failure
│   ├── update-floating-tag.py       # Manages vN major tags
│   └── backfill.py           # Backfill synthesis for past releases
├── templates/
│   └── synthesis-prompt.md   # LLM prompt template
├── tests/                    # pytest suite mirroring scripts/
└── package.json              # Node deps for semantic-release
```

The action runs as a composite GitHub Action (`action.yml`). Node handles `semantic-release`; Python handles everything else (synthesis, API calls, artifact writing).
