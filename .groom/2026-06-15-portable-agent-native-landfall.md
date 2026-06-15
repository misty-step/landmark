# Portable, Agent-Native Landfall Groom

Date: 2026-06-15

## Goal
Make Landfall the no-brainer release notes, changelog, and semantic-versioning
system across projects while reducing GitHub from a hard dependency to one
adapter and one hosted packaging path.

## Source Matrix

| Surface | Status | Evidence | Contribution |
| --- | --- | --- | --- |
| Tidy | complete | `jj status`; archived `008`-`012` with `harness-kit-checks backlog archive` | Active backlog was empty after archiving done tickets. |
| Product/value | complete | `README.md`, dogfood report, prior rollout memory | Landfall should be release intelligence, not only a GitHub Action. |
| Architecture | complete | `action.yml`; `crates/landfall/src/main.rs`; architecture lane | GitHub coupling is concentrated enough to extract adapters. |
| Operator UX | complete | README quick start, setup/fleet docs, dogfood findings | First-run value still assumes GitHub workflow/secrets and a Linux action binary. |
| Agent readiness | complete | `AGENTS.md`, CLI arg/help surface, artifact outputs | The agent contract was stale; schemas/describe/JSON discipline are missing. |
| Verification/security/ops | complete | security lane; `curl_json`; replay scenario list | README timeout/retry guarantee is false; side-effect coverage is incomplete. |
| External exemplars | partial | Existing semantic-release/release-please/changesets examples only | Need one non-GitHub pipeline example exercised by CI. |
| Research | skipped | No external facts needed for this groom | The key design gaps are visible in the live repo. |

## World-Class Target
Landfall should be a small, boring release-intelligence engine that any CI
system or agent can call:

- Input: git history, tags, release metadata, product manifest, model policy.
- Core: semantic version decision, technical changelog, contextual public notes,
  artifact/feed generation, publication plan, cost/status evidence.
- Outputs: files, JSON status, release artifacts, feed entries, webhooks, and
  optional provider mutations.
- Adapters: GitHub, GitLab, Forgejo/Gitea, generic git+artifact, local filesystem.
- Packaging: GitHub Action, shell/CLI, CI snippets, and eventually an agent API.

In that shape, GitHub remains an excellent first adapter, but not the place
where Landfall's domain logic lives.

## Gap Map

| Gap | Evidence | Impact |
| --- | --- | --- |
| GitHub Action is still the public center | README opens as a GitHub Action; `action.yml` requires `github-token`; examples are all GitHub workflows | Non-GitHub consumers cannot see the supported path. |
| Adapter boundary is implicit | `curl_json` uses GitHub REST headers; release/PR/failure/fleet commands use GitHub paths directly | Adding GitLab/Forgejo/local sinks would spread conditionals. |
| Local/git source exists but is not first-class | `backfill` can derive notes from tag ranges; synthesis takes changelog/release body/PR files | Portable mode should not require a GitHub Release. |
| Agent contract was stale | `AGENTS.md` described Python scripts and Python 3.12 after Rust migration | Cold agents would implement against the wrong system. |
| Machine contracts are weak | No versioned JSON schemas for manifest, status, replay, fleet plan, or artifact feed | Agents must infer structure from prose/code. |
| Verification overstates coverage | README promises bounded external calls; `curl_json` has no timeout/retry; replay misses several side-effecting commands | Green does not yet prove hosted release safety. |
| Adoption path still asks for too much upfront | Dogfood found widespread missing secrets and trigger drift; quick start assumes GitHub workflow setup | No 60-second zero-secret value path. |

## Strategy Themes
1. **Make the engine pipeline-neutral.** Extract release source, versioning,
   publication, failure-reporting, and fleet discovery behind provider adapters.
   The first provider matrix should be GitHub + local filesystem/git.

2. **Promote local mode to a product, not a fallback.** A shell or arbitrary CI
   pipeline should be able to run Landfall with only a checkout and optional LLM
   key, producing technical changelog, public notes, artifacts, RSS, and JSON
   status without touching a forge.

3. **Make agents first-class callers.** Publish schemas, a `describe --json`
   surface, deterministic JSON output modes, failure taxonomy, and sample
   evidence packets so agents can integrate Landfall without reading Rust.

4. **Make the verification loop honest.** Back every documented runtime
   guarantee with replay scenarios, especially external timeouts/retries,
   provider failures, secret handling, feeds, notifications, issue lifecycle,
   and release mutation.

5. **Keep GitHub excellent by making it just an adapter.** The GitHub Action
   should become a thin default pipeline assembled from the same CLI primitives
   that non-GitHub users call.

6. **Use fleet work to prove adoption, not just generate files.** Fleet should
   classify repos, identify lowest-friction modes, provision/verify secrets
   safely, open PRs when allowed, and monitor downstream release runs.

## Backlog Diff
Applied:

- Archived completed backlog items `008`-`012` into `backlog.d/_done/`.
- Rewrote `AGENTS.md` to match the Rust runtime and portable architecture goal.
- Added epics `013`-`018` for portability, agent contracts, verification,
  first-run adoption, fleet rollout, and release intelligence.

Proposed deletions/consolidations:

- Do not delete completed backlog history. It now documents the migration arc.
- Future tickets that only add GitHub-specific behavior should be folded into
  provider-adapter work unless the GitHub adapter itself is the product slice.

## Sequence
Now:

- Deliver `013`: pipeline-neutral core with GitHub/local provider separation.
- Deliver the timeout/retry and secret-boundary child from `015` early because
  README currently promises behavior the runtime does not implement.

Next:

- Deliver `014`: schemas and machine contracts, so agents can consume the new
  provider-neutral surface safely.
- Deliver `016`: zero-secret local and arbitrary-CI quickstarts once the core
  commands are stable.

Later:

- Deliver `017`: safe fleet rollout with provider classification and monitored
  downstream adoption.
- Deliver `018`: richer release intelligence and cost controls after the
  portable data plane is stable.

Blocked/unknown:

- Non-GitHub adapter priority needs one target choice for first proof:
  GitLab CI, Forgejo/Gitea, or generic shell-only. The local adapter should ship
  first regardless.

## Best Next Pickup
`backlog.d/013-make-landfall-pipeline-neutral.md` outranks the rest. It changes
the shape of every future feature: GitHub becomes an adapter, local execution
becomes a supported product path, and the GitHub Action can stay as a thin,
excellent default rather than a lock-in point.

## Residual Risk
- This groom used peer read-only lanes and live repo evidence, but no external
  ecosystem research. That is acceptable for the architecture decision; use
  research when choosing the first non-GitHub-hosted adapter.
- The active code still has unverified reliability/security gaps. The groom
  records them, but delivery must make them executable.
