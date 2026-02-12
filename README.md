# Landfall

Landfall is a focused release pipeline GitHub Action for repositories that use conventional commits.
It runs `semantic-release` to publish a version and changelog, then optionally synthesizes user-facing notes with any OpenAI-compatible LLM provider and prepends them to the GitHub Release body.

## What It Does

1. Sets up Node.js and Python 3.12
2. Installs `semantic-release` and release plugins
3. Runs `semantic-release` (version bump, changelog update, release creation)
4. Optionally synthesizes user-facing notes from technical changelog content
5. Updates the GitHub Release body to prepend a `## What's New` section
6. Optionally creates a GitHub issue when synthesis/update fails and exposes synthesis status output

## Quick Start

Create `.github/workflows/release.yml` in your repository:

```yaml
name: Release

on:
  workflow_run:
    workflows:
      - CI
    types:
      - completed
    branches:
      - main
      - master
  workflow_dispatch:

concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: false

jobs:
  release:
    if: github.event_name == 'workflow_dispatch' || github.event.workflow_run.conclusion == 'success'
    runs-on: ubuntu-latest
    timeout-minutes: 15
    permissions:
      contents: write
      issues: write
      pull-requests: write
    steps:
      - name: Checkout
        uses: actions/checkout@v4
        with:
          fetch-depth: 0
          persist-credentials: false

      # Landfall: Automated semantic-release pipeline
      # https://github.com/misty-step/landfall
      - name: Run Landfall
        uses: misty-step/landfall@v1
        with:
          github-token: ${{ secrets.GH_RELEASE_TOKEN }}
          llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
          # Optional: customize model and fallbacks
          # llm-model: anthropic/claude-sonnet-4
          # llm-fallback-models: "google/gemini-2.5-flash,openai/gpt-4o-mini"
```

Landfall is language-agnostic. Your repo does not need `package.json` or Node.js — the action handles its own runtime setup. Any project using conventional commits works.

## Inputs

| Input | Required | Default | Description |
| --- | --- | --- | --- |
| `mode` | No | `full` | Pipeline mode: `full` (semantic-release + synthesis) or `synthesis-only` (synthesize for existing tag). |
| `release-tag` | No* | `""` | Release tag to synthesize notes for (required when `mode: synthesis-only`). |
| `github-token` | Yes | - | Personal access token with repo write access. Used by `semantic-release` and GitHub API update calls. |
| `llm-api-key` | No* | - | API key for synthesis (OpenRouter, OpenAI, or compatible providers). |
| `llm-model` | No | `anthropic/claude-sonnet-4` | Primary model ID for note synthesis. |
| `llm-fallback-models` | No | `google/gemini-2.5-flash,openai/gpt-4o-mini` | Comma-separated fallback model IDs tried in order if primary fails. |
| `llm-api-url` | No | `https://openrouter.ai/api/v1/chat/completions` | OpenAI-compatible chat completions endpoint URL. |
| `node-version` | No | `22` | Node.js version used to run `semantic-release`. |
| `synthesis` | No | `true` | If `true`, generate and prepend user-facing notes. |
| `synthesis-required` | No | `false` | If `true`, fail the action when synthesis/update fails (after failure reporting). |
| `synthesis-strict` | No | `false` | Deprecated alias for `synthesis-required`. |
| `synthesis-failure-issue` | No | `true` | If `true`, create a GitHub issue in the consuming repository when synthesis/update fails. |
| `notes-output-file` | No | `""` | Write synthesized notes to this file path. Use `{version}` placeholder for the release tag (e.g., `docs/releases/{version}.md`). |
| `notes-output-text-file` | No | `""` | Write synthesized notes as plaintext to this file path. Use `{version}` placeholder (e.g., `docs/releases/{version}.txt`). |
| `notes-output-html-file` | No | `""` | Write synthesized notes as an HTML fragment to this file path. Use `{version}` placeholder (e.g., `docs/releases/{version}.html`). |
| `notes-output-json` | No | `""` | Append a structured release entry to this JSON array file. Creates the file if it does not exist. |
| `prompt-template-path` | No | `""` | Path to a custom synthesis prompt template relative to repo root. Overrides `audience` and convention-based detection. |
| `audience` | No | `general` | Built-in prompt variant used when no custom prompt template is found. One of: `general`, `developer`, `end-user`, `enterprise`. |
| `changelog-source` | No | `auto` | Technical source for synthesis. `auto` tries `CHANGELOG.md`, then release body, then merged PR extraction. Or force: `changelog`, `release-body`, `prs`. |

