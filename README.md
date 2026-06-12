# Landfall

Landfall is a focused release pipeline GitHub Action for repositories that use conventional commits.
It runs `semantic-release` to publish a version and changelog, then optionally synthesizes user-facing notes with any OpenAI-compatible LLM provider and prepends them to the GitHub Release body.

## What It Does

1. Uses a checked-in Rust runtime for Landfall-owned release behavior
2. Sets up Node.js only when full semantic-release mode is requested
3. Installs `semantic-release` and release plugins
4. Runs `semantic-release` (version bump, changelog update, release creation)
5. Optionally synthesizes user-facing notes from technical changelog content
6. Updates the GitHub Release body to prepend a `## What's New` section
7. Optionally creates a GitHub issue when synthesis/update fails and exposes synthesis status output

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

## Adoption Dry Run

Before wiring a release workflow, run Landfall's setup analyzer from a checkout:

```bash
dist/landfall setup --repo-root . --output-dir .landfall/setup
```

The command inspects release-tool signals, default branch, tag format, required
secrets, permissions, package topology, and recent conventional-commit usage. It
prints a JSON report with a recommended Landfall mode and writes workflow
candidates for semantic-release, release-please, changesets, changesets
monorepos, and manual-tag repositories. Every generated workflow includes
`healthcheck: "true"`, `GH_RELEASE_TOKEN`, `OPENROUTER_API_KEY`, and the
`contents`, `issues`, and `pull-requests` permissions Landfall needs.

The old Python backfill script is retired from the maintenance surface; use a
release re-run or `mode: synthesis-only` for repair runs.

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
| `synthesis-failure-issue` | No | `false` | If `true`, create a GitHub issue in the consuming repository when synthesis/update fails. |
| `notes-output-file` | No | `""` | Write synthesized notes to this file path. Use `{version}` placeholder for the release tag (e.g., `docs/releases/{version}.md`). |
| `notes-output-text-file` | No | `""` | Write synthesized notes as plaintext to this file path. Use `{version}` placeholder (e.g., `docs/releases/{version}.txt`). |
| `notes-output-html-file` | No | `""` | Write synthesized notes as an HTML fragment to this file path. Use `{version}` placeholder (e.g., `docs/releases/{version}.html`). |
| `notes-output-json` | No | `""` | Append a structured release entry to this JSON array file. Creates the file if it does not exist. |
| `prompt-template-path` | No | `""` | Path to a custom synthesis prompt template relative to repo root. Overrides `audience` and convention-based detection. |
| `audience` | No | `general` | Built-in prompt variant used when no custom prompt template is found. One of: `general`, `developer`, `end-user`, `enterprise`. |
| `product-description` | No | `""` | One-line product description injected into the synthesis prompt as `{{PRODUCT_CONTEXT}}`. |
| `voice-guide` | No | `""` | Tone/style guidance injected into the synthesis prompt as `{{VOICE_GUIDE}}`. |
| `changelog-source` | No | `auto` | Technical source for synthesis. `auto` tries `CHANGELOG.md`, then release body, then merged PR extraction. Or force: `changelog`, `release-body`, `prs`. |
| `healthcheck` | No | `false` | Validate LLM API key with a minimal probe request before synthesis. |
| `floating-tags` | No | `false` | Update floating major version tags (e.g., `v1`) after release. |
| `webhook-url` | No | `""` | Webhook endpoint URL. On synthesis success, POST a JSON payload with version, notes (markdown/HTML/plaintext), and release URL. |
| `webhook-secret` | No | `""` | HMAC-SHA256 secret for signing webhook payloads (X-Signature-256 header). Optional. |
| `slack-webhook-url` | No | `""` | Slack Incoming Webhook URL. On synthesis success, POST a Block Kit message with version, categorized notes, and release link. |
| `rss-feed-file` | No | `""` | Update this RSS 2.0 feed file with each release (includes synthesized notes as HTML). The feed file is committed back to the repo. |
| `rss-max-entries` | No | `50` | Maximum number of items retained in `rss-feed-file`. |

\* `llm-api-key` is required when `synthesis: true`.

## Outputs

