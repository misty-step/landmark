# Make release verification prove every documented guarantee

Priority: P0 · Status: pending · Estimate: XL

## Goal
Ensure `bin/gate` and replay evidence prove Landfall's documented reliability,
security, and side-effect behavior before releases ship.

## Oracle
- [ ] External GitHub, forge, webhook, Slack, and LLM calls have bounded timeout and retry policy with replay tests for hung, slow, 429, and 5xx providers.
- [ ] No secret-bearing token or API key is passed through argv in the runtime hot path.
- [ ] Error, log, status, and evidence outputs redact configured secrets.
- [ ] Replay scenarios cover every side-effecting action subcommand: release mutation, artifact writes, RSS/feed commit, webhook, Slack, failure issue close/report, floating-tag movement, and synthesis status emission.
- [ ] A meta-test fails when `action.yml` invokes a `dist/landfall` subcommand that has no replay coverage.
- [ ] README reliability/security claims are tied to executable checks.

## Children
1. Replace bare `curl` invocations with a bounded request helper or add strict timeout/retry flags and tests at the current chokepoint.
2. Move tokens from argv to environment, stdin, or config files and add an argv-leak regression test.
3. Add a redaction helper used by all error/log/evidence paths.
4. Expand fake provider services and replay scenarios for notifications, feeds, failure issues, and force-pushed floating tags.
5. Add a contract check that maps documented guarantees to replay/test coverage.
6. Upload or link replay evidence in CI failures and groom/deliver closeouts.

## Notes
- Evidence: `curl_json` currently passes `Authorization` as a curl argv header and has no timeout/retry flags.
- Evidence: README says external GitHub and LLM calls use bounded timeouts and retry policy.
- Evidence: current replay coverage focuses on release/LLM paths and does not cover all side-effecting action steps.
- Why: portability increases the chance Landfall runs on shared or long-lived hosts, where secrets and hung providers are higher-impact.
