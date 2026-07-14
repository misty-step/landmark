use crate::*;

#[derive(Parser)]
#[command(name = "landmark", version)]
#[command(about = "Rust runtime for the Landmark release action")]
pub(crate) struct Cli {
    /// Error output format: text or json (json emits a stable code/stage/retryable/user_action envelope)
    #[arg(long = "error-format", global = true, default_value = "text")]
    pub(crate) error_format: String,
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Print the agent-native self-description document (commands, schemas, examples, failure taxonomy)
    Describe(DescribeArgs),
    /// Infer a starter .landmark.yml manifest from repo signals
    Init(InitArgs),
    /// Validate manifest enums and repo signals before a release run
    Doctor(DoctorArgs),
    /// Resolve manifest-derived defaults for GitHub Action inputs
    ManifestDefaults(ManifestDefaultsArgs),
    /// Probe an LLM API key and model before synthesis runs
    Healthcheck(HealthcheckArgs),
    /// Validate git tag history integrity before semantic-release runs
    PreflightTags,
    /// Download an existing GitHub Release body as a changelog source
    FetchReleaseBody(FetchReleaseBodyArgs),
    /// Build a changelog from merged PR titles since the previous release
    ExtractPrs(ExtractPrsArgs),
    /// Generate user-facing release notes from a technical changelog
    Synthesize(Box<SynthesizeArgs>),
    /// Evaluate synthesis publication and summary policy
    ReleasePolicy(ReleasePolicyArgs),
    /// Prepend synthesized notes onto an existing GitHub Release body
    UpdateRelease(UpdateReleaseArgs),
    /// Write release notes to markdown, plaintext, HTML, and JSON outputs
    WriteArtifacts(WriteArtifactsArgs),
    /// Update an RSS release feed file with the latest release notes
    UpdateFeed(UpdateFeedArgs),
    /// POST release notes to a configured webhook endpoint
    NotifyWebhook(NotifyWebhookArgs),
    /// POST an enriched release-kit event to a configured release feed receiver
    NotifyReleaseFeed(NotifyReleaseFeedArgs),
    /// POST release notes to a configured Slack webhook
    NotifySlack(NotifySlackArgs),
    /// Compute a release decision and write technical/public release artifacts
    Run(RunArgs),
    /// Prepare and bind an immutable release publication transaction without remote mutation
    ReleaseTransaction(ReleaseTransactionArgs),
    /// Print the floating major-version tag for a release tag (e.g. v1)
    FloatingTag(FloatingTagArgs),
    /// Close synthesis-failure issues once synthesis has recovered
    CloseResolvedFailures(FailureLifecycleArgs),
    /// Open a GitHub issue reporting a synthesis failure
    ReportSynthesisFailure(ReportFailureArgs),
    /// Write a version into package.json and Cargo.toml
    UpdateVersionMetadata(UpdateVersionArgs),
    /// Fail if package/Cargo metadata versions drift from the latest tag
    CheckVersionSync(CheckVersionArgs),
    /// Validate that action.yml, README, and schemas stay in sync
    CheckActionContract(CheckActionContractArgs),
    /// Run the disposable-repo replay scenarios that prove release behavior
    ReplayAction(ReplayArgs),
    /// Plan or write historical release artifacts for tags that predate Landmark
    Backfill(BackfillArgs),
    /// Diagnose release tooling and generate candidate Landmark workflows
    Setup(SetupArgs),
    /// Scan, plan, and open adoption PRs across many repositories
    Fleet(FleetArgs),
    /// Prepare Landmark's own self-release branch and pull request
    PrepareSelfRelease(PrepareSelfReleaseArgs),
    /// Publish Landmark's own GitHub Release once the release PR has landed
    PublishSelfRelease(PublishSelfReleaseArgs),
}

#[derive(Args)]
pub(crate) struct DescribeArgs {
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args)]
pub(crate) struct InitArgs {
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
    #[arg(long, default_value = ".landmark.yml")]
    pub(crate) output: PathBuf,
    #[arg(long = "dry-run")]
    pub(crate) dry_run: bool,
}

#[derive(Args)]
pub(crate) struct DoctorArgs {
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
    #[arg(long = "format", default_value = "text")]
    pub(crate) format: String,
}

#[derive(Args)]
pub(crate) struct ManifestDefaultsArgs {
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
    #[arg(long = "github-output", default_value = "")]
    pub(crate) github_output: String,
}

