# Landfall

Landfall is a focused release pipeline GitHub Action for repositories that use conventional commits.
It runs `semantic-release` to publish a version and changelog, then optionally synthesizes user-facing notes with any OpenAI-compatible LLM provider and prepends them to the GitHub Release body.

## What It Does

1. Sets up Node.js and Python 3.12
2. Installs `semantic-release` and release plugins
3. Runs `semantic-release` (version bump, changelog update, release creation)
4. Optionally synthesizes user-facing notes from technical changelog content
5. Updates the GitHub Release body to prepend a `## What's New` section

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

## Inputs

| Input | Required | Default | Description |
| --- | --- | --- | --- |
| `github-token` | Yes | - | Personal access token with repo write access. Used by `semantic-release` and GitHub API update calls. |
| `llm-api-key` | No* | - | API key for synthesis (OpenRouter, OpenAI, or compatible providers). |
| `llm-model` | No | `anthropic/claude-sonnet-4` | Primary model ID for note synthesis. |
| `llm-fallback-models` | No | `google/gemini-2.5-flash,openai/gpt-4o-mini` | Comma-separated fallback model IDs tried in order if primary fails. |
| `llm-api-url` | No | `https://openrouter.ai/api/v1/chat/completions` | OpenAI-compatible chat completions endpoint URL. |
| `moonshot-api-key` | No | - | Deprecated alias for `llm-api-key` (legacy workflows). |
| `moonshot-model` | No | `kimi-k2.5` | Deprecated alias for `llm-model` (legacy workflows). |
| `node-version` | No | `22` | Node.js version used to run `semantic-release`. |
| `synthesis` | No | `true` | If `true`, generate and prepend user-facing notes. |

\* Provide `llm-api-key` (preferred) or `moonshot-api-key` (deprecated alias).

## Outputs

| Output | Description |
| --- | --- |
| `released` | `true` if a new release/tag was created, otherwise `false`. |
| `release-tag` | Tag created by `semantic-release` (empty if no release). |

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

### Moonshot / Kimi (legacy)

```yaml
- uses: misty-step/landfall@v1
  with:
    github-token: ${{ secrets.GH_RELEASE_TOKEN }}
    llm-api-key: ${{ secrets.MOONSHOT_API_KEY }}
    llm-model: kimi-k2.5
    llm-api-url: https://api.moonshot.cn/v1/chat/completions
```

## Default semantic-release Config

Landfall ships `configs/.releaserc.json` with:

- `@semantic-release/commit-analyzer`
- `@semantic-release/release-notes-generator`
- `@semantic-release/changelog`
- `@semantic-release/npm` (`npmPublish: false`)
- `@semantic-release/git`
- `@semantic-release/github`

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
