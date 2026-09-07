# Landmark

## Release authority

Landmark is a portable release-intelligence runtime. The Rust CLI owns release
analysis, synthesis, classification, release-kit plans, provenance, approval
state, public release mutation, reconciliation, and the completed receipt.
GitHub Actions and other forge integrations are adapters, not the product
boundary. Non-GitHub callers use the same CLI, local git, manifests, and JSON
contracts; adapters cannot invent their own meaning of "published."

Release judgment and mutation are one responsibility: inspect before writing,
make retries idempotent, finish compatible partial state, and fail closed on
contradictions. `release-transaction prepare|bind|commit` emits the completed
receipt for artifact-bound releases. Self-release shares the reconciliation
core but is not artifact-bound: it carries no OCI/Sigstore identity and emits
no completed receipt. Tags, forge events, and synthesis-status outputs are not
substitutes for that authority.

Product builds own artifact construction, signing, and publication. Landmark
validates supplied manifests and binds immutable artifact identities; it does
not rebuild containers, packages, or binaries. Deployers consume completed
receipts and own promotion, verification, rollback, and convergence. Landmark
does not deploy.

Release-kit contracts delegate bespoke media, brand design, CMS publishing,
and long-running creative work to explicit producer adapters. Keep artifact
dependencies, acceptance, provenance, and approval in the typed kit rather
than embedding those producers in the core. See
[ADR 0004](docs/adr/0004-release-transaction-authority.md).

The [README](README.md), accepted [ADRs](docs/adr/), and versioned
[schemas](schemas/) define supported behavior. [VISION.md](VISION.md) is
optional rationale, not required reading or a product lock.

## Adapter safety

- Bind the Action's downloaded runtime and checksums to its own pinned release,
  not an independently selected latest binary.
- Pass untrusted Action inputs and secrets through `env:`; never interpolate
  them into `run:` shell source.
- Best-effort synthesis, artifact writes, and notifications report failure
  through outputs without blocking an otherwise valid release.
  `synthesis-required: "true"` is the explicit hard-failure policy.
- Token choice affects downstream automation: the ambient GitHub token's
  tags/releases do not trigger workflows like an App installation token's do.
  Preserve that distinction when changing publication adapters.

## Work authority

Work starts from the current operator request, not a timer or historical
queue. Linear owns selected current non-R90 work and priorities, not
runtime transaction state or release receipts; no ticket is required to act
on a direct request. Do not enable automatic intake or duplicate that work
in repo task lists. R90 continues to use Habitat. Dated `.groom/` and
`docs/dogfood/` records are historical evidence, not rollout authority or an
intake backlog. Keep run evidence in approved retained artifact storage and
link its revision from work summaries.

`agents/*/agent.md` and their task prompts are opt-in Forest runtime protocols,
separate from coding guidance. Their authorization, exact-Revision checks,
immutable evidence, and atomic publication requirements remain local to that
runtime. A poll is not permission to select work or activate a scheduler.

Contributor commands live in [CONTRIBUTING.md](CONTRIBUTING.md). Toolchain and
dependency requirements come from the manifests, `rust-toolchain.toml`, and
current CI, not a second version or gate inventory here.