#[derive(Args)]
pub(crate) struct HealthcheckArgs {
    #[arg(long = "api-key")]
    pub(crate) api_key: String,
    #[arg(long)]
    pub(crate) model: String,
    #[arg(long = "api-url")]
    pub(crate) api_url: String,
    #[arg(long)]
    pub(crate) warn_only: bool,
}

#[derive(Args)]
pub(crate) struct FetchReleaseBodyArgs {
    #[arg(long = "github-token")]
    pub(crate) github_token: String,
    #[arg(long)]
    pub(crate) repository: String,
    #[arg(long = "release-tag")]
    pub(crate) release_tag: String,
    #[arg(long = "output-file")]
    pub(crate) output_file: PathBuf,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    pub(crate) api_base_url: String,
}

#[derive(Args)]
pub(crate) struct ExtractPrsArgs {
    #[arg(long = "github-token")]
    pub(crate) github_token: String,
    #[arg(long)]
    pub(crate) repository: String,
    #[arg(long = "release-tag")]
    pub(crate) release_tag: String,
    #[arg(long = "output-file")]
    pub(crate) output_file: PathBuf,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    pub(crate) api_base_url: String,
    /// Path to the local git checkout used to scope PRs to the release's tag range
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
}

#[derive(Args)]
pub(crate) struct SynthesizeArgs {
    /// API key for the LLM provider
    #[arg(long = "api-key")]
    pub(crate) api_key: String,
    /// Primary model ID to try first
    #[arg(long, default_value = "")]
    pub(crate) model: String,
    /// Model policy: cheap, balanced, rich, or off
    #[arg(long = "model-policy", default_value = "")]
    pub(crate) model_policy: String,
    /// Chat completions endpoint URL (OpenAI-compatible)
    #[arg(long = "api-url")]
    pub(crate) api_url: String,
    /// Comma-separated fallback model IDs tried in order if the primary fails
    #[arg(long = "fallback-models", default_value = "")]
    pub(crate) fallback_models: String,
    /// Product name injected into the synthesis prompt
    #[arg(long = "product-name", default_value = "")]
    pub(crate) product_name: String,
    /// One-line product description injected into the synthesis prompt
    #[arg(long = "product-description", default_value = "")]
    pub(crate) product_description: String,
    /// Tone and style guidance injected into the synthesis prompt
    #[arg(long = "voice-guide", default_value = "")]
    pub(crate) voice_guide: String,
    /// Prompt audience variant: general, developer, end-user, or enterprise
    #[arg(long)]
    pub(crate) audience: Option<String>,
    /// Technical source for synthesis: auto, changelog, release-body, or prs
    #[arg(long = "changelog-source")]
    pub(crate) changelog_source: Option<String>,
    /// Release version being synthesized
    #[arg(long)]
    pub(crate) version: String,
    /// Path to CHANGELOG.md to read as the technical source
    #[arg(long = "changelog-file")]
    pub(crate) changelog_file: PathBuf,
    /// Path to a fetched GitHub Release body to use as the technical source
    #[arg(long = "release-body-file", default_value = ".")]
    pub(crate) release_body_file: PathBuf,
    /// Path to an extracted PR changelog to use as the technical source
    #[arg(long = "pr-changelog-file", default_value = ".")]
    pub(crate) pr_changelog_file: PathBuf,
    /// Path to a custom synthesis prompt template overriding the audience default
    #[arg(long = "prompt-template", default_value = ".")]
    pub(crate) prompt_template: PathBuf,
    /// Path to write the synthesis quality verdict (valid/degraded/skipped)
    #[arg(long = "quality-file")]
    pub(crate) quality_file: PathBuf,
    /// Path to write the per-model attempt log as JSON
    #[arg(long = "attempts-file", default_value = ".")]
    pub(crate) attempts_file: PathBuf,
    /// Directory containing the built-in audience prompt templates
    #[arg(long = "templates-dir", default_value = "templates/prompts")]
    pub(crate) templates_dir: PathBuf,
    /// Path to the repository to read manifest and context from
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
    /// Estimate token cost and skip decision without calling the LLM
    #[arg(long = "dry-run-cost")]
    pub(crate) dry_run_cost: bool,
    /// Path to write the synthesis context metadata as JSON
    #[arg(long = "context-metadata-file", default_value = ".")]
    pub(crate) context_metadata_file: PathBuf,
    /// Path to write the claim-to-source grounding map as JSON
    #[arg(long = "claim-map-file", default_value = ".")]
    pub(crate) claim_map_file: PathBuf,
}

