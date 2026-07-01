# Landmark Fleet Integration Playbook

This is the factory-wide adoption standard for Landmark. Use it when wiring a
Misty Step repo so release intelligence is consistent across lanes.

Landmark owns release truth: version decisions, technical changelogs,
user-facing release-note synthesis, release-kit plans, artifact paths,
provenance, and machine-readable evidence. Consumer repos should add the
smallest surface that matches their current release ownership.

## First Pass

Run the local analyzer from a Landmark checkout before editing the consumer
repo:

```bash
target/debug/landmark init --repo-root /path/to/repo --dry-run
target/debug/landmark setup --repo-root /path/to/repo --dry-run
```

The first command previews the manifest. The second prints the recommended
mode, workflow family, required secrets, permissions, and ready-to-copy workflow
candidates. For a fleet pass, use the planner:

```bash
target/debug/landmark fleet scan --owner misty-step --active-only --output /tmp/landmark-fleet.json
target/debug/landmark fleet plan --input /tmp/landmark-fleet.json --output-dir /tmp/landmark-fleet-plan
target/debug/landmark fleet open-prs --dry-run --plan-dir /tmp/landmark-fleet-plan --output-dir /tmp/landmark-fleet-prs
```

Use `--deep-checks` for a smaller owner slice when you need GitHub to verify
branch protection and Actions secret names. Secret values are never requested or
printed.

## Files

Every integrated repo gets a manifest unless it already has one:

```yaml
product:
  name: <Product>
  description: <One-line product context for release notes.>
audience: developer
voice: Clear, concrete, and specific to shipped behavior.
changelog:
  source: auto
release:
  profile: synthesis-only
model:
  policy: balanced
budget:
  max_input_tokens: 12000
  max_output_tokens: 1200
```

Set `release.profile: full` only when Landmark should own semantic-release for
the repo. Keep `synthesis-only` when another tool or a human already creates the
GitHub Release.

## Choose The Workflow Shape

### Existing Release Producer

If the repo already uses release-please, Changesets, manual GitHub Releases, or
a custom release job, attach Landmark after that producer in `synthesis-only`
mode. Keep the existing workflow path when patching an existing release
workflow.

```yaml
- uses: misty-step/landmark@v1
  with:
    mode: synthesis-only
    healthcheck: "true"
    release-tag: ${{ steps.release.outputs.tag_name }}
    github-token: ${{ secrets.GH_RELEASE_TOKEN }}
    llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
```

For manual GitHub Releases, add `.github/workflows/landmark-release.yml`:

```yaml
name: Synthesize Release Notes

on:
  release:
    types: [published]

permissions:
  contents: write
  issues: write
  pull-requests: write

jobs:
  synthesize:
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: misty-step/landmark@v1
        with:
          mode: synthesis-only
          healthcheck: "true"
          node-version: "24"
          release-tag: ${{ github.event.release.tag_name }}
          github-token: ${{ secrets.GH_RELEASE_TOKEN }}
          llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
          changelog-source: auto
```

### Landmark-Owned Releases

Use full mode only when the repo is ready for Landmark to run semantic-release.
Trigger it from the repo's canonical gate workflow so a release is impossible
without the repo's normal verification.

```yaml
name: Release Intelligence

on:
  workflow_run:
    workflows:
      - <canonical gate workflow name>
    types:
      - completed
    branches:
      - <default branch>
  workflow_dispatch:

permissions:
  contents: write
  issues: write
  pull-requests: write

concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: false

jobs:
  landmark:
    if: github.event_name == 'workflow_dispatch' || github.event.workflow_run.conclusion == 'success'
    runs-on: ubuntu-latest
    timeout-minutes: 15
    steps:
      - name: Checkout repository history
        uses: actions/checkout@v4
        with:
          fetch-depth: 0
          persist-credentials: false
      - name: Run Landmark
        uses: misty-step/landmark@v1
        with:
          mode: full
          healthcheck: "true"
          github-token: ${{ secrets.GH_RELEASE_TOKEN }}
          llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
          node-version: "24"
          synthesis: "true"
          synthesis-required: "false"
```

## Required Secrets And Permissions

GitHub workflows need:

- `GH_RELEASE_TOKEN`: repository write access for releases and release-body
  updates.
- `OPENROUTER_API_KEY`: LLM synthesis. Missing or stale keys should not block
  release unless the repo deliberately sets `synthesis-required: "true"`.

Declare `contents: write`, `issues: write`, and `pull-requests: write`.

## Verification

Before opening a PR:

1. Run the repo's canonical gate.
2. Run `target/debug/landmark setup --repo-root /path/to/repo --dry-run` and
   confirm the selected mode still matches the files.
3. For a Landmark repo change, run `bin/gate`.

Before merging a consumer PR, confirm the branch includes only the manifest,
the release workflow or existing workflow patch, and any repo-local doc pointer
needed by that repo. Do not commit release artifacts generated by a dry run
unless the repo explicitly tracks them.
