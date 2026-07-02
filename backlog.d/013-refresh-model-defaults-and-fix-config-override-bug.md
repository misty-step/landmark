# Refresh model defaults and fix config-override bug

Priority: P1 · Status: pending · Estimate: S

## Goal
Replace the stale hardcoded model pins (`openai/gpt-4o-mini` classification,
`anthropic/claude-sonnet-4` synthesis) with current (July 2026) models, and
fix `release_classification_models()` silently ignoring `config.model`/
`config.model_policy` on OpenRouter.

## Context
`~/.factory-lanes/wave1/landmark-model-refresh.md` (2026-07-02 model refresh
research, triggered by the operator flagging both pins as "clearly incorrect
and incredibly dated"). Confirmed stale: `openai/gpt-4o-mini` is a mid-2024
model with no listing changes since; `anthropic/claude-sonnet-4` has two
newer same-vendor successors, including Claude Sonnet 5 (released
2026-06-30, two days before this audit) which is both cheaper ($2/$10 per 1M
vs Sonnet 4's pricing) and higher quality.

Three sites hardcode model-tier defaults independently: `cheap_model()`
(synthesis.rs:831), `rich_model()` (synthesis.rs:839), and
`release_classification_models()` (release_classification.rs:178). The third
has a live bug: on OpenRouter with `model_policy != "cheap"`, it
unconditionally pushes `"openai/gpt-4o-mini"` first, ignoring
`config.model`/`config.model_policy` even when the manifest's own
`model.primary` field (already defined in
`schemas/landmark-manifest.v1.schema.json`) is set. The config surface
exists; classification just doesn't honor it.

## Oracle
- [ ] Classification's cheap-tier default becomes `deepseek/deepseek-v4-flash`
      (confirmed $0.089/$0.18 per 1M input/output, 1M context, released
      2026-04-24); `anthropic/claude-haiku-4.5` is the fallback in the model
      chain.
- [ ] Synthesis's rich/balanced-high-significance default becomes
      `anthropic/claude-sonnet-5` ($2/$10 per 1M); the cheap tier becomes
      `anthropic/claude-haiku-4.5` ($1/$5 per 1M) — replacing gpt-4o-mini.
- [ ] `release_classification_models()` respects `config.model` /
      `config.model_policy` the same way `cheap_model()`/`rich_model()`
      already do — the manifest's `model.primary` is no longer silently
      overridden when set.
- [ ] The three independent default-model sites are collapsed into one
      `default_model_for_tier(tier, api_url)` source of truth (or an
      equivalent single point of definition).
- [ ] Each hardcoded default carries a `// model pin reviewed: YYYY-MM`
      comment so staleness is grep-able instead of rediscovered from
      scratch.
- [ ] A regression fixture covers the config-override bug: with
      `model.primary` set and `model.policy` not `cheap`, on an OpenRouter
      API URL, `release_classification_models()` returns the configured
      model first, not a hardcoded literal.
- [ ] `bin/gate` passes.

## Children
1. Add `default_model_for_tier` (or equivalent) as the single source of
   truth for cheap/rich/classification-cheap defaults; update all three call
   sites to use it.
2. Fix `release_classification_models()` to check `config.model`/
   `config.model_policy` before falling back to the tier default, matching
   `cheap_model()`/`rich_model()`'s existing precedence.
3. Update the three literal model IDs to the July 2026 picks above; add the
   reviewed-date comment convention.
4. Add the regression fixture for the config-override bug.
5. Consider a lightweight staleness check (e.g. `bin/gate` warns if a
   reviewed-date comment is older than ~9-12 months) so this doesn't
   fossilize again — optional, not blocking for this ticket's oracle.

## Notes
Filed from the 2026-07-02 model refresh research. Cross-link:
`backlog.d/011-unify-classification-and-synthesis-grounding.md` (separate,
input-grounding concern — not model selection); this ticket is purely about
which model IDs are pinned and whether config actually reaches them.