#[derive(Args)]
pub(crate) struct ReleasePolicyArgs {
    #[command(subcommand)]
    pub(crate) command: ReleasePolicyCommand,
}

#[derive(Subcommand)]
pub(crate) enum ReleasePolicyCommand {
    /// Decide whether synthesis failures should block release-body publication
    Publication(PublicationArgs),
    /// Summarize synthesis, publication, and notification outcomes as status JSON
    Summary(Box<SummaryArgs>),
}

#[derive(Args)]
pub(crate) struct PublicationArgs {
    #[arg(long = "synthesis-required")]
    pub(crate) synthesis_required: String,
    #[arg(long = "synthesis-strict")]
    pub(crate) synthesis_strict: String,
    #[arg(long = "synth-succeeded")]
    pub(crate) synth_succeeded: String,
    #[arg(long = "synth-quality", default_value = "")]
    pub(crate) synth_quality: String,
    #[arg(long = "synth-failure-stage", default_value = "")]
    pub(crate) synth_failure_stage: String,
    #[arg(long = "synth-failure-message", default_value = "")]
    pub(crate) synth_failure_message: String,
    #[arg(long = "github-output")]
    pub(crate) github_output: PathBuf,
}

#[derive(Args)]
pub(crate) struct SummaryArgs {
    #[arg(long = "synthesis-enabled")]
    pub(crate) synthesis_enabled: String,
    #[arg(long)]
    pub(crate) released: String,
    #[arg(long = "synth-succeeded")]
    pub(crate) synth_succeeded: String,
    #[arg(long = "synth-quality", default_value = "")]
    pub(crate) synth_quality: String,
    #[arg(long = "update-succeeded")]
    pub(crate) update_succeeded: String,
    #[arg(long = "synth-failure-stage", default_value = "")]
    pub(crate) synth_failure_stage: String,
    #[arg(long = "synth-failure-message", default_value = "")]
    pub(crate) synth_failure_message: String,
    #[arg(long = "update-failure-stage", default_value = "")]
    pub(crate) update_failure_stage: String,
    #[arg(long = "update-failure-message", default_value = "")]
    pub(crate) update_failure_message: String,
    #[arg(long = "artifact-succeeded", default_value = "")]
    pub(crate) artifact_succeeded: String,
    #[arg(long = "artifact-failure-stage", default_value = "")]
    pub(crate) artifact_failure_stage: String,
    #[arg(long = "artifact-failure-message", default_value = "")]
    pub(crate) artifact_failure_message: String,
    #[arg(long = "rss-enabled", default_value = "")]
    pub(crate) rss_enabled: String,
    #[arg(long = "rss-succeeded", default_value = "")]
    pub(crate) rss_succeeded: String,
    #[arg(long = "rss-failure-stage", default_value = "")]
    pub(crate) rss_failure_stage: String,
    #[arg(long = "rss-failure-message", default_value = "")]
    pub(crate) rss_failure_message: String,
    #[arg(long = "webhook-enabled", default_value = "")]
    pub(crate) webhook_enabled: String,
    #[arg(long = "webhook-sent", default_value = "")]
    pub(crate) webhook_sent: String,
    #[arg(long = "slack-enabled", default_value = "")]
    pub(crate) slack_enabled: String,
    #[arg(long = "slack-sent", default_value = "")]
    pub(crate) slack_sent: String,
    #[arg(long = "github-output")]
    pub(crate) github_output: PathBuf,
    #[arg(long = "attempts-file", default_value = ".")]
    pub(crate) attempts_file: String,
    #[arg(long = "context-metadata-file", default_value = ".")]
    pub(crate) context_metadata_file: String,
}

#[derive(Args)]
pub(crate) struct UpdateReleaseArgs {
    #[arg(long = "github-token")]
    pub(crate) github_token: String,
    #[arg(long)]
    pub(crate) repository: String,
    #[arg(long)]
    pub(crate) tag: String,
    #[arg(long = "notes-file")]
    pub(crate) notes_file: PathBuf,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    pub(crate) api_base_url: String,
}

