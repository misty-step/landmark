# Landmark

Landmark is a portable release-intelligence runtime for repositories that use
conventional commits. It can run as a GitHub Action, but the product boundary
is the Rust CLI: local scripts, generic CI systems, and agents can invoke the
same runtime to produce version decisions, technical changelogs, public release
notes, release-kit plans, feeds, and machine-readable evidence.

## What It Does

1. Uses a checked-in Rust runtime for Landmark-owned release behavior
2. Sets up Node.js only when full semantic-release mode is requested
3. Installs `semantic-release` and release plugins
4. Runs `semantic-release` (version bump, changelog update, release creation)
5. Optionally synthesizes user-facing notes from technical changelog content
6. Updates the GitHub Release body to prepend a `## What's New` section
7. Plans final-mile artifacts such as docs updates, blog drafts, demo scripts,
   images, GIFs, and videos through typed producer contracts
8. Optionally creates a GitHub issue when synthesis/update fails and exposes synthesis status output

## Adoption Modes

Landmark's product boundary is the Rust CLI. Start locally, then choose the
smallest integration mode that matches the release system you already have.

### Local CLI Preview

Use this first in a checkout. It requires no secrets and does not call GitHub or
an LLM:

```bash
cargo run --locked -- run --provider local --repo-root .
```

The command reads git tags and conventional commits, chooses the next semantic
version when `--release-tag` is omitted, writes `.landmark/run/evidence.json`,
generates a technical changelog, writes `.landmark/run/release-kit.json`, and
writes markdown, plaintext, HTML, JSON, and RSS release artifacts under
`docs/releases/`. The evidence packet also embeds the release-kit plan so
`--dry-run` can print the complete final-mile artifact graph without writing
files.

