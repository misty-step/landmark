# Build Landmark — Focused Release Pipeline GitHub Action

Read CLAUDE.md for full context. Build a working prototype of the Landmark GitHub Action.

## What to Build

### 1. action.yml (Composite GitHub Action)
Inputs:
- `github-token` (required): PAT with repo write access
- `llm-api-key`: API key for LLM synthesis (OpenRouter, OpenAI, or compatible)
- `llm-model` (default: anthropic/claude-sonnet-4): Primary model ID
- `llm-fallback-models` (default: google/gemini-2.5-flash,openai/gpt-4o-mini): Comma-separated fallbacks
- `llm-api-url` (default: OpenRouter): Chat completions endpoint
- `node-version` (default: 22): Node.js version
- `synthesis` (default: true): Whether to run LLM synthesis

Steps:
1. Setup Node.js
2. Use the checked-in Rust runtime for Landmark-owned behavior
3. Install dependencies (semantic-release + plugins)
4. Run semantic-release (generates changelog, bumps version, creates release)
5. If synthesis=true: Run the Rust runtime to generate user-facing notes
6. Update the GitHub Release body with synthesized notes

### 2. Rust synthesize command
- Takes the technical changelog (from semantic-release output or CHANGELOG.md diff)
- Calls any OpenAI-compatible API to synthesize user-facing release notes
- Uses the prompt template from templates/synthesis-prompt.md
- Tries primary model first, then fallback models in order
- Outputs the synthesized notes to stdout

### 3. Rust update-release command
- Takes the synthesized notes and the release tag
- Updates the GitHub Release body via GitHub API
- Prepends "## What's New" (user-facing) above the technical notes

### 4. templates/synthesis-prompt.md
- Prompt template with placeholders: {{PRODUCT_NAME}}, {{VERSION}}, {{TECHNICAL_CHANGELOG}}
- Instructs the LLM to write benefit-focused, user-facing release notes
- Groups by: New Features, Improvements, Bug Fixes
- Skips internal/CI changes

### 5. configs/.releaserc.json
- Default semantic-release config that works for most repos
- Plugins: commit-analyzer, release-notes-generator, changelog, npm (no publish), git, github

### 6. package.json
- Dependencies: semantic-release and all plugins
- No runtime deps beyond that

### 7. README.md
- Clear, concise documentation
- Quick start: how to add Landmark to a repo
- Configuration options
- Example output (before/after: technical vs user-facing notes)

## Technical Notes
- Uses OpenRouter by default: POST https://openrouter.ai/api/v1/chat/completions
- Default model: anthropic/claude-sonnet-4 with fallbacks to gemini-2.5-flash and gpt-4o-mini
- The synthesis doesn't need to be agentic — a single chat completion is fine
- Keep HTTP calls inside the Rust runtime and preserve token redaction boundaries

## Git Workflow
- Commit frequently with conventional commit messages
- When done, push to origin master
- Write TASK_COMPLETE when finished