#[derive(Args)]
pub(crate) struct WriteArtifactsArgs {
    #[arg(long = "notes-file")]
    pub(crate) notes_file: PathBuf,
    #[arg(long)]
    pub(crate) version: String,
    #[arg(long, default_value = "")]
    pub(crate) repository: String,
    #[arg(long = "release-url", default_value = "")]
    pub(crate) release_url: String,
    #[arg(long, default_value = "")]
    pub(crate) audience: String,
    #[arg(long = "output-file", default_value = "")]
    pub(crate) output_file: String,
    #[arg(long = "output-text-file", default_value = "")]
    pub(crate) output_text_file: String,
    #[arg(long = "output-html-file", default_value = "")]
    pub(crate) output_html_file: String,
    #[arg(long = "output-json", default_value = "")]
    pub(crate) output_json: String,
}

#[derive(Args)]
pub(crate) struct UpdateFeedArgs {
    #[arg(long = "feed-file")]
    pub(crate) feed_file: String,
    #[arg(long = "max-entries")]
    pub(crate) max_entries: usize,
    #[arg(long)]
    pub(crate) repository: String,
    #[arg(long = "release-tag")]
    pub(crate) release_tag: String,
    #[arg(long = "release-url")]
    pub(crate) release_url: String,
    #[arg(long = "notes-file")]
    pub(crate) notes_file: PathBuf,
    #[arg(long)]
    pub(crate) workspace: PathBuf,
}

#[derive(Args)]
pub(crate) struct NotifyWebhookArgs {
    #[arg(long = "webhook-url")]
    pub(crate) webhook_url: String,
    #[arg(long = "webhook-secret", default_value = "")]
    pub(crate) webhook_secret: String,
    #[arg(long)]
    pub(crate) version: String,
    #[arg(long)]
    pub(crate) repository: String,
    #[arg(long = "release-url")]
    pub(crate) release_url: String,
    #[arg(long = "notes-file")]
    pub(crate) notes_file: PathBuf,
}

#[derive(Args)]
pub(crate) struct NotifyReleaseFeedArgs {
    #[arg(long = "receiver-url", alias = "feed-url", default_value = "")]
    pub(crate) receiver_url: String,
    #[arg(long = "receiver-secret", alias = "feed-secret", default_value = "")]
    pub(crate) receiver_secret: String,
    #[arg(long = "evidence-file", default_value = ".landmark/run/evidence.json")]
    pub(crate) evidence_file: PathBuf,
    #[arg(
        long = "release-kit-file",
        default_value = ".landmark/run/release-kit.json"
    )]
    pub(crate) release_kit_file: PathBuf,
    #[arg(long = "receipt-file", default_value = "")]
    pub(crate) receipt_file: PathBuf,
}

#[derive(Args)]
pub(crate) struct NotifySlackArgs {
    #[arg(long = "slack-webhook-url")]
    pub(crate) slack_webhook_url: String,
    #[arg(long)]
    pub(crate) version: String,
    #[arg(long)]
    pub(crate) repository: String,
    #[arg(long = "release-url")]
    pub(crate) release_url: String,
    #[arg(long = "notes-file")]
    pub(crate) notes_file: PathBuf,
}

