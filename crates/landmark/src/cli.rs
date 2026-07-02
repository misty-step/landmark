use crate::*;

#[derive(Parser)]
#[command(name = "landmark", version)]
#[command(about = "Rust runtime for the Landmark release action")]
pub(crate) struct Cli {
    #[arg(long = "error-format", global = true, default_value = "text")]
    pub(crate) error_format: String,
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    Describe(DescribeArgs),
    Init(InitArgs),
    Doctor(DoctorArgs),
    ManifestDefaults(ManifestDefaultsArgs),
    Healthcheck(HealthcheckArgs),
    PreflightTags,
    FetchReleaseBody(FetchReleaseBodyArgs),
    ExtractPrs(ExtractPrsArgs),
    Synthesize(Box<SynthesizeArgs>),
    ReleasePolicy(ReleasePolicyArgs),
    UpdateRelease(UpdateReleaseArgs),
    WriteArtifacts(WriteArtifactsArgs),
    UpdateFeed(UpdateFeedArgs),
    NotifyWebhook(NotifyWebhookArgs),
    NotifySlack(NotifySlackArgs),
    Run(RunArgs),
    FloatingTag(FloatingTagArgs),
    CloseResolvedFailures(FailureLifecycleArgs),
    ReportSynthesisFailure(ReportFailureArgs),
    UpdateVersionMetadata(UpdateVersionArgs),
    CheckVersionSync(CheckVersionArgs),
    CheckActionContract(CheckActionContractArgs),
    ReplayAction(ReplayArgs),
    Backfill(BackfillArgs),
    Setup(SetupArgs),
    Fleet(FleetArgs),
    PrepareSelfRelease(PrepareSelfReleaseArgs),
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
}

#[derive(Args)]
pub(crate) struct SynthesizeArgs {
    #[arg(long = "api-key")]
    pub(crate) api_key: String,
    #[arg(long, default_value = "")]
    pub(crate) model: String,
    #[arg(long = "model-policy", default_value = "")]
    pub(crate) model_policy: String,
    #[arg(long = "api-url")]
    pub(crate) api_url: String,
    #[arg(long = "fallback-models", default_value = "")]
    pub(crate) fallback_models: String,
    #[arg(long = "product-name", default_value = "")]
    pub(crate) product_name: String,
    #[arg(long = "product-description", default_value = "")]
    pub(crate) product_description: String,
    #[arg(long = "voice-guide", default_value = "")]
    pub(crate) voice_guide: String,
    #[arg(long)]
    pub(crate) audience: Option<String>,
    #[arg(long = "changelog-source")]
    pub(crate) changelog_source: Option<String>,
    #[arg(long)]
    pub(crate) version: String,
    #[arg(long = "changelog-file")]
    pub(crate) changelog_file: PathBuf,
    #[arg(long = "release-body-file", default_value = ".")]
    pub(crate) release_body_file: PathBuf,
    #[arg(long = "pr-changelog-file", default_value = ".")]
    pub(crate) pr_changelog_file: PathBuf,
    #[arg(long = "prompt-template", default_value = ".")]
    pub(crate) prompt_template: PathBuf,
    #[arg(long = "quality-file")]
    pub(crate) quality_file: PathBuf,
    #[arg(long = "attempts-file", default_value = ".")]
    pub(crate) attempts_file: PathBuf,
    #[arg(long = "templates-dir", default_value = "templates/prompts")]
    pub(crate) templates_dir: PathBuf,
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
    #[arg(long = "dry-run-cost")]
    pub(crate) dry_run_cost: bool,
    #[arg(long = "context-metadata-file", default_value = ".")]
    pub(crate) context_metadata_file: PathBuf,
}

#[derive(Args)]
pub(crate) struct ReleasePolicyArgs {
    #[command(subcommand)]
    pub(crate) command: ReleasePolicyCommand,
}