| Output | Description |
| --- | --- |
| `released` | `true` if a new release/tag was created, otherwise `false`. |
| `release-tag` | Tag created by `semantic-release` (empty if no release). |
| `synthesis-succeeded` | `true` only when synthesis and release-body update both succeed for the released tag. |
| `synthesis-quality` | `valid`, `degraded`, or `failed`. |
| `synthesis-status` | Compact JSON status with quality, failure stage/message, model attempts, and publication destination outcomes. |
| `release-notes` | Synthesized user-facing release notes markdown. Empty if synthesis was skipped or failed. |
| `webhook-sent` | `true` when the generic webhook notification was sent successfully. |
| `slack-sent` | `true` when the Slack notification was sent successfully. |
| `synthesis-failure-issue-action` | Companion failure-issue lifecycle result: `closed`, `reported`, `failed`, or `skipped`. |

## Release Integrity Policy

Landfall separates the semantic-release publish step from its owned synthesis and
distribution steps:

- `synthesis-required: "true"` treats failed or degraded synthesis as a hard
  failure and blocks release-body mutation and floating-tag movement.
- Optional synthesis still allows the release to exist, but partial Landfall
  failures are reported through `synthesis-succeeded: false` and protected
  outputs such as floating tags do not move unless synthesis and release-body
  update both succeed.
- External GitHub and LLM calls made by the Rust runtime use bounded
  timeouts and retry policy.
- Generated `release-notes` output uses a collision-resistant GitHub output
  delimiter so synthesized content cannot truncate the output payload.

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

This skips Node.js setup and semantic-release entirely; the Rust runtime handles synthesis. Works with any release tool that creates GitHub Releases.

### Integration with Other Tools

Ready-to-use workflow examples for common release tools. Each uses `mode: synthesis-only` so Landfall only adds the synthesis layer.