#[derive(Args)]
pub(crate) struct RunArgs {
    /// Release provider: local (no GitHub calls) or github (can mutate the release body)
    #[arg(long = "provider", default_value = "local")]
    pub(crate) provider: String,
    /// Path to the repository to analyze
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
    /// owner/repo; inferred from repo_root's directory name when omitted
    #[arg(long = "repository", default_value = "")]
    pub(crate) repository: String,
    /// Explicit release tag to use instead of computing one from commits
    #[arg(long = "release-tag", default_value = "")]
    pub(crate) release_tag: String,
    /// Previous release tag to diff against; inferred from the latest matching tag when omitted
    #[arg(long = "previous-tag", default_value = "")]
    pub(crate) previous_tag: String,
    /// GitHub token for provider=github; required with --publish-release-body
    #[arg(long = "github-token", default_value = "")]
    pub(crate) github_token: String,
    /// GitHub API base URL (override for GitHub Enterprise)
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    pub(crate) api_base_url: String,
    /// GitHub server URL used to build release links
    #[arg(long = "server-url", default_value = "")]
    pub(crate) server_url: String,
    /// Mutate the existing GitHub Release body with generated notes (provider=github only)
    #[arg(long = "publish-release-body")]
    pub(crate) publish_release_body: bool,
    /// Compute the release decision and print evidence without writing any files
    #[arg(long = "dry-run")]
    pub(crate) dry_run: bool,
    /// Use pre-written release notes instead of rendering them from commits
    #[arg(long = "notes-file", default_value = "")]
    pub(crate) notes_file: String,
    /// Directory for the evidence and release-kit JSON artifacts
    #[arg(long = "output-dir", default_value = ".landmark/run")]
    pub(crate) output_dir: PathBuf,
    /// Path to write the internal technical changelog
    #[arg(
        long = "technical-changelog-file",
        default_value = ".landmark/run/technical-changelog.md"
    )]
    pub(crate) technical_changelog_file: String,
    /// Path to write the run-evidence.v1 JSON packet
    #[arg(long = "evidence-file", default_value = ".landmark/run/evidence.json")]
    pub(crate) evidence_file: String,
    /// Markdown release notes output path; use {version} as a placeholder
    #[arg(long = "output-file", default_value = "docs/releases/{version}.md")]
    pub(crate) output_file: String,
    /// Plaintext release notes output path; use {version} as a placeholder
    #[arg(
        long = "output-text-file",
        default_value = "docs/releases/{version}.txt"
    )]
    pub(crate) output_text_file: String,
    /// HTML release notes output path; use {version} as a placeholder
    #[arg(
        long = "output-html-file",
        default_value = "docs/releases/{version}.html"
    )]
    pub(crate) output_html_file: String,
    /// Path to append a structured release-entry JSON record
    #[arg(long = "output-json", default_value = "docs/releases/releases.json")]
    pub(crate) output_json: String,
    /// Path to an RSS feed file to update with this release
    #[arg(long = "rss-feed-file", default_value = "docs/releases/feed.xml")]
    pub(crate) rss_feed_file: String,
    /// Maximum number of entries to retain in the RSS feed
    #[arg(long = "rss-max-entries", default_value_t = 50)]
    pub(crate) rss_max_entries: usize,
}

#[derive(Args)]
pub(crate) struct ReleaseTransactionArgs {
    #[command(subcommand)]
    pub(crate) command: ReleaseTransactionCommand,
}

#[derive(Subcommand)]
pub(crate) enum ReleaseTransactionCommand {
    /// Compute release identity and notes without creating a tag or release
    Prepare(PrepareReleaseTransactionArgs),
    /// Bind verified immutable artifacts to a prepared transaction
    Bind(BindReleaseTransactionArgs),
}

#[derive(Args)]
pub(crate) struct PrepareReleaseTransactionArgs {
    /// Path to the repository whose release is being prepared
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
    /// owner/repo for the release; inferred from repo-root when omitted
    #[arg(long, default_value = "")]
    pub(crate) repository: String,
    /// Explicit release tag instead of computing one from commits
    #[arg(long = "release-tag", default_value = "")]
    pub(crate) release_tag: String,
    /// Previous release tag; inferred from git history when omitted
    #[arg(long = "previous-tag", default_value = "")]
    pub(crate) previous_tag: String,
    /// Pre-written release notes; generated deterministically when omitted
    #[arg(long = "notes-file", default_value = "")]
    pub(crate) notes_file: String,
    /// Canonical transaction state file; prepare is compare-and-swap idempotent
    #[arg(long)]
    pub(crate) transaction: PathBuf,
}

#[derive(Args)]
pub(crate) struct BindReleaseTransactionArgs {
    /// Prepared or already-bound release transaction JSON
    #[arg(long)]
    pub(crate) transaction: PathBuf,
    /// Product-supplied immutable artifact manifest JSON
    #[arg(long = "artifact-manifest")]
    pub(crate) artifact_manifest: PathBuf,
    /// Local root containing every relative artifact path named by the manifest
    #[arg(long = "artifact-root")]
    pub(crate) artifact_root: PathBuf,
    /// Cosign-compatible verifier executable
    #[arg(long = "cosign", default_value = "cosign")]
    pub(crate) cosign: PathBuf,
    /// Trusted public key for cosign verify-blob; mutually exclusive with keyless identity
    #[arg(long = "verification-key")]
    pub(crate) verification_key: Option<PathBuf>,
    /// Expected keyless certificate identity
    #[arg(long = "certificate-identity", default_value = "")]
    pub(crate) certificate_identity: String,
    /// Expected keyless certificate OIDC issuer
    #[arg(long = "certificate-oidc-issuer", default_value = "")]
    pub(crate) certificate_oidc_issuer: String,
}

#[derive(Args)]
pub(crate) struct FloatingTagArgs {
    #[arg(long = "release-tag")]
    pub(crate) release_tag: String,
}

