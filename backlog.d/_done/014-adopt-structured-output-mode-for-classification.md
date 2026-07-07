# Adopt structured-output mode for classification

Priority: P2 · Status: pending · Estimate: S

## Goal
Replace prompt-only JSON compliance for the classification call with native
`response_format: json_schema` (strict mode) enforcement, removing reliance
on `extract_json_object`'s manual brace-matching to recover valid JSON from
free-form model output.

## Context
`~/.factory-lanes/wave1/landmark-model-refresh.md` (2026-07-02 model refresh
research, Part 2 architecture re-exam). Classification
(`request_release_classification`, release_classification.rs) asks for
strict JSON purely via prompt text — "Return only one strict JSON object
matching the requested schema" — and the already-constructed `output_schema`
object embedded in the request payload is descriptive prompt content only,
never passed as an actual `response_format` field. The response is then
parsed with `extract_json_object` (release_classification.rs:318), a manual
string-extraction routine, rather than relying on a schema-enforced response.

Every model in the classification roster (DeepSeek V4 family, Claude family,
GPT-5.x family) supports schema-enforced structured output on OpenRouter as
of mid-2026. This is the one concrete place the pipeline lags current
practice; the fix is mechanical — the `output_schema` value already
constructed for the prompt can be promoted directly into the request's
`response_format` field.

## Oracle
- [ ] `request_release_classification`'s HTTP payload includes a
      `response_format: {"type": "json_schema", "json_schema": {...},
      "strict": true}` (or the closest OpenRouter/model-supported
      equivalent) built from the existing `output_schema` value, instead of
      relying solely on prompt-text instruction.
- [ ] `extract_json_object` either becomes a defensive fallback (used only
      when `response_format` isn't honored by a given model/provider) or is
      removed if schema enforcement makes it unreachable in practice — pick
      one and make the reasoning explicit in the code/commit.
- [ ] A regression fixture exercises a real (or realistically mocked)
      response under the new `response_format` and confirms classification
      still parses correctly.
- [ ] `bin/gate` passes.

## Children
1. Add the `response_format` field to the classification request payload,
   derived from the existing `output_schema`.
2. Decide `extract_json_object`'s fate (fallback vs. removal) and implement.
3. Add/update the regression fixture.
4. Spot-check against at least one non-OpenAI model in the roster (e.g.
   DeepSeek or Claude via OpenRouter) to confirm `response_format` is
   honored cross-provider, not just by OpenAI-shaped backends — OpenRouter's
   passthrough behavior for `response_format` varies by upstream provider.

## Notes
Filed from the 2026-07-02 model refresh research. Not in scope: the
grounding-language prompt nudge (already a child of
`backlog.d/011-unify-classification-and-synthesis-grounding.md`) and the
classification-notice blockquote leaking into published release bodies
(named in the original `landmark-model-audit.md` secondary recommendations,
not re-ticketed here — output-hygiene issue, not a model/architecture one).