\* `llm-api-key` is required when `synthesis: true`.

## Outputs

| Output | Description |
| --- | --- |
| `released` | `true` if a new release/tag was created, otherwise `false`. |
| `release-tag` | Tag created by `semantic-release` (empty if no release). |
| `synthesis-succeeded` | `true` only when synthesis and release-body update both succeed for the released tag. |
| `release-notes` | Synthesized user-facing release notes markdown. Empty if synthesis was skipped or failed. |

## Provider Examples

### OpenRouter (default)

```yaml
- uses: misty-step/landfall@v1
  with:
    github-token: ${{ secrets.GH_RELEASE_TOKEN }}
    llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
```

### OpenAI

```yaml
- uses: misty-step/landfall@v1
  with:
    github-token: ${{ secrets.GH_RELEASE_TOKEN }}
    llm-api-key: ${{ secrets.OPENAI_API_KEY }}
    llm-model: gpt-4o
    llm-api-url: https://api.openai.com/v1/chat/completions
```

### Custom OpenAI-Compatible Provider

```yaml
- uses: misty-step/landfall@v1
  with:
    github-token: ${{ secrets.GH_RELEASE_TOKEN }}
    llm-api-key: ${{ secrets.PROVIDER_API_KEY }}
    llm-model: provider/model-id
    llm-api-url: https://provider.example.com/v1/chat/completions
```

### Synthesis-Only Mode (release-please, changesets, manual tags)

Use `mode: synthesis-only` when another tool handles versioning and you only want Landfall for note synthesis:

```yaml
- uses: misty-step/landfall@v2
  with:
    mode: synthesis-only
    release-tag: ${{ steps.release.outputs.tag_name }}
    github-token: ${{ secrets.GH_RELEASE_TOKEN }}
    llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
```

This skips Node.js setup and semantic-release entirely — only Python is installed for synthesis. Works with any release tool that creates GitHub Releases.

### Backfill Existing Releases

Use `scripts/backfill.py` to repair already-published releases that are missing `## What's New`.

Single release tag:

```bash
python scripts/backfill.py \
  --repo owner/repo \
  --github-token "$GH_RELEASE_TOKEN" \
  --llm-api-key "$OPENROUTER_API_KEY" \
  --prompt-template templates/synthesis-prompt.md \
  --release-tag v1.12.0
```

All missing releases:

```bash
python scripts/backfill.py \
  --repo owner/repo \
  --github-token "$GH_RELEASE_TOKEN" \
  --llm-api-key "$OPENROUTER_API_KEY" \
  --prompt-template templates/synthesis-prompt.md \
  --all-missing
```

Notes:
- `--all-missing` is optional; omitting both selectors keeps current behavior (scan all releases and fill only missing ones).
- Use `--dry-run` to preview without updating release bodies.

## Portable Release Notes (Private Repos)

For private repos where GitHub Releases aren't publicly visible, use artifact outputs to make notes portable:

```yaml
- name: Run Landfall
  id: landfall
  uses: misty-step/landfall@v1
  with:
    github-token: ${{ secrets.GH_RELEASE_TOKEN }}
    llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
    notes-output-file: docs/releases/{version}.md
    notes-output-text-file: docs/releases/{version}.txt
    notes-output-html-file: docs/releases/{version}.html
    notes-output-json: releases.json
```

This writes per-version markdown files and maintains a JSON feed for changelog pages:

```json
[
  {
    "version": "1.2.0",
    "date": "2026-02-08",
    "notes": "## New Features\n- ...",
    "notes_plaintext": "New Features\n\n- ...",
    "notes_html": "<h2>New Features</h2>\n<ul>\n<li>...</li>\n</ul>"
  }
]
```

The `release-notes` output is always available for piping to downstream steps:

```yaml
- name: Notify Slack
  if: steps.landfall.outputs.released == 'true'
  run: |
    echo "${{ steps.landfall.outputs.release-notes }}" | post-to-slack
```

## Dogfooding Landfall

If Landfall releases itself, use local action code in `.github/workflows/release.yml`:

```yaml
- name: Run Landfall
  id: landfall
  uses: ./
  with:
    github-token: ${{ secrets.GH_RELEASE_TOKEN }}
    llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
    synthesis-required: "true"
```

Then move `v1` to the new `release-tag` output in the same workflow. This avoids stale major-tag drift.

### Metadata Version Sync (Landfall Repo)

This repository keeps `package.json` and `pyproject.toml` versions aligned to release tags:

- `.releaserc.json` runs `scripts/update-version-metadata.py` in semantic-release `prepare`.
- The release commit includes `CHANGELOG.md`, `package.json`, and `pyproject.toml`.
- CI runs `python scripts/check-version-sync.py` to fail fast when metadata drifts from the latest semver tag.

## Custom semantic-release Config

Landfall ships a default config at `configs/.releaserc.json`. If your repo has its own semantic-release config file (`.releaserc`, `.releaserc.json`, `.releaserc.yml`, `.releaserc.yaml`, `release.config.js`, `release.config.cjs`, or `release.config.mjs`), Landfall uses it instead of the bundled defaults.

This lets you customize branches, plugins, commit-analyzer rules, or anything else semantic-release supports.

If no config file is found, Landfall falls back to its bundled config with:

- `@semantic-release/commit-analyzer`
- `@semantic-release/release-notes-generator`
- `@semantic-release/changelog`
- `@semantic-release/git`
- `@semantic-release/github`

`CHANGELOG.md` is fully managed by `@semantic-release/changelog`. Do not keep a manual `# Changelog` or `## [Unreleased]` section in this repository, or release entries will be duplicated/mixed.

## Custom Prompt Templates

Landfall resolves the synthesis prompt template in this order:

1. **Explicit input** — `prompt-template-path: my-templates/release.md`
2. **Convention** — `.landfall/synthesis-prompt.md` in your repo root
3. **Bundled audience variant** — Landfall's built-in template selected by `audience` (default `general`)

Built-in audience variants:

| Audience | Template |
| --- | --- |
| `general` | [`templates/prompts/general.md`](templates/prompts/general.md) |
| `developer` | [`templates/prompts/developer.md`](templates/prompts/developer.md) |
| `end-user` | [`templates/prompts/end-user.md`](templates/prompts/end-user.md) |
| `enterprise` | [`templates/prompts/enterprise.md`](templates/prompts/enterprise.md) |

Custom templates must include these variables:

| Variable | Value |
| --- | --- |
| `{{PRODUCT_NAME}}` | Repository or product name |
| `{{VERSION}}` | Release version/tag |
| `{{TECHNICAL_CHANGELOG}}` | Extracted changelog content |

See [`templates/synthesis-prompt.md`](templates/synthesis-prompt.md) or [`templates/prompts/general.md`](templates/prompts/general.md) as a starting point for your own template.

## Example: Technical vs User-Facing

Technical release notes (generated):

```markdown
### Features
- add workspace import command (#214)

### Bug Fixes
- handle retries when webhook signature is stale (#229)

### Chores
- bump ci cache key
```

Synthesized `## What's New` section (prepended):

```markdown
## What's New

## New Features
- You can now import workspace configuration in one command, reducing setup time.

## Bug Fixes
- Webhook deliveries now retry more reliably when signatures expire.
```

Landfall intentionally omits internal-only changes (CI/tooling) from user-facing summaries.