| Tool | Example | Trigger |
| --- | --- | --- |
| [release-please](https://github.com/googleapis/release-please-action) | [`examples/release-please.yml`](examples/release-please.yml) | Push to main (release-please creates the release) |
| [Changesets](https://github.com/changesets/changesets) | [`examples/changesets.yml`](examples/changesets.yml) | Push to main (changesets publishes packages) |
| Changesets monorepo | [`examples/changesets-monorepo.yml`](examples/changesets-monorepo.yml) | Push to main (matrix per published package) |
| Manual tags | [`examples/manual-tag.yml`](examples/manual-tag.yml) | Tag push matching `v*` |

Copy the relevant example to `.github/workflows/` in your repository and update the secrets.

### Backfill Existing Releases

Backfill is no longer part of the core action surface. To repair an already-published release, rerun Landfall in `synthesis-only` mode for the target tag so the same Rust runtime path updates the GitHub Release body.

Single release tag:

```yaml
- uses: misty-step/landfall@v1
  with:
    mode: synthesis-only
    release-tag: v1.12.0
    github-token: ${{ secrets.GH_RELEASE_TOKEN }}
    llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
```

For multiple historical tags, dispatch one synthesis-only run per tag. This keeps repair behavior identical to normal release-note synthesis.

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

This writes per-version markdown files and maintains a typed JSON artifact feed for changelog pages:

```json
[
  {
    "version": "1.2.0",
    "tag": "v1.2.0",
    "notes": "## New Features\n- ...",
    "markdown": "## New Features\n- ...",
    "plaintext": "New Features\n...",
    "html": "<h2>New Features</h2>\n<ul>\n<li>...</li>\n</ul>",
    "slack": "## New Features\n- ...",
    "sections": [
      {
        "title": "New Features",
        "bullets": [
          {
            "text": "...",
            "links": []
          }
        ]
      }
    ],
    "published_at": "2026-02-08T12:00:00Z"
  }
]
```

The `synthesis-status` output is a compact JSON object for automation:

```json
{
  "synthesis_enabled": true,
  "released": true,
  "succeeded": true,
  "quality": "valid",
  "failure_stage": "",
  "failure_message": "",
  "model_attempts": [
    {
      "model": "anthropic/claude-sonnet-4",
      "succeeded": true,
      "quality": "valid",
      "message": ""
    }
  ],
  "destinations": {
    "release_body": { "enabled": true, "succeeded": true, "failure_stage": "", "failure_message": "" },
    "artifacts": { "enabled": true, "succeeded": true, "failure_stage": "", "failure_message": "" },
    "rss": { "enabled": false, "succeeded": false, "failure_stage": "", "failure_message": "" },
    "webhook": { "enabled": false, "succeeded": false, "failure_stage": "", "failure_message": "" },
    "slack": { "enabled": false, "succeeded": false, "failure_stage": "", "failure_message": "" }
  }
}
```

## RSS Release Feed

To publish a simple RSS 2.0 release feed (for feed readers, docs sites, etc.), set `rss-feed-file`:

```yaml
- uses: misty-step/landfall@v1
  with:
    github-token: ${{ secrets.GH_RELEASE_TOKEN }}
    llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
    rss-feed-file: docs/releases.xml
    rss-max-entries: "50"
```

Landfall updates the feed on each synthesized release and commits the file back to your repo.

For automatic Slack notifications, set `slack-webhook-url`.

The `release-notes` output is still available for custom notifications:

```yaml
- name: Custom Notify Slack
  if: steps.landfall.outputs.released == 'true'
  run: |
    echo "${{ steps.landfall.outputs.release-notes }}" | post-to-slack
```

## Dogfooding Landfall

Landfall releases itself without pushing generated release commits directly to
protected `master`. The repository workflow has two phases:

- `prepare-release-pr` runs `./dist/landfall prepare-self-release`, updates
  `CHANGELOG.md`, `package.json`, `crates/landfall/Cargo.toml`, and
  `Cargo.lock`, then rebuilds the checked-in Linux action binary and rewrites
  `dist/landfall.sha256` on `landfall/self-release`. It then opens or updates
  a release PR, which must pass the normal `merge-gate` before it can land.
  Hosted Quality Checks rebuild the binary in Ubuntu, upload the fresh binary
  as evidence, and byte-compare it to `dist/landfall`; that hosted comparison
  is authoritative for non-Linux developers.
- `publish-landed-release` runs on `master` pushes. It publishes a GitHub
  Release only when landed metadata is ahead of the latest semver tag, then
  runs Landfall in `synthesis-only` mode to update the release body and floating
  major tag. That synthesis pass is non-blocking because the release has already
  been published; failed or degraded synthesis is surfaced through Landfall
  outputs without turning a published release into a failed deployment.

The local replay oracle for this path is:

```bash
dist/landfall replay-action \
  --evidence-dir .landfall/replay \
  --scenario self_release_pr_path
```

### Metadata Version Sync (Landfall Repo)

This repository keeps `package.json` and the Rust crate version aligned to release tags:

- `prepare-self-release` updates `package.json`,
  `crates/landfall/Cargo.toml`, `Cargo.lock`, `dist/landfall`, and
  `dist/landfall.sha256` before opening the release PR.
- `.releaserc.json` still runs `./dist/landfall update-version-metadata` for
  consumers using full semantic-release mode.
- The release commit includes `CHANGELOG.md`, `package.json`,
  `crates/landfall/Cargo.toml`, `Cargo.lock`, `dist/landfall`, and
  `dist/landfall.sha256`.
- CI runs `cargo run --locked -- check-version-sync` to fail fast when metadata drifts from the latest semver tag.

### Action Contract Validation (Landfall Repo)

The public action contract is checked from `action.yml`:

- `cargo run --locked -- check-action-contract` fails when the README inputs table diverges from action metadata.
- The same command scans examples, project docs, and release workflows for unknown or deprecated Landfall inputs.
- CI runs the contract check before tests so stale consumer instructions fail fast.

### Consumer Replay Harness (Landfall Repo)

Landfall's replay harness creates disposable git fixture repositories and fake
local GitHub/LLM endpoints, then exercises synthesis, release-body updates,
artifact writing, failure policy, and floating-tag behavior without production
secrets:

```bash
bin/replay-action --evidence-dir .landfall/replay
```

The command writes `.landfall/replay/replay-result.json` with action outputs,
generated notes, release body before/after state, git tags, structured logs, and
fake service requests. CI runs a bounded replay on pull requests, the full replay
on `master`, and uploads the evidence packet for inspection.

For a local one-command gate, run:

```bash
bin/gate
```

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

Optional variables (supported, not required):

| Variable | Value |
| --- | --- |
| `{{BULLET_TARGET}}` | Suggested bullet range (for example, `3-7`) |
| `{{BREAKING_CHANGES_SECTION}}` | Breaking-change candidates extracted from the technical changelog (empty when none) |
| `{{PRODUCT_CONTEXT}}` | Optional `## Product context` section (from `product-description`) |
| `{{VOICE_GUIDE}}` | Optional `## Voice guide` section (from `voice-guide`) |

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
