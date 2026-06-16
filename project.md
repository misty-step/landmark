# Project: Landmark

## Vision
Automated release pipeline GitHub Action — semantic-release + LLM synthesis — that turns conventional commits into user-facing release notes without manual effort.

**North Star:** Any repo (any language, public or private) ships beautiful release notes automatically. Technical changelogs for engineers; synthesized notes for users; marketing-ready artifacts (HTML, JSON, RSS, Slack) for the team. All from git activity alone.

**Target User:** Developers who ship software and want their users to know what changed — without writing release notes by hand.

**Key Differentiators:**
- LLM synthesis: commit lists → human-readable "What's New"
- Language-agnostic: no `package.json` in consumer repos, no ecosystem lock-in
- Multi-format output: MD, HTML, plaintext, JSON, RSS feed
- Distribution channels: GitHub Release, webhook, Slack Block Kit, RSS
- Fallback chains: primary model → fallback models → graceful degradation

**Current Focus:** Release integrity, contract validation, and consumer adoption after clearing the February backlog.

## Domain Glossary

| Term | Definition |
|------|-----------|
| synthesis | LLM step that converts technical changelog → user-facing "What's New" notes |
| full mode | Default: semantic-release + synthesis pipeline |
| synthesis-only mode | Skip semantic-release; synthesize notes for an existing tag |
| floating tag | Major version alias tag (e.g., `v1`) that points to latest release |
| changelog-source | Where synthesis pulls its input: `auto`, `changelog`, `release-body`, `prs` |
| audience | Built-in prompt variant: `general`, `developer`, `end-user`, `enterprise` |
| synthesis-required | If `true`, fail the action when synthesis or publication policy fails |
| preflight check | Pre-semantic-release validation: tag history integrity, config detection |
| notes artifact | Output file written by write-artifacts.py (MD/HTML/text/JSON) |
| RUNNER_TEMP | GitHub Actions temp dir; all intermediate files live here |

## Active Focus

- **Milestone:** Post-backlog reset
- **Backlog source:** `backlog.d/`
- **Theme:** Make Landmark trustworthy as reusable infrastructure: self-validating docs/contracts, explicit release integrity policy, live consumer replay, and lower-friction ecosystem adoption.

## Quality Bar

- [ ] LLM synthesis produces valid section-headed markdown (no raw commit lists)
- [ ] All failure modes emit clear, actionable `::warning::` messages
- [ ] Tests cover both the happy path and every failure branch
- [ ] Default action mode publishes the release even when synthesis fails; `synthesis-required` makes synthesis and publication policy failures explicit blockers
- [ ] No shell injection vectors in `run:` blocks (use `env:` for all inputs)
- [ ] Rust runtime handles all edge cases without crashing CI

## Patterns to Follow

### Shell Safety (action.yml run blocks)
```yaml
# NEVER interpolate inputs directly
run: landfall synthesize --api-key "${{ inputs.llm-api-key }}"  # BAD

# ALWAYS use env: block
env:
  API_KEY: ${{ inputs.llm-api-key }}
run: landfall synthesize --api-key "${API_KEY}"                  # GOOD
```

### Output Writing
```bash
# Multi-line outputs use a collision-resistant heredoc delimiter
{
  echo "notes<<${delimiter}"
  echo "${notes}"
  echo "${delimiter}"
} >> "${GITHUB_OUTPUT}"
```

### Graceful Degradation
```bash
# Synthesis steps exit 0 on failure (release must ship regardless)
set_output "succeeded" "false"
set_output "failure_stage" "synthesis"
exit 0   # NOT exit 1
```

### Rust Runtime Conventions
- One CLI owns Landmark behavior behind stable subcommands
- Structured diagnostics stay actionable in GitHub Actions logs
- HTTP calls use bounded failure semantics
- Tests live with the Rust crate; replay verifies action-level behavior

## Lessons Learned

| Decision | Outcome | Lesson |
|----------|---------|--------|
| interpolating inputs directly in `run:` | Shell injection risk | Always use `env:` block — enforced in memory |
| synthesis raising exit 1 on failure | Blocked releases | Synthesis always exits 0; `synthesis-required` opt-in strictness |
| single model with no fallback | Single point of failure | Fallback chain (primary → fallback models) is now standard |

---
*Last updated: 2026-02-23*
*Updated during: /groom session*