#[derive(Args)]
pub(crate) struct FailureLifecycleArgs {
    #[arg(long = "github-token")]
    pub(crate) github_token: String,
    #[arg(long)]
    pub(crate) repository: String,
    #[arg(long = "release-tag")]
    pub(crate) release_tag: String,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    pub(crate) api_base_url: String,
}

#[derive(Args)]
pub(crate) struct ReportFailureArgs {
    #[arg(long = "github-token")]
    pub(crate) github_token: String,
    #[arg(long)]
    pub(crate) repository: String,
    #[arg(long = "release-tag")]
    pub(crate) release_tag: String,
    #[arg(long = "workflow-run-url")]
    pub(crate) workflow_run_url: String,
    #[arg(long = "workflow-name")]
    pub(crate) workflow_name: String,
    #[arg(long = "failure-stage")]
    pub(crate) failure_stage: String,
    #[arg(long = "failure-message")]
    pub(crate) failure_message: String,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    pub(crate) api_base_url: String,
}

#[derive(Args)]
pub(crate) struct UpdateVersionArgs {
    #[arg(long)]
    pub(crate) version: String,
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
}

#[derive(Args)]
pub(crate) struct CheckVersionArgs {
    #[arg(long, default_value = "HEAD")]
    pub(crate) reference: String,
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
    #[arg(long = "allow-release-candidate")]
    pub(crate) allow_release_candidate: bool,
}

#[derive(Args)]
pub(crate) struct CheckActionContractArgs {
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
}

#[derive(Args)]
pub(crate) struct ReplayArgs {
    #[arg(long = "evidence-dir", default_value = "")]
    pub(crate) evidence_dir: String,
    #[arg(long = "scenario")]
    pub(crate) scenario: Vec<String>,
    #[arg(long = "format", default_value = "text")]
    pub(crate) format: String,
}

#[derive(Args)]
pub(crate) struct BackfillArgs {
    /// Path to the repository to analyze
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
    /// Earliest tag to backfill from; backfills all matching tags when omitted
    #[arg(long, default_value = "")]
    pub(crate) since: String,
    /// Backfill mode: artifacts-only (safe) or release-body (mutates GitHub Releases)
    #[arg(long, default_value = "artifacts-only")]
    pub(crate) mode: String,
    /// Preview the backfill plan without writing artifacts or releases
    #[arg(long = "dry-run")]
    pub(crate) dry_run: bool,
    /// owner/repo; required for mode=release-body
    #[arg(long = "repository", default_value = "")]
    pub(crate) repository: String,
    #[arg(
        long = "github-token",
        default_value = "",
        help = "GitHub token; defaults to GITHUB_TOKEN when omitted"
    )]
    pub(crate) github_token: String,
    /// GitHub API base URL (override for GitHub Enterprise)
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    pub(crate) api_base_url: String,
    /// Required alongside mode=release-body to confirm mutating existing releases
    #[arg(long = "confirm-release-body")]
    pub(crate) confirm_release_body: bool,
    /// Maximum number of tags to backfill; 0 means no limit
    #[arg(long = "max-tags", default_value_t = 0)]
    pub(crate) max_tags: usize,
    /// Markdown release notes output path; use {version} as a placeholder
    #[arg(long = "output-file", default_value = "docs/releases/{version}.md")]
    pub(crate) output_file: String,
    /// Plaintext release notes output path; use {version} as a placeholder
    #[arg(
        long = "output-text-file",
        default_value = "docs/releases/{version}.txt"
    )]
    pub(crate) output_text_file: String,
    /// HTML release notes output path; use {version} as a placeholder
    #[arg(
        long = "output-html-file",
        default_value = "docs/releases/{version}.html"
    )]
    pub(crate) output_html_file: String,
    /// Path to append structured release-entry JSON records
    #[arg(long = "output-json", default_value = "docs/releases/releases.json")]
    pub(crate) output_json: String,
    /// Path to an RSS feed file to update with backfilled entries
    #[arg(long = "rss-feed-file", default_value = "docs/releases/feed.xml")]
    pub(crate) rss_feed_file: String,
    /// Maximum number of entries to retain in the RSS feed
    #[arg(long = "rss-max-entries", default_value_t = 50)]
    pub(crate) rss_max_entries: usize,
    /// Path to the backfill progress manifest, used to resume interrupted runs
    #[arg(
        long = "resume-file",
        default_value = ".landmark/backfill-manifest.json"
    )]
    pub(crate) resume_file: PathBuf,
}

