# Cover the remaining classify_failure branches with tests

Priority: P2 · Status: done · Estimate: S

## Goal
`classify_failure` (`crates/landmark/src/errors.rs:26-104`) is the function
behind every `--error-format json` failure envelope's `code`/`stage`/
`retryable`/`user_action` fields — an explicitly agent-native contract
(`docs/agent-integration.md` documents it directly). Only 2 of its 10 branches
have a pinned test today.

## Oracle
- [x] A test exists for each remaining branch of `classify_failure`:
      `provider_outage` (429/rate-limit/timeout), `budget_skip`
      (budget/model.policy=off), `synthesis_degradation`
      (degraded/quality), `publication_mutation_failure` (release
      body/publish-release-body), `feed_failure` (rss/feed),
      `artifact_write_failure` (write/file/permission), `invalid_input`
      (unsupported provider/requires/must), and the `command_failed`
      catch-all for a message matching none of the above.
- [x] Each test asserts `code`, `stage`, and `retryable` (not just that a
      match occurred) so silent branch reordering or overlap regressions get
      caught.
- [x] `cargo test --locked` and `bin/gate` pass.

## Evidence
`failure_classifier_covers_remaining_branches` (`crates/landmark/src/tests.rs`)
adds all 8 remaining cases. Each message was hand-traced against every
earlier branch's trigger substrings to make sure it hits only its intended
branch (documented inline: the function is an order-sensitive first-match
if/else-if chain, e.g. `"could not update the release body"` must avoid
`"auth"`/`"github-token"`/`"429"`/`"budget"`/`"degraded"` etc. to actually land
on `publication_mutation_failure` instead of an earlier branch). All 10 cases
(2 pre-existing + 7 new named branches + the `command_failed` catch-all)
passed on the first run, confirming the trace was correct.

## Notes
Verified live: `grep -rn "classify_failure" crates/landmark/src` shows only
two call sites in `crates/landmark/src/tests.rs` (`--publish-release-body
requires --github-token` and `manifest changelog.source must be auto`),
covering `provider_auth` and `invalid_changelog_source` only. The function's
branch order matters (it's a first-match `if`/`else if` chain over
substring checks on a lowercased message), so untested branches are exactly
where an added/reordered branch could silently steal matches from another
without any test failing.

**Why:** confirmed by direct `grep` during this groom pass; not called out in
the teardown report, which focused on classification/version-decision rather
than the failure-envelope taxonomy.
