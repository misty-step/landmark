# Project: Landmark

## Vision
Portable release-intelligence runtime that turns git, package, docs, PR, and
release state into a complete, evidence-backed release kit.

**North Star:** Any repo (any language, public or private) ships with a complete
final-mile release packet: version decision, technical changelog, public notes,
artifact/feed outputs, migration/docs guidance, announcement drafts, and typed
contracts for richer producers such as demo videos, GIFs, images, blog posts,
and essays. All grounded in release facts and reviewable evidence.

**Target User:** Developers and release operators who ship software and need the
right audiences to understand what changed, why it matters, how to adopt it, and
which launch artifacts remain blocked or approved.

**Key Differentiators:**
- Release intelligence: commits, PRs, docs, packages, tags, prior releases, and
  provider state -> typed release context and artifact plan
- LLM synthesis: technical release facts -> audience-specific output drafts
- Language-agnostic: no `package.json` in consumer repos, no ecosystem lock-in
- Release kit contract: planned/produced/verified artifacts, producer contracts,
  provenance, approvals, and blockers
- Multi-format output: MD, HTML, plaintext, JSON, RSS feed, webhooks, Slack, and
  adapter contracts for media/docs/blog producers
- Distribution channels: GitHub Release, webhook, Slack Block Kit, RSS
- Fallback chains: primary model → fallback models → graceful degradation

**Current Focus:** Deepen Landmark as the final-mile release-intelligence core:
release facts, artifact plans, evidence, adoption safety, and producer contracts
before adding specialized media or publishing engines.

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
| release kit | Typed packet of release facts, recommended outputs, artifact status, provenance, approvals, and producer contracts |
| producer adapter | Explicit local, browser, service, harness, or human boundary that turns release-kit inputs into a rich artifact such as a video, GIF, image, essay, docs patch, or blog draft |
| final-mile artifact | Any output needed to ship the release beyond versioning: docs updates, migration guide, demo script, video, GIF, image, blog post, announcement copy, feed item, or social copy |

## Active Focus

- **Milestone:** Post-backlog reset
- **Backlog source:** `backlog.d/`
- **Theme:** Make Landmark trustworthy as reusable release-intelligence
  infrastructure: self-validating docs/contracts, explicit release integrity
  policy, live consumer replay, safe fleet adoption, and release-kit producer
  contracts.

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
run: landmark synthesize --api-key "${{ inputs.llm-api-key }}"  # BAD

# ALWAYS use env: block
env:
  API_KEY: ${{ inputs.llm-api-key }}
run: landmark synthesize --api-key "${API_KEY}"                  # GOOD
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