#[derive(Args)]
pub(crate) struct SetupArgs {
    /// Path to the repository to diagnose
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
    /// Directory to write generated workflow candidates; skips writing when empty
    #[arg(long = "output-dir", default_value = "")]
    pub(crate) output_dir: String,
    /// Print the diagnosis and recommendation without writing workflow files
    #[arg(long = "dry-run")]
    pub(crate) dry_run: bool,
}

#[derive(Args)]
pub(crate) struct FleetArgs {
    #[command(subcommand)]
    pub(crate) command: FleetCommand,
}

#[derive(Subcommand)]
pub(crate) enum FleetCommand {
    /// Scan GitHub repositories for release tooling and Landmark adoption signals
    Scan(FleetScanArgs),
    /// Turn a fleet scan into a ranked, per-repo adoption plan
    Plan(FleetPlanArgs),
    /// Render and optionally open adoption PRs from a fleet plan
    OpenPrs(FleetOpenPrsArgs),
}

#[derive(Args)]
#[command(
    after_help = "Token note: if --github-token is omitted, Landmark reads GITHUB_TOKEN from the environment. Prefer the environment to avoid token-bearing argv."
)]
pub(crate) struct FleetScanArgs {
    /// GitHub user or organization to scan; repeatable
    #[arg(long)]
    pub(crate) owner: Vec<String>,
    /// Path to write the fleet scan JSON
    #[arg(long, default_value = ".landmark/fleet.json")]
    pub(crate) output: PathBuf,
    /// Maximum number of repositories to scan; 0 means no limit
    #[arg(long = "max-repos", default_value_t = 0)]
    pub(crate) max_repos: usize,
    /// Skip archived and inactive repositories
    #[arg(long = "active-only")]
    pub(crate) active_only: bool,
    /// Number of repositories to scan concurrently
    #[arg(long = "concurrency", default_value_t = 4)]
    pub(crate) concurrency: usize,
    /// Fetch branch-protection and workflow content, not just repo metadata
    #[arg(long = "deep-checks")]
    pub(crate) deep_checks: bool,
    /// GitHub API base URL (override for GitHub Enterprise)
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    pub(crate) api_base_url: String,
    /// GitHub token; defaults to GITHUB_TOKEN when omitted
    #[arg(long = "github-token", default_value = "")]
    pub(crate) github_token: String,
    #[arg(long = "fixture", hide = true, default_value = "")]
    pub(crate) fixture: String,
    /// Output format: text or json
    #[arg(long = "format", default_value = "text")]
    pub(crate) format: String,
}

#[derive(Args)]
pub(crate) struct FleetPlanArgs {
    /// Path to a fleet scan JSON produced by fleet scan
    #[arg(long, default_value = ".landmark/fleet.json")]
    pub(crate) input: PathBuf,
    /// Directory to write the per-repo adoption plan
    #[arg(long = "output-dir", default_value = ".landmark/fleet-plan")]
    pub(crate) output_dir: PathBuf,
    /// Output format: text or json
    #[arg(long = "format", default_value = "text")]
    pub(crate) format: String,
}

#[derive(Args)]
pub(crate) struct FleetOpenPrsArgs {
    /// Directory containing the fleet adoption plan
    #[arg(long = "plan-dir", default_value = ".landmark/fleet-plan")]
    pub(crate) plan_dir: PathBuf,
    /// Directory to write rendered PR bodies and diffs
    #[arg(long = "output-dir", default_value = ".landmark/fleet-plan/prs")]
    pub(crate) output_dir: PathBuf,
    /// Render PR content without pushing branches or opening PRs
    #[arg(long = "dry-run")]
    pub(crate) dry_run: bool,
    /// Required alongside a non-dry-run to confirm pushing to remote repositories
    #[arg(long = "confirm-remote")]
    pub(crate) confirm_remote: bool,
    /// Maximum number of PRs to open; 0 means no limit
    #[arg(long = "max-prs", default_value_t = 0)]
    pub(crate) max_prs: usize,
    /// Output format: text or json
    #[arg(long = "format", default_value = "text")]
    pub(crate) format: String,
}

