# Landfall — Focused Release Pipeline

## What This Is
A reusable GitHub Action that handles the complete release pipeline:
1. Analyze conventional commits to determine version bump
2. Generate technical changelog (CHANGELOG.md)
3. Push version bump + changelog to repo
4. Create GitHub Release
5. LLM-synthesize user-facing release notes from technical changelog
6. Update GitHub Release body with user-facing notes

## Architecture
Composite GitHub Action with these steps:
- `semantic-release` handles steps 1-4 (proven, battle-tested)
- Custom Python script handles step 5-6 (LLM synthesis via OpenAI-compatible API)

## Key Design Decisions
- **Unix philosophy**: This does ONE thing — releases. Not code review, not monitoring.
- **Wraps semantic-release**: Don't reinvent the wheel. Extend it.
- **LLM synthesis is the value-add**: Technical changelogs exist. User-facing notes don't.
- **OpenRouter by default**: Supports provider choice and model fallback chains.
- **Reusable Action**: Any repo can opt in with a simple workflow file.

## File Structure
```
landfall/
├── action.yml              # Reusable GitHub Action (called by repos)
├── scripts/
│   ├── synthesize.py       # LLM synthesis of user-facing notes
│   └── update-release.py   # Updates GitHub Release body
├── templates/
│   └── synthesis-prompt.md # Prompt template for LLM
├── configs/
│   └── .releaserc.json    # Default semantic-release config
├── README.md
├── CLAUDE.md               # This file
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
      - uses: misty-step/landfall@v1
        with:
          github-token: ${{ secrets.GH_RELEASE_TOKEN }}
          llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
          # Optional:
          # llm-model: anthropic/claude-sonnet-4
          # llm-fallback-models: "google/gemini-2.5-flash,openai/gpt-4o-mini"
```

## Requirements
- Node.js 22+
- Python 3.12+
- `GH_RELEASE_TOKEN` secret (PAT with repo write + admin bypass)
- `OPENROUTER_API_KEY` secret (or another compatible provider API key)