Build from source with `cargo run --locked -- ...` or a locally built
`target/debug/landmark`, or download a published per-target release binary
(`landmark-x86_64-unknown-linux-musl`, `landmark-aarch64-unknown-linux-musl`,
`landmark-aarch64-apple-darwin`, `landmark-x86_64-apple-darwin`) plus
`checksums.txt` from a [GitHub Release](https://github.com/misty-step/landmark/releases).
The GitHub Action downloads and checksum-verifies the matching binary itself;
it no longer ships a checked-in binary.

The executable quickstart oracle is:

```bash
cargo run --locked -- replay-action --scenario first_run_local_preview
```

### Generic CI

A shell script, GitLab CI job, Forgejo workflow, Buildkite step, or agent can run
the same Rust runtime directly:

```bash
cargo run --locked -- run \
  --provider local \
  --repo-root . \
  --output-dir .landmark/run \
  --output-file docs/releases/{version}.md \
  --output-json docs/releases/releases.json \
  --rss-feed-file docs/releases/feed.xml
```

Use `--dry-run` when you only want the evidence preview on stdout. Use
`--provider github --publish-release-body` only when the CI job is explicitly
allowed to mutate an existing GitHub Release.

### GitHub Action Full Mode

Use full mode when Landmark should run `semantic-release`, create the release,
then synthesize and publish notes:

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

      # Landmark: Automated semantic-release pipeline
      # https://github.com/misty-step/landmark
      - name: Run Landmark
        uses: misty-step/landmark@v1
        with:
          github-token: ${{ secrets.GH_RELEASE_TOKEN }}
          llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
          # Optional: customize model and fallbacks
          # llm-model: anthropic/claude-sonnet-5
          # llm-fallback-models: "google/gemini-2.5-flash,anthropic/claude-haiku-4.5"
```

Landmark is language-agnostic. Your repo does not need `package.json` or Node.js
unless full mode is running `semantic-release`; the action handles its own Node
24 runtime setup.

Full mode's version/changelog/tag/release decisions are `semantic-release`'s,
kept only as a named **compatibility path** for repos that already run it.
Landmark's own Rust version-decision engine (`landmark run`,
`prepare-self-release`) is the source of truth everywhere else; `semantic-release`
stays wired into full mode until a Rust-owned full-mode v2 can analyze, version,
changelog, tag, and publish without it.

### GitHub Action Synthesis-Only Mode

Use synthesis-only when release-please, Changesets, manual GitHub Releases, or a
custom pipeline already creates the version and release:

```yaml
- uses: misty-step/landmark@v1
  with:
    mode: synthesis-only
    release-tag: ${{ steps.release.outputs.tag_name }}
    github-token: ${{ secrets.GH_RELEASE_TOKEN }}
    llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
```

This skips semantic-release entirely. Ready-to-use synthesis-only examples:

| Tool | Example | Trigger |
| --- | --- | --- |
| [release-please](https://github.com/googleapis/release-please-action) | [`examples/release-please.yml`](examples/release-please.yml) | Push to main; release-please creates the release |
| [Changesets](https://github.com/changesets/changesets) | [`examples/changesets.yml`](examples/changesets.yml) | Push to main; Changesets publishes packages |
| Changesets monorepo | [`examples/changesets-monorepo.yml`](examples/changesets-monorepo.yml) | Push to main; matrix per published package |
| Manual GitHub Releases | [`examples/manual-tag.yml`](examples/manual-tag.yml) | `release.published` event |

## Agent-Native Contracts

Agents should start with the self-description document instead of scraping this
README:

```bash
landmark describe --json
```

The describe payload exposes commands, modes, providers, inputs, outputs,
schemas, examples, preview policy for mutating commands, and the failure
taxonomy. stdout carries JSON payloads; stderr carries logs and errors. Use
`--error-format json` on any command to receive a stable failure envelope with
`code`, `stage`, `retryable`, `user_action`, and redacted `context`.

The cold-agent integration oracle is:

```bash
landmark replay-action --scenario agent_native_contracts --format json
```

The checked schema registry lives in `schemas/`:

- `schemas/landmark-manifest.v1.schema.json` for `.landmark.yml`
- `schemas/synthesis-status.v1.schema.json` for synthesis status output
- `schemas/release-context.v1.schema.json` for deterministic release context packets
- `schemas/release-kit.v1.schema.json` for final-mile release kit plans and producer contracts
- `schemas/replay-result.v1.schema.json` for replay evidence
- `schemas/fleet-plan.v1.schema.json` for fleet adoption plans
- `schemas/release-entry.v1.schema.json` for release-note JSON entries
- `schemas/run-evidence.v1.schema.json` for `landmark run` evidence packets
- `schemas/failure-envelope.v1.schema.json` for `--error-format json` stderr

For the full cold-agent contract, see `docs/agent-integration.md`.

## Final-Mile Release Kit

Landmark's durable role is release truth and artifact coordination, not every
creative production engine. It should own the typed release kit:

- release facts, classification, and audience-specific importance
- artifact recommendations, paths, dependencies, and acceptance checks
- provenance for the sources each artifact used
- approval state, blockers, and waivers
- producer contracts for richer outputs such as docs patches, migration guides,
  blog posts, essays, social copy, screenshots, images, GIFs, and demo videos

Landmark should directly produce simple/default text and data artifacts close to
release truth: technical changelogs, public notes, markdown/plaintext/HTML/JSON
entries, RSS feeds, migration notes, documentation patch suggestions, and
announcement/blog drafts. Specialized media and publishing work belongs behind
explicit producer adapters: local CLIs, browser capture, external services,
harness skills, or human approval. A video producer should receive a release-kit
brief and return artifact paths, hashes, evidence, and review status; it should
not become part of the core release engine.

This keeps the core deep: Landmark decides what the release needs, why it needs
it, what facts must be preserved, and whether the final packet is complete. The
repo or organization chooses the producers that satisfy those contracts.

## Adoption Dry Run

Before wiring a release workflow, run Landmark's setup analyzer from a checkout:

```bash
landmark init --repo-root . --output .landmark.yml --dry-run
landmark setup --repo-root . --output-dir .landmark/setup
```

`init` infers a first `.landmark.yml` from package metadata, README content,
release-tool signals, and the repository name. `setup` then inspects
release-tool signals, default branch, tag format, required secrets,
permissions, package topology, recent conventional-commit usage, and any
checked-in `.landmark.yml`. It prints a JSON report with a recommended Landmark
mode and writes workflow candidates for semantic-release, release-please,
changesets, changesets monorepos, and manual-tag repositories. Every generated
workflow includes `healthcheck: 'true'`, `GH_RELEASE_TOKEN`,
`OPENROUTER_API_KEY`, and the `contents`, `issues`, and `pull-requests`
permissions Landmark needs.

## Fleet Adoption

Landmark can plan adoption across many GitHub repositories before opening any
branches:

```bash
landmark fleet scan \
  --owner phrazzld \
  --owner misty-step \
  --output .landmark/fleet.json

landmark fleet plan \
  --input .landmark/fleet.json \
  --output-dir .landmark/fleet-plan

landmark fleet open-prs \
  --dry-run \
  --plan-dir .landmark/fleet-plan \
  --output-dir .landmark/fleet-plan/prs
```

`fleet scan` is read-only. It lists repository activity, default branch,
archive/private state, detected release tooling, tag format, package topology,
workflow files, Landmark presence, branch-protection availability, and required
secret metadata. Secret values are never requested or printed. If GitHub hides
secret or branch-protection metadata for a repository, the scan records
`unavailable` with the missing scope or access boundary instead of guessing.
The default scan uses bounded concurrency and avoids expensive per-repo secret
and branch-protection probes; pass `--deep-checks` for a smaller owner slice
when you want GitHub to verify branch protection and Actions secret names.

`fleet plan` classifies each repository by kind and release surface, including
`non-release` repositories that should not publish release artifacts, then ranks
it into an integration mode: `local`, `generic-ci`, `github-full`,
`github-synthesis-only`, `manifest-only`, `backfill-first`, `blocked`, or
`skipped`. It writes `.landmark/fleet-plan/plan.json` and a Markdown operator
dashboard with repository kind, release surface, integration rationale, risk
flags, required secret names, missing secrets, skip reasons, workflow patch
paths, initial version/tag recommendations, artifact paths, rollback guidance,
historical backfill preview commands, and migration notes. GitHub secrets are
required only for GitHub integration modes; local, generic CI, manifest-only,
backfill-first, and skipped non-release plans avoid secret blockers. Active
application and library repositories with packages but no release tags or
release automation enter the `backfill-first` lane; documentation/config-only
repositories with no package topology remain `skipped` as non-release.
Repositories with existing release-please or Changesets workflows get generated
patch records for those workflow paths. Repositories with existing
semantic-release workflows are blocked until an operator explicitly chooses
whether Landmark should replace the full release job. Workflow patch records
preserve the existing workflow YAML body, add or replace `jobs.synthesize`, and
block instead of serializing a workflow body that contains obvious token-like
literals.

`fleet open-prs --dry-run` renders the exact `.landmark.yml`, any generated
existing-workflow updates such as `.github/workflows/release-please.yml` or
`.github/workflows/changesets.yml`, fallback
`.github/workflows/landmark-release.yml` for repos without an owning workflow,
`diff.md`, and `open-prs.json` receipt Landmark would propose for each eligible
repository under `.landmark/fleet-plan/prs/`. Local, generic CI,
backfill-first, and manifest-only modes do not get a GitHub release workflow.
For `backfill-first`, the PR is manifest-only/local-artifacts adoption: it adds
`.landmark.yml` with local artifact destinations and carries the first-release
approval notes in `diff.md`; it does not require `GH_RELEASE_TOKEN`,
`OPENROUTER_API_KEY`, or release-body mutation.
Confirmed rollout requires `--confirm-remote --max-prs 1`; the receipt records
branch names, commit messages, rollback/disposition guidance, evidence
directories, and an `APPLY.md` packet with the remote branch, commit, PR,
rollback, and monitoring commands. Operators merge one downstream PR, watch its
release run, then continue the fleet rollout deliberately.

For the Misty Step factory-wide adoption standard, including when to choose
full mode versus synthesis-only, see
[`docs/fleet-integration-playbook.md`](docs/fleet-integration-playbook.md).

## Product Manifest

Landmark reads `.landmark.yml` from the repository root before synthesis. It
keeps product context, audience, voice, changelog source, artifact outputs, and
model policy in the repo instead of requiring every workflow to repeat them.
Non-empty action inputs still win over manifest values.
When `model.primary` is omitted, `model.policy` selects Landmark's built-in
default model tier: `cheap` uses `anthropic/claude-haiku-4.5`, while `balanced`
and `rich` use `anthropic/claude-sonnet-5`. `off` disables LLM synthesis while
still publishing the technical release.

```yaml
product:
  name: Landmark
  description: Versioning, changelog, and release-note automation.
audience: developer # general, developer, end-user, enterprise
voice: Clear, concrete, and release-operator friendly.
changelog:
  source: auto # auto, changelog, release-body, prs
artifacts:
  markdown: docs/releases/{version}.md
  plaintext: docs/releases/{version}.txt
  html: docs/releases/{version}.html
  json: docs/releases/releases.json
  rss: docs/releases/feed.xml
release:
  profile: full # full or synthesis-only
model:
  policy: balanced # cheap, balanced, rich, off
  primary: anthropic/claude-sonnet-5
  fallbacks:
    - google/gemini-2.5-flash
    - anthropic/claude-haiku-4.5
budget:
  max_input_tokens: 12000
  max_output_tokens: 1200
  max_usd: 0.25
```

Use `landmark doctor --repo-root .` to validate manifest enums before a
workflow run. Use `landmark setup --repo-root . --output-dir
.landmark/setup` after editing the manifest to regenerate workflow candidates
that reflect the durable defaults.

Use `landmark synthesize --dry-run-cost ...` to inspect the release context
packet without calling an LLM. The dry run reports deterministic repo facts,
estimated input/output tokens, model tier, selected model, skip/use/escalation
decision, cost estimate, deterministic release classification, and the final
context sources included in the prompt. In `balanced` mode, docs-only,
chore-only, dependency-only, and internal-tooling releases are skipped; breaking,
security, and migration-heavy releases escalate to the rich tier.

During real synthesis runs with `model.policy` enabled and an API key present,
Landmark first sends the parsed commits, commit bodies, diff statistics, and the
rendered changelog context through a cheap OpenAI-compatible classifier. It
uses the configured `model.primary` when set, falling back to
`deepseek/deepseek-v4-flash` (with `anthropic/claude-haiku-4.5` as a further
fallback) when no primary model is configured — the same precedence on every
endpoint. Conventional `feat`, `fix`, `perf`,
security, migration, and breaking-change signals remain the deterministic floor:
if the model would downgrade or skip them, Landmark records the disagreement in
the synthesis context, preserves a synthesis-worthy classification, and appends
a short classification notice to the generated release notes.

Use `landmark run --provider local --repo-root .` to write a release-kit
plan at `.landmark/run/release-kit.json` and record its schema and hash in
`.landmark/run/evidence.json`; the evidence packet includes the deterministic
version decision and changed-file list so downstream producers do not have to
re-read git state. `--dry-run` keeps the filesystem untouched and prints the
same `release_kit` object inside the stdout evidence packet. Low internal
releases keep the kit to Landmark-owned changelog and notes evidence;
high-importance, security, breaking, or migration-heavy releases add planned
producer-adapter artifacts such as migration guides, docs updates, blog drafts,
and demo videos with explicit handoff contracts, evidence paths, and pending
approval state.

`landmark notify-release-feed` is the first remote-service producer adapter for
the release kit. It reads `.landmark/run/evidence.json` and
`.landmark/run/release-kit.json`, fills the text-floor artifacts
(`version-decision`, `changed-files`, and `changelog-diff`) as produced
producer-adapter outputs, and POSTs the full `landmark.release-kit.v1` JSON body
to the receiver. The adapter uses the same signature scheme as `notify-webhook`:
`X-Signature-256: sha256=<hmac>`, computed over the raw JSON body. Configure it
with `RELEASE_FEED_URL` and `RELEASE_FEED_SECRET`; `LANDMARK_RELEASE_FEED_*` and
`RELEASE_KIT_FEED_*` are accepted aliases. Missing URL or secret config is a
clean skip.

Use `landmark backfill --repo-root . --since <tag> --mode artifacts-only
--dry-run` to preview historical artifact migration for repositories that
already have release tags. For `backfill-first` repos, create the
operator-approved initial tag only after inspecting the fleet plan
recommendation, then run the preview command before enabling any
release-mutating workflow.

## Inputs

| Input | Required | Default | Description |
| --- | --- | --- | --- |
| `mode` | No | `full` | Pipeline mode: `full` (semantic-release + synthesis) or `synthesis-only` (synthesize for existing tag). |
| `release-tag` | No* | `""` | Release tag to synthesize notes for (required when `mode: synthesis-only`). |
| `github-token` | Yes | - | Personal access token with repo write access. Used by `semantic-release` and GitHub API update calls. |
| `llm-api-key` | No* | - | API key for synthesis (OpenRouter, OpenAI, or compatible providers). |
| `llm-model` | No | manifest policy default | Primary model ID for note synthesis. |
| `llm-fallback-models` | No | manifest, then `google/gemini-2.5-flash,anthropic/claude-haiku-4.5` | Comma-separated fallback model IDs tried in order if primary fails. |
| `llm-api-url` | No | `https://openrouter.ai/api/v1/chat/completions` | OpenAI-compatible chat completions endpoint URL. |
| `node-version` | No | `24` | Node.js version used to run `semantic-release`. |
| `synthesis` | No | `true` | If `true`, generate and prepend user-facing notes. |
| `synthesis-required` | No | `false` | If `true`, fail the action when synthesis/update fails (after failure reporting). |
| `synthesis-strict` | No | `false` | Deprecated alias for `synthesis-required`. |
| `synthesis-failure-issue` | No | `false` | If `true`, create a GitHub issue in the consuming repository when synthesis/update fails. |
| `notes-output-file` | No | manifest, then `""` | Write synthesized notes to this file path. Use `{version}` placeholder for the release tag (e.g., `docs/releases/{version}.md`). |
| `notes-output-text-file` | No | manifest, then `""` | Write synthesized notes as plaintext to this file path. Use `{version}` placeholder (e.g., `docs/releases/{version}.txt`). |
| `notes-output-html-file` | No | manifest, then `""` | Write synthesized notes as an HTML fragment to this file path. Use `{version}` placeholder (e.g., `docs/releases/{version}.html`). |
| `notes-output-json` | No | manifest, then `""` | Append a structured release entry to this JSON array file. Creates the file if it does not exist. |
| `prompt-template-path` | No | `""` | Path to a custom synthesis prompt template relative to repo root. Overrides `audience` and convention-based detection. |
| `audience` | No | manifest, then `general` | Built-in prompt variant used when no custom prompt template is found. One of: `general`, `developer`, `end-user`, `enterprise`. |
| `product-description` | No | manifest, then `""` | One-line product description injected into the synthesis prompt as `{{PRODUCT_CONTEXT}}`. |
| `voice-guide` | No | manifest, then `""` | Tone/style guidance injected into the synthesis prompt as `{{VOICE_GUIDE}}`. |
| `changelog-source` | No | manifest, then `auto` | Technical source for synthesis. `auto` tries `CHANGELOG.md`, then release body, then merged PR extraction. Or force: `changelog`, `release-body`, `prs`. |
| `healthcheck` | No | `false` | Validate LLM API key with a minimal probe request before synthesis. |
| `floating-tags` | No | `false` | Update floating major version tags (e.g., `v1`) after release. |
| `webhook-url` | No | `""` | Webhook endpoint URL. On synthesis success, POST a JSON payload with version, notes (markdown/HTML/plaintext), and release URL. |
| `webhook-secret` | No | `""` | HMAC-SHA256 secret for signing webhook payloads (X-Signature-256 header). Optional. |
| `slack-webhook-url` | No | `""` | Slack Incoming Webhook URL. On synthesis success, POST a Block Kit message with version, categorized notes, and release link. |
| `rss-feed-file` | No | manifest, then `""` | Update this RSS 2.0 feed file with each release (includes synthesized notes as HTML). The feed file is committed back to the repo. |
| `rss-max-entries` | No | `50` | Maximum number of items retained in `rss-feed-file`. |

\* `llm-api-key` is required when `synthesis: true` and the model policy does
not skip the LLM call.

## Outputs

| Output | Description |
| --- | --- |
| `released` | `true` if a new release/tag was created, otherwise `false`. |
| `release-tag` | Tag created by `semantic-release` (empty if no release). |
| `synthesis-succeeded` | `true` when synthesis/update succeeds or when policy intentionally skips LLM synthesis for the released tag. |
| `synthesis-quality` | `valid`, `degraded`, `skipped`, or `failed`. |
| `synthesis-status` | Compact JSON status with quality, failure stage/message, model attempts, context sources, cost estimate, release classification, and publication destination outcomes. |
| `release-notes` | Synthesized user-facing release notes markdown. Empty if synthesis was skipped or failed. |
| `webhook-sent` | `true` when the generic webhook notification was sent successfully. |
| `slack-sent` | `true` when the Slack notification was sent successfully. |
| `synthesis-failure-issue-action` | Companion failure-issue lifecycle result: `closed`, `reported`, `failed`, or `skipped`. |

## Release Integrity Policy

Landmark separates the semantic-release publish step from its owned synthesis and
distribution steps:

- `synthesis-required: "true"` treats failed or degraded synthesis as a hard
  failure and blocks release-body mutation and floating-tag movement.
- Optional synthesis still allows the release to exist, but partial Landmark
  failures are reported through `synthesis-succeeded: false` and protected
  outputs such as floating tags do not move unless synthesis and release-body
  update both succeed.
- Intentional synthesis skips from `model.policy: off`, low-significance
  balanced policy, or manifest budget limits are treated as successful policy
  outcomes. Release-body mutation and artifact writes are skipped, while
  `synthesis-status.context.cost` records the reason.
- External GitHub, webhook, release-feed, Slack, and LLM calls made by the Rust
  runtime use bounded curl calls (`--connect-timeout`, `--max-time`, retries for
  429/5xx). `replay-action --scenario http_resilience_policy` exercises slow,
  throttled, and failing providers, `replay-action --scenario release_feed_adapter`
  exercises the signed release-kit receiver path, and
  `replay-action --scenario action_side_effect_coverage` fails if `action.yml`
  invokes a Landmark subcommand without replay coverage.
- Generated `release-notes` output uses a collision-resistant GitHub output
  delimiter so synthesized content cannot truncate the output payload.

## Provider Examples

### OpenRouter (default)

```yaml
- uses: misty-step/landmark@v1
  with:
    github-token: ${{ secrets.GH_RELEASE_TOKEN }}
    llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
```

### OpenAI

```yaml
- uses: misty-step/landmark@v1
  with:
    github-token: ${{ secrets.GH_RELEASE_TOKEN }}
    llm-api-key: ${{ secrets.OPENAI_API_KEY }}
    llm-model: gpt-4o
    llm-api-url: https://api.openai.com/v1/chat/completions
```

### Custom OpenAI-Compatible Provider

```yaml
- uses: misty-step/landmark@v1
  with:
    github-token: ${{ secrets.GH_RELEASE_TOKEN }}
    llm-api-key: ${{ secrets.PROVIDER_API_KEY }}
    llm-model: provider/model-id
    llm-api-url: https://provider.example.com/v1/chat/completions
```

### Backfill Existing Releases

Backfill is a Rust-owned CLI path for mature repositories that already have tags, GitHub Releases, or a `CHANGELOG.md`. It plans historical release-note artifacts without calling the LLM, then can write the same markdown, plaintext, HTML, JSON, and RSS-compatible artifact formats used by normal synthesis.

Preview the migration from an existing tag:

```bash
landmark backfill --repo-root . --since v1.0.0 --dry-run
```

Write portable artifacts only, which is the default safe migration mode:

```bash
landmark backfill \
  --repo-root . \
  --since v1.0.0 \
  --mode artifacts-only \
  --repository owner/repo \
  --github-token "$GH_RELEASE_TOKEN"
```

Preview GitHub Release body updates before considering mutation:

```bash
landmark backfill \
  --repo-root . \
  --since v1.0.0 \
  --mode release-body \
  --dry-run \
  --repository owner/repo \
  --github-token "$GH_RELEASE_TOKEN"
```

`release-body` writes are refused unless the run is a dry-run or the operator passes `--confirm-release-body`. The output manifest lists processed tags, skipped tags, remaining tags, artifact paths, preview hashes, and the estimated cost. Artifact backfill does not call the LLM; use the manifest to batch later synthesis if you want enhanced historical notes.

## Portable Release Notes (Private Repos)

For private repos where GitHub Releases aren't publicly visible, use artifact outputs to make notes portable:

```yaml
- name: Run Landmark
  id: landmark
  uses: misty-step/landmark@v1
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
      "model": "anthropic/claude-sonnet-5",
      "succeeded": true,
      "quality": "valid",
      "message": "",
      "cost": {
        "input_tokens": 1800,
        "output_tokens": 1000,
        "model_tier": "balanced",
        "model": "anthropic/claude-sonnet-5",
        "estimated_usd": 0.0068,
        "skip": false,
        "skip_reason": ""
      }
    }
  ],
  "context": {
    "release": {
      "version": "v1.2.0",
      "changelog_source": "auto",
      "model_policy": "balanced"
    },
    "deterministic": {
      "commits": [
        {
          "subject": "feat(cli): add import",
          "body": "Adds a guided import flow.",
          "short_hash": "abc1234",
          "conventional_type": "feat",
          "breaking": false
        }
      ],
      "tags": ["v1.1.0"],
      "changed_files": ["src/import.rs"],
      "diff_stats": [{ "path": "src/import.rs", "additions": 42, "deletions": 3, "binary": false }],
      "docs": [{ "path": "README.md", "title": "Landmark" }],
      "artifacts": {
        "internal_technical_changelog": "landmark.internal-technical-changelog.v1",
        "public_release_notes": "landmark.public-release-notes.v1:developer"
      }
    },
    "sources": [
      { "name": "prompt_template", "kind": "prompt", "estimated_tokens": 700, "included": true },
      { "name": "technical_changelog", "kind": "auto", "estimated_tokens": 900, "included": true },
      { "name": "product_manifest", "kind": "manifest", "estimated_tokens": 40, "included": true }
    ],
    "classification": {
      "categories": ["user-visible"],
      "significance": "medium",
      "user_visible": true,
      "breaking": false,
      "security": false,
      "migration_heavy": false,
      "source": "model",
      "model": "deepseek/deepseek-v4-flash",
      "deterministic_signals": ["conventional:feat"],
      "disagreements": [],
      "reasons": ["model classified the parsed release evidence as user-visible"]
    },
    "cost": {
      "input_tokens": 1800,
      "output_tokens": 1000,
      "model_tier": "balanced",
      "model": "anthropic/claude-sonnet-5",
      "estimated_usd": 0.0068,
      "skip": false,
      "skip_reason": ""
    },
    "decision": {
      "action": "used",
      "reason": "balanced policy uses balanced model tier",
      "llm_required": true,
      "model_tier": "balanced"
    }
  },
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
- uses: misty-step/landmark@v1
  with:
    github-token: ${{ secrets.GH_RELEASE_TOKEN }}
    llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
    rss-feed-file: docs/releases.xml
    rss-max-entries: "50"
```

Landmark updates the feed on each synthesized release and commits the file back to your repo.

For automatic Slack notifications, set `slack-webhook-url`.

The `release-notes` output is still available for custom notifications:

```yaml
- name: Custom Notify Slack
  if: steps.landmark.outputs.released == 'true'
  run: |
    echo "${{ steps.landmark.outputs.release-notes }}" | post-to-slack
```

## Dogfooding Landmark

Landmark releases itself without pushing generated release commits directly to
protected `master`. The repository workflow has two phases:

- `prepare-release-pr` runs `cargo run --locked -- prepare-self-release`,
  updates `CHANGELOG.md`, `package.json`, `crates/landmark/Cargo.toml`, and
  `Cargo.lock` on `landmark/self-release`. It then opens or updates a release
  PR, which must pass the normal `merge-gate` before it can land.
- `publish-landed-release` runs on `master` pushes. It publishes a GitHub
  Release only when landed metadata is ahead of the latest semver tag.
- `build-release-assets` then builds the release binary for each supported
  target (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`,
  `aarch64-apple-darwin`, `x86_64-apple-darwin`) and `publish-release-assets`
  uploads them plus a `checksums.txt` to the GitHub Release, then runs
  Landmark in `synthesis-only` mode to update the release body and floating
  major tag. That synthesis pass is non-blocking because the release has
  already been published; failed or degraded synthesis is surfaced through
  Landmark outputs without turning a published release into a failed
  deployment. The floating major tag (and any `@v1`-pinned consumer) only
  moves once release assets are live, so consumers never resolve a tag whose
  binary is not yet downloadable.

The local replay oracle for this path is:

```bash
cargo run --locked -- replay-action \
  --evidence-dir .landmark/replay \
  --scenario self_release_pr_path
```

### Metadata Version Sync (Landmark Repo)

This repository keeps `package.json` and the Rust crate version aligned to release tags:

- `prepare-self-release` updates `package.json`,
  `crates/landmark/Cargo.toml`, and `Cargo.lock` before opening the release PR.
- `.releaserc.json` still runs `cargo run --locked -- update-version-metadata`
  for consumers using full semantic-release mode.
- The release commit includes `CHANGELOG.md`, `package.json`,
  `crates/landmark/Cargo.toml`, and `Cargo.lock`.
- CI runs `cargo run --locked -- check-version-sync` to fail fast when metadata drifts from the latest semver tag.

### Action Contract Validation (Landmark Repo)

The public action contract is checked from `action.yml`:

- `cargo run --locked -- check-action-contract` fails when the README inputs table diverges from action metadata.
- The same command scans examples, project docs, and release workflows for unknown or deprecated Landmark inputs.
- CI runs the contract check before tests so stale consumer instructions fail fast.

### Consumer Replay Harness (Landmark Repo)

Landmark's replay harness creates disposable git fixture repositories and fake
local GitHub/LLM endpoints, then exercises synthesis, release-body updates,
artifact writing, failure policy, and floating-tag behavior without production
secrets:

```bash
bin/replay-action --evidence-dir .landmark/replay
```

The command writes `.landmark/replay/replay-result.json` with action outputs,
generated notes, release body before/after state, git tags, structured logs, and
fake service requests. CI runs a bounded replay on pull requests, the full replay
on `master`, and uploads the evidence packet for inspection.

For a local one-command gate, run:

```bash
bin/gate
```

For branch protection, require these hosted checks:

- `merge-gate`: aggregate gate for the `local-gate` job that runs `bin/gate`.
- `trufflehog`: repository secret scan.

## Custom semantic-release Config

Landmark ships a default config at `configs/.releaserc.json`. If your repo has its own semantic-release config file (`.releaserc`, `.releaserc.json`, `.releaserc.yml`, `.releaserc.yaml`, `release.config.js`, `release.config.cjs`, or `release.config.mjs`), Landmark uses it instead of the bundled defaults.

This lets you customize branches, plugins, commit-analyzer rules, or anything else semantic-release supports.

If no config file is found, Landmark falls back to its bundled config with:

- `@semantic-release/commit-analyzer`
- `@semantic-release/release-notes-generator`
- `@semantic-release/changelog`
- `@semantic-release/git`
- `@semantic-release/github`

`CHANGELOG.md` is fully managed by `@semantic-release/changelog`. Do not keep a manual `# Changelog` or `## [Unreleased]` section in this repository, or release entries will be duplicated/mixed.

## Custom Prompt Templates

Landmark resolves the synthesis prompt template in this order:

1. **Explicit input** — `prompt-template-path: my-templates/release.md`
2. **Convention** — `.landmark/synthesis-prompt.md` in your repo root
3. **Bundled audience variant** — Landmark's built-in template selected by `audience` (default `general`)

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
- Import workspace configuration in one command, cutting setup time.

## Bug Fixes
- Webhook deliveries now retry more reliably when signatures expire.
```

Landmark intentionally omits internal-only changes (CI/tooling) from user-facing summaries.