#[derive(Args)]
pub(crate) struct PrepareSelfReleaseArgs {
    /// Path to the repository to prepare a release for
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
    /// owner/repo for the release
    #[arg(long, default_value = "misty-step/landmark")]
    pub(crate) repository: String,
    /// Branch name for the self-release pull request
    #[arg(long = "release-branch", default_value = "landmark/self-release")]
    pub(crate) release_branch: String,
    /// Path to a GITHUB_OUTPUT file to append step outputs to
    #[arg(long = "github-output", default_value = "")]
    pub(crate) github_output: String,
}

#[derive(Args)]
pub(crate) struct PublishSelfReleaseArgs {
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
    #[arg(long = "github-token")]
    pub(crate) github_token: String,
    #[arg(long)]
    pub(crate) repository: String,
    #[arg(long = "target-sha")]
    pub(crate) target_sha: String,
    #[arg(long = "github-output", default_value = "")]
    pub(crate) github_output: String,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    pub(crate) api_base_url: String,
}

#[derive(Serialize)]
pub(crate) struct SelfReleasePlan {
    pub(crate) released: bool,
    pub(crate) reason: String,
    pub(crate) latest_version: String,
    pub(crate) next_version: String,
    /// The bump before pre-stable demotion (`major`/`minor`/`patch`/`none`), and
    /// whether pre-stable Cargo-style 0.x rules were applied. Keeps the demotion
    /// visible in the plan. See card landmark-016.
    pub(crate) raw_bump: String,
    pub(crate) stability: String,
    pub(crate) release_tag: String,
    pub(crate) release_branch: String,
    pub(crate) pull_request_title: String,
    pub(crate) commit_message: String,
    pub(crate) changed_files: Vec<String>,
    pub(crate) changelog: String,
    pub(crate) commits: Vec<SelfReleaseCommit>,
    pub(crate) decisive_commit: Option<String>,
    pub(crate) unknown_commits: Vec<String>,
}

#[derive(Clone, Serialize)]
pub(crate) struct SelfReleaseCommit {
    pub(crate) hash: String,
    pub(crate) short_hash: String,
    pub(crate) subject: String,
    pub(crate) category: String,
    pub(crate) scope: String,
    pub(crate) description: String,
    pub(crate) breaking: bool,
}

#[derive(Serialize)]
pub(crate) struct SelfReleasePublish {
    pub(crate) published: bool,
    pub(crate) reason: String,
    pub(crate) latest_version: String,
    pub(crate) version: String,
    pub(crate) release_tag: String,
    pub(crate) release_url: String,
}

pub(crate) fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Describe(args) => describe(args),
        Commands::Init(args) => init(args),
        Commands::Doctor(args) => doctor(args),
        Commands::ManifestDefaults(args) => manifest_defaults(args),
        Commands::Healthcheck(args) => healthcheck(args),
        Commands::PreflightTags => preflight_tags(),
        Commands::FetchReleaseBody(args) => fetch_release_body(args),
        Commands::ExtractPrs(args) => extract_prs(args),
        Commands::Synthesize(args) => synthesize(*args),
        Commands::ReleasePolicy(args) => release_policy(args),
        Commands::UpdateRelease(args) => update_release(args),
        Commands::WriteArtifacts(args) => write_artifacts(args),
        Commands::UpdateFeed(args) => update_feed(args),
        Commands::NotifyWebhook(args) => notify_webhook(args),
        Commands::NotifyReleaseFeed(args) => notify_release_feed(args),
        Commands::NotifySlack(args) => notify_slack(args),
        Commands::Run(args) => run_pipeline(args),
        Commands::ReleaseTransaction(args) => release_transaction(args),
        Commands::FloatingTag(args) => {
            if let Some(tag) = parse_major_tag(&args.release_tag) {
                println!("{tag}");
            }
            Ok(())
        }
        Commands::CloseResolvedFailures(args) => close_resolved_failures(args),
        Commands::ReportSynthesisFailure(args) => report_synthesis_failure(args),
        Commands::UpdateVersionMetadata(args) => update_version_metadata(args),
        Commands::CheckVersionSync(args) => check_version_sync(args),
        Commands::CheckActionContract(args) => check_action_contract(args),
        Commands::ReplayAction(args) => replay_action(args),
        Commands::Backfill(args) => backfill(args),
        Commands::Setup(args) => setup(args),
        Commands::Fleet(args) => fleet(args),
        Commands::PrepareSelfRelease(args) => prepare_self_release(args),
        Commands::PublishSelfRelease(args) => publish_self_release(args),
    }
}
