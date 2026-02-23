# Implementation Retro

---

## Issue #88 — dx: fail-fast on LLM config mismatch (2026-02-23)

**Predicted effort:** M (1-3 days)
**Actual effort:** S (< 1 day, ~1.5h including polish pass)

### Scope Changes

Added beyond original spec:
- CI umbrella job fix (discovered during PR checks — PR was hanging on a required `"CI"` check that didn't exist)
- Shell injection fix on the healthcheck step (`${{ inputs.* }}` → `env:` block), part of standing #93 scope but touched the same step
- Polish pass findings: unquoted bash var → array, "Synthesis skipped:" prefix removed from fatal path, `startswith()` for URL check, 3 additional tests (403 fatal, 403 warn-only, RequestException)

Not added (excluded by scope):
- Issue #93 (other `${{ inputs.* }}` injection vectors beyond the healthcheck step) — tracked separately

### Blockers

None. Issue was well-specced with acceptance criteria, affected files, and example code. Went straight to implementation.

### Reusable Pattern for Future Scoping

**"Validate before publish" features are almost always smaller than they look.** The healthcheck already existed (`healthcheck.py` was complete, `probe_api` worked). The scope was: (1) auto-trigger condition change, (2) warn-only mode, (3) better error message. Each is a small delta.

When grooming similar "fail-fast / validate early" issues, ask: does the validation mechanism already exist? If yes, scope is likely S, not M.

**PR polish added ~30 min but improved correctness** (403/network test coverage was a real gap, the message framing was genuinely misleading). Polish pass pays dividends on user-facing DX features where error message quality matters.

---