#[derive(Subcommand)]
pub(crate) enum ReleasePolicyCommand {
    Publication(PublicationArgs),
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
    #[arg(long = "provider", default_value = "local")]
    pub(crate) provider: String,
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
    #[arg(long = "repository", default_value = "")]
    pub(crate) repository: String,
    #[arg(long = "release-tag", default_value = "")]
    pub(crate) release_tag: String,
    #[arg(long = "previous-tag", default_value = "")]
    pub(crate) previous_tag: String,
    #[arg(long = "github-token", default_value = "")]
    pub(crate) github_token: String,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    pub(crate) api_base_url: String,
    #[arg(long = "server-url", default_value = "")]
    pub(crate) server_url: String,
    #[arg(long = "publish-release-body")]
    pub(crate) publish_release_body: bool,
    #[arg(long = "dry-run")]
    pub(crate) dry_run: bool,
    #[arg(long = "notes-file", default_value = "")]
    pub(crate) notes_file: String,
    #[arg(long = "output-dir", default_value = ".landmark/run")]
    pub(crate) output_dir: PathBuf,
    #[arg(
        long = "technical-changelog-file",
        default_value = ".landmark/run/technical-changelog.md"
    )]
    pub(crate) technical_changelog_file: String,
    #[arg(long = "evidence-file", default_value = ".landmark/run/evidence.json")]
    pub(crate) evidence_file: String,
    #[arg(long = "output-file", default_value = "docs/releases/{version}.md")]
    pub(crate) output_file: String,
    #[arg(
        long = "output-text-file",
        default_value = "docs/releases/{version}.txt"
    )]
    pub(crate) output_text_file: String,
    #[arg(
        long = "output-html-file",
        default_value = "docs/releases/{version}.html"
    )]
    pub(crate) output_html_file: String,
    #[arg(long = "output-json", default_value = "docs/releases/releases.json")]
    pub(crate) output_json: String,
    #[arg(long = "rss-feed-file", default_value = "docs/releases/feed.xml")]
    pub(crate) rss_feed_file: String,
    #[arg(long = "rss-max-entries", default_value_t = 50)]
    pub(crate) rss_max_entries: usize,
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
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
    #[arg(long, default_value = "")]
    pub(crate) since: String,
    #[arg(long, default_value = "artifacts-only")]
    pub(crate) mode: String,
    #[arg(long = "dry-run")]
    pub(crate) dry_run: bool,
    #[arg(long = "repository", default_value = "")]
    pub(crate) repository: String,
    #[arg(
        long = "github-token",
        default_value = "",
        help = "GitHub token; defaults to GITHUB_TOKEN when omitted"
    )]
    pub(crate) github_token: String,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    pub(crate) api_base_url: String,
    #[arg(long = "confirm-release-body")]
    pub(crate) confirm_release_body: bool,
    #[arg(long = "max-tags", default_value_t = 0)]
    pub(crate) max_tags: usize,
    #[arg(long = "output-file", default_value = "docs/releases/{version}.md")]
    pub(crate) output_file: String,
    #[arg(
        long = "output-text-file",
        default_value = "docs/releases/{version}.txt"
    )]
    pub(crate) output_text_file: String,
    #[arg(
        long = "output-html-file",
        default_value = "docs/releases/{version}.html"
    )]
    pub(crate) output_html_file: String,
    #[arg(long = "output-json", default_value = "docs/releases/releases.json")]
    pub(crate) output_json: String,
    #[arg(long = "rss-feed-file", default_value = "docs/releases/feed.xml")]
    pub(crate) rss_feed_file: String,
    #[arg(long = "rss-max-entries", default_value_t = 50)]
    pub(crate) rss_max_entries: usize,
    #[arg(
        long = "resume-file",
        default_value = ".landmark/backfill-manifest.json"
    )]
    pub(crate) resume_file: PathBuf,
}

#[derive(Args)]
pub(crate) struct SetupArgs {
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
    #[arg(long = "output-dir", default_value = "")]
    pub(crate) output_dir: String,
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
    Scan(FleetScanArgs),
    Plan(FleetPlanArgs),
    OpenPrs(FleetOpenPrsArgs),
}

#[derive(Args)]
#[command(
    after_help = "Token note: if --github-token is omitted, Landmark reads GITHUB_TOKEN from the environment. Prefer the environment to avoid token-bearing argv."
)]
pub(crate) struct FleetScanArgs {
    #[arg(long)]
    pub(crate) owner: Vec<String>,
    #[arg(long, default_value = ".landmark/fleet.json")]
    pub(crate) output: PathBuf,
    #[arg(long = "max-repos", default_value_t = 0)]
    pub(crate) max_repos: usize,
    #[arg(long = "active-only")]
    pub(crate) active_only: bool,
    #[arg(long = "concurrency", default_value_t = 4)]
    pub(crate) concurrency: usize,
    #[arg(long = "deep-checks")]
    pub(crate) deep_checks: bool,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    pub(crate) api_base_url: String,
    #[arg(long = "github-token", default_value = "")]
    pub(crate) github_token: String,
    #[arg(long = "fixture", hide = true, default_value = "")]
    pub(crate) fixture: String,
    #[arg(long = "format", default_value = "text")]
    pub(crate) format: String,
}

#[derive(Args)]
pub(crate) struct FleetPlanArgs {
    #[arg(long, default_value = ".landmark/fleet.json")]
    pub(crate) input: PathBuf,
    #[arg(long = "output-dir", default_value = ".landmark/fleet-plan")]
    pub(crate) output_dir: PathBuf,
    #[arg(long = "format", default_value = "text")]
    pub(crate) format: String,
}

#[derive(Args)]
pub(crate) struct FleetOpenPrsArgs {
    #[arg(long = "plan-dir", default_value = ".landmark/fleet-plan")]
    pub(crate) plan_dir: PathBuf,
    #[arg(long = "output-dir", default_value = ".landmark/fleet-plan/prs")]
    pub(crate) output_dir: PathBuf,
    #[arg(long = "dry-run")]
    pub(crate) dry_run: bool,
    #[arg(long = "confirm-remote")]
    pub(crate) confirm_remote: bool,
    #[arg(long = "max-prs", default_value_t = 0)]
    pub(crate) max_prs: usize,
    #[arg(long = "format", default_value = "text")]
    pub(crate) format: String,
}

#[derive(Args)]
pub(crate) struct PrepareSelfReleaseArgs {
    #[arg(long = "repo-root", default_value = ".")]
    pub(crate) repo_root: PathBuf,
    #[arg(long, default_value = "misty-step/landmark")]
    pub(crate) repository: String,
    #[arg(long = "release-branch", default_value = "landmark/self-release")]
    pub(crate) release_branch: String,
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
    pub(crate) release_tag: String,
    pub(crate) release_branch: String,
    pub(crate) pull_request_title: String,
    pub(crate) commit_message: String,
    pub(crate) changed_files: Vec<String>,
    pub(crate) changelog: String,
    pub(crate) commits: Vec<SelfReleaseCommit>,
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
        Commands::NotifySlack(args) => notify_slack(args),
        Commands::Run(args) => run_pipeline(args),
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
