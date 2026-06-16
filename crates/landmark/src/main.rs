use chrono::Utc;
use clap::{Args, CommandFactory, Parser, Subcommand};
use hmac::{Hmac, Mac};
use pulldown_cmark::{Options, Parser as MarkdownParser, html};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tiny_http::{Header, Method, Response, Server};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const VALID_NOTES: &str = "## Improvements\n\n- Added a replay harness that checks release behavior in a disposable repo.\n- Captured release body updates, artifacts, tags, and structured logs.\n- Kept the run local so no production secrets or GitHub releases are touched.\n";
const INVALID_NOTES: &str = "hello, here are the release notes";
const LINUX_ACTION_TARGET: &str = "x86_64-unknown-linux-musl";

#[derive(Parser)]
#[command(name = "landmark", version)]
#[command(about = "Rust runtime for the Landmark release action")]
struct Cli {
    #[arg(long = "error-format", global = true, default_value = "text")]
    error_format: String,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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
struct DescribeArgs {
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct InitArgs {
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
    #[arg(long, default_value = ".landmark.yml")]
    output: PathBuf,
    #[arg(long = "dry-run")]
    dry_run: bool,
}

#[derive(Args)]
struct DoctorArgs {
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
    #[arg(long = "format", default_value = "text")]
    format: String,
}

#[derive(Args)]
struct ManifestDefaultsArgs {
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
    #[arg(long = "github-output", default_value = "")]
    github_output: String,
}

#[derive(Args)]
struct HealthcheckArgs {
    #[arg(long = "api-key")]
    api_key: String,
    #[arg(long)]
    model: String,
    #[arg(long = "api-url")]
    api_url: String,
    #[arg(long)]
    warn_only: bool,
}

#[derive(Args)]
struct FetchReleaseBodyArgs {
    #[arg(long = "github-token")]
    github_token: String,
    #[arg(long)]
    repository: String,
    #[arg(long = "release-tag")]
    release_tag: String,
    #[arg(long = "output-file")]
    output_file: PathBuf,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    api_base_url: String,
}

#[derive(Args)]
struct ExtractPrsArgs {
    #[arg(long = "github-token")]
    github_token: String,
    #[arg(long)]
    repository: String,
    #[arg(long = "release-tag")]
    release_tag: String,
    #[arg(long = "output-file")]
    output_file: PathBuf,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    api_base_url: String,
}

#[derive(Args)]
struct SynthesizeArgs {
    #[arg(long = "api-key")]
    api_key: String,
    #[arg(long, default_value = "")]
    model: String,
    #[arg(long = "model-policy", default_value = "")]
    model_policy: String,
    #[arg(long = "api-url")]
    api_url: String,
    #[arg(long = "fallback-models", default_value = "")]
    fallback_models: String,
    #[arg(long = "product-name", default_value = "")]
    product_name: String,
    #[arg(long = "product-description", default_value = "")]
    product_description: String,
    #[arg(long = "voice-guide", default_value = "")]
    voice_guide: String,
    #[arg(long)]
    audience: Option<String>,
    #[arg(long = "changelog-source")]
    changelog_source: Option<String>,
    #[arg(long)]
    version: String,
    #[arg(long = "changelog-file")]
    changelog_file: PathBuf,
    #[arg(long = "release-body-file", default_value = ".")]
    release_body_file: PathBuf,
    #[arg(long = "pr-changelog-file", default_value = ".")]
    pr_changelog_file: PathBuf,
    #[arg(long = "prompt-template", default_value = ".")]
    prompt_template: PathBuf,
    #[arg(long = "quality-file")]
    quality_file: PathBuf,
    #[arg(long = "attempts-file", default_value = ".")]
    attempts_file: PathBuf,
    #[arg(long = "templates-dir", default_value = "templates/prompts")]
    templates_dir: PathBuf,
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
    #[arg(long = "dry-run-cost")]
    dry_run_cost: bool,
    #[arg(long = "context-metadata-file", default_value = ".")]
    context_metadata_file: PathBuf,
}

#[derive(Args)]
struct ReleasePolicyArgs {
    #[command(subcommand)]
    command: ReleasePolicyCommand,
}

#[derive(Subcommand)]
enum ReleasePolicyCommand {
    Publication(PublicationArgs),
    Summary(Box<SummaryArgs>),
}

#[derive(Args)]
struct PublicationArgs {
    #[arg(long = "synthesis-required")]
    synthesis_required: String,
    #[arg(long = "synthesis-strict")]
    synthesis_strict: String,
    #[arg(long = "synth-succeeded")]
    synth_succeeded: String,
    #[arg(long = "synth-quality", default_value = "")]
    synth_quality: String,
    #[arg(long = "synth-failure-stage", default_value = "")]
    synth_failure_stage: String,
    #[arg(long = "synth-failure-message", default_value = "")]
    synth_failure_message: String,
    #[arg(long = "github-output")]
    github_output: PathBuf,
}

#[derive(Args)]
struct SummaryArgs {
    #[arg(long = "synthesis-enabled")]
    synthesis_enabled: String,
    #[arg(long)]
    released: String,
    #[arg(long = "synth-succeeded")]
    synth_succeeded: String,
    #[arg(long = "synth-quality", default_value = "")]
    synth_quality: String,
    #[arg(long = "update-succeeded")]
    update_succeeded: String,
    #[arg(long = "synth-failure-stage", default_value = "")]
    synth_failure_stage: String,
    #[arg(long = "synth-failure-message", default_value = "")]
    synth_failure_message: String,
    #[arg(long = "update-failure-stage", default_value = "")]
    update_failure_stage: String,
    #[arg(long = "update-failure-message", default_value = "")]
    update_failure_message: String,
    #[arg(long = "artifact-succeeded", default_value = "")]
    artifact_succeeded: String,
    #[arg(long = "artifact-failure-stage", default_value = "")]
    artifact_failure_stage: String,
    #[arg(long = "artifact-failure-message", default_value = "")]
    artifact_failure_message: String,
    #[arg(long = "rss-enabled", default_value = "")]
    rss_enabled: String,
    #[arg(long = "rss-succeeded", default_value = "")]
    rss_succeeded: String,
    #[arg(long = "rss-failure-stage", default_value = "")]
    rss_failure_stage: String,
    #[arg(long = "rss-failure-message", default_value = "")]
    rss_failure_message: String,
    #[arg(long = "webhook-enabled", default_value = "")]
    webhook_enabled: String,
    #[arg(long = "webhook-sent", default_value = "")]
    webhook_sent: String,
    #[arg(long = "slack-enabled", default_value = "")]
    slack_enabled: String,
    #[arg(long = "slack-sent", default_value = "")]
    slack_sent: String,
    #[arg(long = "github-output")]
    github_output: PathBuf,
    #[arg(long = "attempts-file", default_value = ".")]
    attempts_file: String,
    #[arg(long = "context-metadata-file", default_value = ".")]
    context_metadata_file: String,
}

#[derive(Args)]
struct UpdateReleaseArgs {
    #[arg(long = "github-token")]
    github_token: String,
    #[arg(long)]
    repository: String,
    #[arg(long)]
    tag: String,
    #[arg(long = "notes-file")]
    notes_file: PathBuf,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    api_base_url: String,
}

#[derive(Args)]
struct WriteArtifactsArgs {
    #[arg(long = "notes-file")]
    notes_file: PathBuf,
    #[arg(long)]
    version: String,
    #[arg(long = "output-file", default_value = "")]
    output_file: String,
    #[arg(long = "output-text-file", default_value = "")]
    output_text_file: String,
    #[arg(long = "output-html-file", default_value = "")]
    output_html_file: String,
    #[arg(long = "output-json", default_value = "")]
    output_json: String,
}

#[derive(Args)]
struct UpdateFeedArgs {
    #[arg(long = "feed-file")]
    feed_file: String,
    #[arg(long = "max-entries")]
    max_entries: usize,
    #[arg(long)]
    repository: String,
    #[arg(long = "release-tag")]
    release_tag: String,
    #[arg(long = "release-url")]
    release_url: String,
    #[arg(long = "notes-file")]
    notes_file: PathBuf,
    #[arg(long)]
    workspace: PathBuf,
}

#[derive(Args)]
struct NotifyWebhookArgs {
    #[arg(long = "webhook-url")]
    webhook_url: String,
    #[arg(long = "webhook-secret", default_value = "")]
    webhook_secret: String,
    #[arg(long)]
    version: String,
    #[arg(long)]
    repository: String,
    #[arg(long = "release-url")]
    release_url: String,
    #[arg(long = "notes-file")]
    notes_file: PathBuf,
}

#[derive(Args)]
struct NotifySlackArgs {
    #[arg(long = "slack-webhook-url")]
    slack_webhook_url: String,
    #[arg(long)]
    version: String,
    #[arg(long)]
    repository: String,
    #[arg(long = "release-url")]
    release_url: String,
    #[arg(long = "notes-file")]
    notes_file: PathBuf,
}

#[derive(Args)]
struct RunArgs {
    #[arg(long = "provider", default_value = "local")]
    provider: String,
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
    #[arg(long = "repository", default_value = "")]
    repository: String,
    #[arg(long = "release-tag", default_value = "")]
    release_tag: String,
    #[arg(long = "previous-tag", default_value = "")]
    previous_tag: String,
    #[arg(long = "github-token", default_value = "")]
    github_token: String,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    api_base_url: String,
    #[arg(long = "server-url", default_value = "")]
    server_url: String,
    #[arg(long = "publish-release-body")]
    publish_release_body: bool,
    #[arg(long = "dry-run")]
    dry_run: bool,
    #[arg(long = "notes-file", default_value = "")]
    notes_file: String,
    #[arg(long = "output-dir", default_value = ".landmark/run")]
    output_dir: PathBuf,
    #[arg(
        long = "technical-changelog-file",
        default_value = ".landmark/run/technical-changelog.md"
    )]
    technical_changelog_file: String,
    #[arg(long = "evidence-file", default_value = ".landmark/run/evidence.json")]
    evidence_file: String,
    #[arg(long = "output-file", default_value = "docs/releases/{version}.md")]
    output_file: String,
    #[arg(
        long = "output-text-file",
        default_value = "docs/releases/{version}.txt"
    )]
    output_text_file: String,
    #[arg(
        long = "output-html-file",
        default_value = "docs/releases/{version}.html"
    )]
    output_html_file: String,
    #[arg(long = "output-json", default_value = "docs/releases/releases.json")]
    output_json: String,
    #[arg(long = "rss-feed-file", default_value = "docs/releases/feed.xml")]
    rss_feed_file: String,
    #[arg(long = "rss-max-entries", default_value_t = 50)]
    rss_max_entries: usize,
}

#[derive(Args)]
struct FloatingTagArgs {
    #[arg(long = "release-tag")]
    release_tag: String,
}

#[derive(Args)]
struct FailureLifecycleArgs {
    #[arg(long = "github-token")]
    github_token: String,
    #[arg(long)]
    repository: String,
    #[arg(long = "release-tag")]
    release_tag: String,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    api_base_url: String,
}

#[derive(Args)]
struct ReportFailureArgs {
    #[arg(long = "github-token")]
    github_token: String,
    #[arg(long)]
    repository: String,
    #[arg(long = "release-tag")]
    release_tag: String,
    #[arg(long = "workflow-run-url")]
    workflow_run_url: String,
    #[arg(long = "workflow-name")]
    workflow_name: String,
    #[arg(long = "failure-stage")]
    failure_stage: String,
    #[arg(long = "failure-message")]
    failure_message: String,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    api_base_url: String,
}

#[derive(Args)]
struct UpdateVersionArgs {
    #[arg(long)]
    version: String,
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
}

#[derive(Args)]
struct CheckVersionArgs {
    #[arg(long, default_value = "HEAD")]
    reference: String,
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
    #[arg(long = "allow-release-candidate")]
    allow_release_candidate: bool,
}

#[derive(Args)]
struct CheckActionContractArgs {
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
}

#[derive(Args)]
struct ReplayArgs {
    #[arg(long = "evidence-dir", default_value = "")]
    evidence_dir: String,
    #[arg(long = "scenario")]
    scenario: Vec<String>,
    #[arg(long = "format", default_value = "text")]
    format: String,
}

#[derive(Args)]
struct BackfillArgs {
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
    #[arg(long, default_value = "")]
    since: String,
    #[arg(long, default_value = "artifacts-only")]
    mode: String,
    #[arg(long = "dry-run")]
    dry_run: bool,
    #[arg(long = "repository", default_value = "")]
    repository: String,
    #[arg(
        long = "github-token",
        default_value = "",
        help = "GitHub token; defaults to GITHUB_TOKEN when omitted"
    )]
    github_token: String,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    api_base_url: String,
    #[arg(long = "confirm-release-body")]
    confirm_release_body: bool,
    #[arg(long = "max-tags", default_value_t = 0)]
    max_tags: usize,
    #[arg(long = "output-file", default_value = "docs/releases/{version}.md")]
    output_file: String,
    #[arg(
        long = "output-text-file",
        default_value = "docs/releases/{version}.txt"
    )]
    output_text_file: String,
    #[arg(
        long = "output-html-file",
        default_value = "docs/releases/{version}.html"
    )]
    output_html_file: String,
    #[arg(long = "output-json", default_value = "docs/releases/releases.json")]
    output_json: String,
    #[arg(long = "rss-feed-file", default_value = "docs/releases/feed.xml")]
    rss_feed_file: String,
    #[arg(long = "rss-max-entries", default_value_t = 50)]
    rss_max_entries: usize,
    #[arg(
        long = "resume-file",
        default_value = ".landmark/backfill-manifest.json"
    )]
    resume_file: PathBuf,
}

#[derive(Args)]
struct SetupArgs {
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
    #[arg(long = "output-dir", default_value = "")]
    output_dir: String,
    #[arg(long = "dry-run")]
    dry_run: bool,
}

#[derive(Args)]
struct FleetArgs {
    #[command(subcommand)]
    command: FleetCommand,
}

#[derive(Subcommand)]
enum FleetCommand {
    Scan(FleetScanArgs),
    Plan(FleetPlanArgs),
    OpenPrs(FleetOpenPrsArgs),
}

#[derive(Args)]
#[command(
    after_help = "Token note: if --github-token is omitted, Landmark reads GITHUB_TOKEN from the environment. Prefer the environment to avoid token-bearing argv."
)]
struct FleetScanArgs {
    #[arg(long)]
    owner: Vec<String>,
    #[arg(long, default_value = ".landmark/fleet.json")]
    output: PathBuf,
    #[arg(long = "max-repos", default_value_t = 0)]
    max_repos: usize,
    #[arg(long = "active-only")]
    active_only: bool,
    #[arg(long = "concurrency", default_value_t = 4)]
    concurrency: usize,
    #[arg(long = "deep-checks")]
    deep_checks: bool,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    api_base_url: String,
    #[arg(long = "github-token", default_value = "")]
    github_token: String,
    #[arg(long = "fixture", hide = true, default_value = "")]
    fixture: String,
    #[arg(long = "format", default_value = "text")]
    format: String,
}

#[derive(Args)]
struct FleetPlanArgs {
    #[arg(long, default_value = ".landmark/fleet.json")]
    input: PathBuf,
    #[arg(long = "output-dir", default_value = ".landmark/fleet-plan")]
    output_dir: PathBuf,
    #[arg(long = "format", default_value = "text")]
    format: String,
}

#[derive(Args)]
struct FleetOpenPrsArgs {
    #[arg(long = "plan-dir", default_value = ".landmark/fleet-plan")]
    plan_dir: PathBuf,
    #[arg(long = "output-dir", default_value = ".landmark/fleet-plan/prs")]
    output_dir: PathBuf,
    #[arg(long = "dry-run")]
    dry_run: bool,
    #[arg(long = "confirm-remote")]
    confirm_remote: bool,
    #[arg(long = "max-prs", default_value_t = 0)]
    max_prs: usize,
    #[arg(long = "format", default_value = "text")]
    format: String,
}

#[derive(Args)]
struct PrepareSelfReleaseArgs {
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
    #[arg(long, default_value = "misty-step/landmark")]
    repository: String,
    #[arg(long = "release-branch", default_value = "landmark/self-release")]
    release_branch: String,
    #[arg(long = "dist-target", default_value = LINUX_ACTION_TARGET)]
    dist_target: String,
    #[arg(long = "github-output", default_value = "")]
    github_output: String,
}

#[derive(Args)]
struct PublishSelfReleaseArgs {
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
    #[arg(long = "github-token")]
    github_token: String,
    #[arg(long)]
    repository: String,
    #[arg(long = "target-sha")]
    target_sha: String,
    #[arg(long = "github-output", default_value = "")]
    github_output: String,
    #[arg(long = "api-base-url", default_value = "https://api.github.com")]
    api_base_url: String,
}

#[derive(Serialize)]
struct SelfReleasePlan {
    released: bool,
    reason: String,
    latest_version: String,
    next_version: String,
    release_tag: String,
    release_branch: String,
    pull_request_title: String,
    commit_message: String,
    changed_files: Vec<String>,
    changelog: String,
    commits: Vec<SelfReleaseCommit>,
}

#[derive(Clone, Serialize)]
struct SelfReleaseCommit {
    hash: String,
    short_hash: String,
    subject: String,
    category: String,
    scope: String,
    description: String,
    breaking: bool,
}

#[derive(Serialize)]
struct SelfReleasePublish {
    published: bool,
    reason: String,
    latest_version: String,
    version: String,
    release_tag: String,
    release_url: String,
}

fn main() {
    let cli = Cli::parse();
    let error_format = cli.error_format.clone();
    if let Err(error) = run(cli) {
        if error_format == "json" {
            eprintln!("{}", structured_error_json(&error.to_string()));
        } else {
            eprintln!("{error}");
        }
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
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

fn structured_error_json(message: &str) -> String {
    let failure = classify_failure(message);
    serde_json::to_string_pretty(&json!({
        "error": {
            "code": failure.code,
            "stage": failure.stage,
            "retryable": failure.retryable,
            "user_action": failure.user_action,
            "context": {
                "message": redact_context(message)
            }
        }
    }))
    .unwrap_or_else(|_| "{\"error\":{\"code\":\"internal_error\",\"stage\":\"internal\",\"retryable\":false,\"user_action\":\"inspect stderr\",\"context\":{}}}".into())
}

struct FailureClass {
    code: &'static str,
    stage: &'static str,
    retryable: bool,
    user_action: &'static str,
}

fn classify_failure(message: &str) -> FailureClass {
    let lower = message.to_ascii_lowercase();
    if lower.contains("github-token") || lower.contains("gh_token") || lower.contains("auth") {
        FailureClass {
            code: "provider_auth",
            stage: "provider",
            retryable: false,
            user_action: "Provide a valid provider token through the documented secret or environment variable.",
        }
    } else if lower.contains("http 429")
        || lower.contains("rate limit")
        || lower.contains("timeout")
    {
        FailureClass {
            code: "provider_outage",
            stage: "provider",
            retryable: true,
            user_action: "Retry after the provider recovers or reduce request volume.",
        }
    } else if lower.contains("changelog.source") || lower.contains("invalid changelog") {
        FailureClass {
            code: "invalid_changelog_source",
            stage: "configuration",
            retryable: false,
            user_action: "Set changelog.source to auto, changelog, release-body, or prs.",
        }
    } else if lower.contains("budget") || lower.contains("model.policy=off") {
        FailureClass {
            code: "budget_skip",
            stage: "synthesis",
            retryable: false,
            user_action: "Raise the configured budget, change model policy, or accept synthesis skip.",
        }
    } else if lower.contains("degraded") || lower.contains("quality") {
        FailureClass {
            code: "synthesis_degradation",
            stage: "synthesis",
            retryable: false,
            user_action: "Inspect synthesis attempts and either improve source context or relax strict synthesis policy.",
        }
    } else if lower.contains("release body") || lower.contains("publish-release-body") {
        FailureClass {
            code: "publication_mutation_failure",
            stage: "publication",
            retryable: true,
            user_action: "Check release existence, provider permissions, and publication mode.",
        }
    } else if lower.contains("rss") || lower.contains("feed") {
        FailureClass {
            code: "feed_failure",
            stage: "artifact",
            retryable: false,
            user_action: "Check feed path, max entries, and existing feed XML.",
        }
    } else if lower.contains("write") || lower.contains("file") || lower.contains("permission") {
        FailureClass {
            code: "artifact_write_failure",
            stage: "artifact",
            retryable: false,
            user_action: "Check output paths and filesystem permissions.",
        }
    } else if lower.contains("unsupported provider")
        || lower.contains("requires")
        || lower.contains("must")
    {
        FailureClass {
            code: "invalid_input",
            stage: "configuration",
            retryable: false,
            user_action: "Correct the command arguments and retry.",
        }
    } else {
        FailureClass {
            code: "command_failed",
            stage: "runtime",
            retryable: false,
            user_action: "Inspect the command context and Landmark evidence packet.",
        }
    }
}

fn redact_context(value: &str) -> String {
    let token_re = Regex::new(r"(ghp|github_pat|sk|xox[baprs])[-_][-_=A-Za-z0-9]{8,}").unwrap();
    redact_configured_secrets(&token_re.replace_all(value, "[REDACTED]"))
}

fn redact_known_secrets(value: &str) -> String {
    redact_context(value)
}

fn redact_configured_secrets(value: &str) -> String {
    redact_secret_values(
        value,
        configured_secret_env_names()
            .into_iter()
            .filter_map(|name| env::var(name).ok()),
    )
}

fn redact_secret_values<I>(value: &str, secrets: I) -> String
where
    I: IntoIterator<Item = String>,
{
    let mut redacted = value.to_string();
    for secret in secrets {
        if secret.len() >= 8 && redacted.contains(&secret) {
            redacted = redacted.replace(&secret, "[REDACTED]");
        }
    }
    redacted
}

fn configured_secret_env_names() -> [&'static str; 5] {
    [
        "GITHUB_TOKEN",
        "GH_TOKEN",
        "OPENROUTER_API_KEY",
        "WEBHOOK_SECRET",
        "SLACK_WEBHOOK_URL",
    ]
}

#[derive(Clone, Serialize)]
struct SchemaDescriptor {
    name: &'static str,
    path: &'static str,
    id: &'static str,
    version: &'static str,
    artifact: &'static str,
}

#[derive(Clone, Serialize)]
struct CommandContract {
    command: &'static str,
    mode: &'static str,
    mutates: bool,
    preview: &'static str,
    stdout: &'static str,
    stderr: &'static str,
    json_output: bool,
}

fn describe(args: DescribeArgs) -> Result<()> {
    if !args.json {
        println!("Run `landmark describe --json` for the machine-readable Landmark contract.");
        return Ok(());
    }
    let command = Cli::command();
    let commands: Vec<Value> = command
        .get_subcommands()
        .map(describe_clap_command)
        .collect();
    let document = json!({
        "schema_version": "landmark.describe.v1",
        "landmark_version": env!("CARGO_PKG_VERSION"),
        "product_boundary": "Rust CLI runtime; GitHub Action is an adapter wrapper",
        "providers": ["local", "github"],
        "modes": ["full", "synthesis-only", "artifacts-only", "release-body"],
        "stdout_stderr": {
            "json_payloads": "stdout",
            "logs_and_errors": "stderr",
            "json_errors": "pass --error-format json"
        },
        "commands": commands,
        "contracts": command_contracts(),
        "schemas": schema_descriptors(),
        "failure_taxonomy": failure_taxonomy(),
        "examples": [
            {
                "name": "local dry-run release evidence",
                "command": "landmark run --provider local --repo-root . --dry-run"
            },
            {
                "name": "machine-readable failure",
                "command": "landmark --error-format json run --provider unsupported --dry-run"
            },
            {
                "name": "cold-agent replay oracle",
                "command": "landmark replay-action --scenario agent_native_contracts --format json"
            }
        ]
    });
    println!("{}", serde_json::to_string_pretty(&document)?);
    Ok(())
}

fn describe_clap_command(command: &clap::Command) -> Value {
    let inputs: Vec<Value> = command
        .get_arguments()
        .filter(|arg| !arg.is_hide_set())
        .map(|arg| {
            let defaults: Vec<String> = arg
                .get_default_values()
                .iter()
                .map(|value| value.to_string_lossy().to_string())
                .collect();
            json!({
                "id": arg.get_id().to_string(),
                "long": arg.get_long().unwrap_or_default(),
                "required": arg.is_required_set(),
                "default": defaults,
                "help": arg.get_help().map(|help| help.to_string()).unwrap_or_default()
            })
        })
        .collect();
    let subcommands: Vec<Value> = command
        .get_subcommands()
        .map(describe_clap_command)
        .collect();
    json!({
        "name": command.get_name(),
        "about": command.get_about().map(|about| about.to_string()).unwrap_or_default(),
        "inputs": inputs,
        "subcommands": subcommands
    })
}

fn schema_descriptors() -> Vec<SchemaDescriptor> {
    vec![
        SchemaDescriptor {
            name: "landmark_manifest",
            path: "schemas/landmark-manifest.v1.schema.json",
            id: "https://landmark.dev/schemas/landmark-manifest.v1.schema.json",
            version: "v1",
            artifact: ".landmark.yml",
        },
        SchemaDescriptor {
            name: "synthesis_status",
            path: "schemas/synthesis-status.v1.schema.json",
            id: "https://landmark.dev/schemas/synthesis-status.v1.schema.json",
            version: "v1",
            artifact: "synthesis-status output",
        },
        SchemaDescriptor {
            name: "release_context",
            path: "schemas/release-context.v1.schema.json",
            id: "https://landmark.dev/schemas/release-context.v1.schema.json",
            version: "v1",
            artifact: "release context packet",
        },
        SchemaDescriptor {
            name: "replay_result",
            path: "schemas/replay-result.v1.schema.json",
            id: "https://landmark.dev/schemas/replay-result.v1.schema.json",
            version: "v1",
            artifact: "replay-action evidence",
        },
        SchemaDescriptor {
            name: "fleet_plan",
            path: "schemas/fleet-plan.v1.schema.json",
            id: "https://landmark.dev/schemas/fleet-plan.v1.schema.json",
            version: "v1",
            artifact: "fleet plan",
        },
        SchemaDescriptor {
            name: "release_entry",
            path: "schemas/release-entry.v1.schema.json",
            id: "https://landmark.dev/schemas/release-entry.v1.schema.json",
            version: "v1",
            artifact: "release notes JSON entry",
        },
        SchemaDescriptor {
            name: "run_evidence",
            path: "schemas/run-evidence.v1.schema.json",
            id: "https://landmark.dev/schemas/run-evidence.v1.schema.json",
            version: "v1",
            artifact: "landmark run evidence packet",
        },
        SchemaDescriptor {
            name: "failure_envelope",
            path: "schemas/failure-envelope.v1.schema.json",
            id: "https://landmark.dev/schemas/failure-envelope.v1.schema.json",
            version: "v1",
            artifact: "--error-format json stderr",
        },
    ]
}

fn command_contracts() -> Vec<CommandContract> {
    vec![
        CommandContract {
            command: "describe --json",
            mode: "agent-discovery",
            mutates: false,
            preview: "not needed",
            stdout: "Describe document JSON",
            stderr: "logs and errors only",
            json_output: true,
        },
        CommandContract {
            command: "init",
            mode: "configuration-bootstrap",
            mutates: true,
            preview: "--dry-run prints inferred manifest without writing .landmark.yml",
            stdout: "manifest YAML with --dry-run, otherwise no payload",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "doctor",
            mode: "configuration-validation",
            mutates: false,
            preview: "not needed",
            stdout: "text verdict, or JSON with --format json",
            stderr: "logs and errors only; pass --error-format json for failures",
            json_output: true,
        },
        CommandContract {
            command: "manifest-defaults",
            mode: "action-adapter",
            mutates: true,
            preview: "omit --github-output for JSON-only output",
            stdout: "manifest default JSON when --github-output is omitted",
            stderr: "logs and errors only",
            json_output: true,
        },
        CommandContract {
            command: "healthcheck",
            mode: "provider-health",
            mutates: false,
            preview: "not needed",
            stdout: "text health verdict",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "preflight-tags",
            mode: "action-adapter",
            mutates: true,
            preview: "not available; this command mutates only the local git checkout tags",
            stdout: "none",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "fetch-release-body",
            mode: "github-adapter",
            mutates: true,
            preview: "not available; writes the requested local output file",
            stdout: "none",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "extract-prs",
            mode: "github-adapter",
            mutates: true,
            preview: "not available; writes the requested local output file",
            stdout: "none",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "synthesize",
            mode: "synthesis",
            mutates: true,
            preview: "--dry-run-cost emits the context packet without calling an LLM or writing notes",
            stdout: "release notes markdown, or context packet JSON with --dry-run-cost",
            stderr: "logs and errors only",
            json_output: true,
        },
        CommandContract {
            command: "release-policy",
            mode: "action-adapter",
            mutates: true,
            preview: "not available; writes GitHub output key-value fields",
            stdout: "none",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "update-release",
            mode: "github-adapter",
            mutates: true,
            preview: "not available; use run --dry-run for publication preview",
            stdout: "none",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "write-artifacts",
            mode: "artifact-adapter",
            mutates: true,
            preview: "not available; use run --dry-run for artifact path preview",
            stdout: "release notes markdown",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "update-feed",
            mode: "artifact-adapter",
            mutates: true,
            preview: "not available; use run --dry-run for feed path preview",
            stdout: "none",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "notify-webhook",
            mode: "notification-adapter",
            mutates: true,
            preview: "not available; this command sends exactly one webhook request",
            stdout: "none",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "notify-slack",
            mode: "notification-adapter",
            mutates: true,
            preview: "not available; this command sends exactly one Slack webhook request",
            stdout: "none",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "setup",
            mode: "adoption-planning",
            mutates: true,
            preview: "--dry-run or omit --output-dir for JSON-only planning",
            stdout: "SetupReport JSON",
            stderr: "logs and errors only",
            json_output: true,
        },
        CommandContract {
            command: "run",
            mode: "portable-release",
            mutates: true,
            preview: "--dry-run computes evidence without writing artifacts or mutating releases",
            stdout: "RunEvidence JSON",
            stderr: "logs and errors only",
            json_output: true,
        },
        CommandContract {
            command: "backfill",
            mode: "historical-artifacts",
            mutates: true,
            preview: "--dry-run writes no release-body mutations and records planned artifacts",
            stdout: "BackfillManifest JSON",
            stderr: "logs and errors only",
            json_output: true,
        },
        CommandContract {
            command: "fleet scan",
            mode: "fleet-adoption",
            mutates: true,
            preview: "writes only the requested local JSON scan file; use --fixture for offline replay",
            stdout: "text receipt when --output is set, otherwise FleetScan JSON; pass --format json for JSON stdout",
            stderr: "logs and errors only",
            json_output: true,
        },
        CommandContract {
            command: "fleet plan",
            mode: "fleet-adoption",
            mutates: true,
            preview: "local files only under --output-dir",
            stdout: "text receipt plus plan.json artifact, or FleetPlan JSON with --format json",
            stderr: "logs and errors only",
            json_output: true,
        },
        CommandContract {
            command: "fleet open-prs",
            mode: "fleet-adoption",
            mutates: true,
            preview: "requires --dry-run; remote PR mutation is intentionally unavailable",
            stdout: "text receipt plus open-prs.json artifact, or FleetPrPlan JSON with --format json",
            stderr: "logs and errors only",
            json_output: true,
        },
        CommandContract {
            command: "floating-tag",
            mode: "action-adapter",
            mutates: false,
            preview: "not needed",
            stdout: "major floating tag text when release tag is stable",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "close-resolved-failures",
            mode: "github-adapter",
            mutates: true,
            preview: "not available; closes matching Landmark failure issues when resolved",
            stdout: "none",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "report-synthesis-failure",
            mode: "github-adapter",
            mutates: true,
            preview: "not available; creates or comments on a GitHub issue",
            stdout: "none",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "update-version-metadata",
            mode: "self-release",
            mutates: true,
            preview: "not available; writes package metadata for self-release",
            stdout: "none",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "check-version-sync",
            mode: "verification",
            mutates: false,
            preview: "not needed",
            stdout: "text version verdict",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "check-action-contract",
            mode: "verification",
            mutates: false,
            preview: "not needed",
            stdout: "text contract verdict",
            stderr: "logs and errors only",
            json_output: false,
        },
        CommandContract {
            command: "replay-action",
            mode: "verification",
            mutates: true,
            preview: "writes disposable fixture repos and evidence only",
            stdout: "text receipt, or ReplayResult JSON with --format json",
            stderr: "logs and errors only",
            json_output: true,
        },
        CommandContract {
            command: "prepare-self-release",
            mode: "self-release",
            mutates: true,
            preview: "not available; prepares the self-release branch and prints a release plan",
            stdout: "SelfReleasePlan JSON",
            stderr: "logs and errors only",
            json_output: true,
        },
        CommandContract {
            command: "publish-self-release",
            mode: "self-release",
            mutates: true,
            preview: "not available; publishes release state after protected-branch merge",
            stdout: "SelfReleasePublish JSON",
            stderr: "logs and errors only",
            json_output: true,
        },
    ]
}

fn failure_taxonomy() -> Vec<Value> {
    [
        (
            "provider_auth",
            "provider",
            false,
            "Provider credentials are missing, invalid, or insufficient.",
        ),
        (
            "provider_outage",
            "provider",
            true,
            "Provider is rate-limited, unavailable, or timed out.",
        ),
        (
            "invalid_changelog_source",
            "configuration",
            false,
            "Manifest or input selects an unsupported changelog source.",
        ),
        (
            "budget_skip",
            "synthesis",
            false,
            "Configured budget or model policy intentionally skipped synthesis.",
        ),
        (
            "synthesis_degradation",
            "synthesis",
            false,
            "LLM output failed quality policy or degraded below the required threshold.",
        ),
        (
            "artifact_write_failure",
            "artifact",
            false,
            "Local release artifact write failed.",
        ),
        (
            "feed_failure",
            "artifact",
            false,
            "RSS feed read, merge, or write failed.",
        ),
        (
            "publication_mutation_failure",
            "publication",
            true,
            "Remote release body mutation failed.",
        ),
    ]
    .into_iter()
    .map(|(code, stage, retryable, meaning)| {
        json!({
            "code": code,
            "stage": stage,
            "retryable": retryable,
            "meaning": meaning
        })
    })
    .collect()
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "on"
    )
}

fn sanitize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn write_outputs(path: &Path, fields: &[(&str, String)]) -> Result<()> {
    ensure_parent(path)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    for (key, value) in fields {
        writeln!(file, "{key}={value}")?;
    }
    Ok(())
}

fn run_cmd<I, S>(program: &str, args: I, cwd: &Path) -> Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(program).args(args).current_dir(cwd).output()?;
    Ok(output)
}

fn run_ok<I, S>(program: &str, args: I, cwd: &Path) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_cmd(program, args, cwd)?;
    if !output.status.success() {
        return Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct LandmarkManifest {
    product: ProductManifest,
    audience: Option<String>,
    voice: Option<String>,
    changelog: ChangelogManifest,
    artifacts: ArtifactManifest,
    release: ReleaseManifest,
    model: ModelManifest,
    budget: BudgetManifest,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct ProductManifest {
    name: Option<String>,
    description: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct ChangelogManifest {
    source: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct ArtifactManifest {
    markdown: Option<String>,
    plaintext: Option<String>,
    html: Option<String>,
    json: Option<String>,
    rss: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct ReleaseManifest {
    profile: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct ModelManifest {
    policy: Option<String>,
    primary: Option<String>,
    fallbacks: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct BudgetManifest {
    max_input_tokens: Option<u64>,
    max_output_tokens: Option<u64>,
    max_usd: Option<f64>,
}

#[derive(Clone, Debug)]
struct EffectiveSynthesisConfig {
    product_name: String,
    product_description: String,
    voice_guide: String,
    audience: String,
    changelog_source: String,
    model_policy: String,
    model: String,
    fallback_models: String,
    max_input_tokens: Option<u64>,
    max_output_tokens: Option<u64>,
    max_usd: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
struct SynthesisContextPacket {
    product: ContextProduct,
    release: ContextRelease,
    deterministic: DeterministicReleaseContext,
    sources: Vec<ContextSource>,
    classification: ReleaseClassification,
    cost: CostEstimate,
    decision: SynthesisDecision,
}

#[derive(Clone, Debug, Serialize)]
struct ContextProduct {
    name: String,
    audience: String,
    description: String,
}

#[derive(Clone, Debug, Serialize)]
struct ContextRelease {
    version: String,
    changelog_source: String,
    model_policy: String,
}

#[derive(Clone, Debug, Serialize)]
struct ContextSource {
    name: String,
    kind: String,
    bytes: usize,
    estimated_tokens: u64,
    included: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
struct DeterministicReleaseContext {
    commits: Vec<ContextCommit>,
    tags: Vec<String>,
    changed_files: Vec<String>,
    manifest: ContextManifestSummary,
    docs: Vec<ContextDocument>,
    package: Option<ContextPackage>,
    prior_releases: Vec<String>,
    pr_metadata: ContextOptionalSource,
    release_body: ContextOptionalSource,
    artifacts: ContextArtifactAudiences,
}

#[derive(Clone, Debug, Serialize)]
struct ContextCommit {
    subject: String,
    short_hash: String,
    conventional_type: String,
    breaking: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
struct ContextManifestSummary {
    present: bool,
    product_name: String,
    audience: String,
    model_policy: String,
}

#[derive(Clone, Debug, Serialize)]
struct ContextDocument {
    path: String,
    title: String,
    estimated_tokens: u64,
}

#[derive(Clone, Debug, Serialize)]
struct ContextPackage {
    manager: String,
    name: String,
    description: String,
}

#[derive(Clone, Debug, Default, Serialize)]
struct ContextOptionalSource {
    present: bool,
    estimated_tokens: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
struct ContextArtifactAudiences {
    internal_technical_changelog: String,
    public_release_notes: String,
}

#[derive(Clone, Debug, Serialize)]
struct ReleaseClassification {
    categories: Vec<String>,
    significance: String,
    user_visible: bool,
    breaking: bool,
    security: bool,
    migration_heavy: bool,
    reasons: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct CostEstimate {
    input_tokens: u64,
    output_tokens: u64,
    model_tier: String,
    model: String,
    estimated_usd: f64,
    skip: bool,
    skip_reason: String,
}

#[derive(Clone, Debug, Serialize)]
struct SynthesisDecision {
    action: String,
    reason: String,
    llm_required: bool,
    model_tier: String,
}

fn init(args: InitArgs) -> Result<()> {
    let manifest = infer_manifest(&args.repo_root);
    let rendered = render_manifest_yaml(&manifest)?;
    if args.dry_run {
        print!("{rendered}");
        return Ok(());
    }
    let output = args.repo_root.join(args.output);
    ensure_parent(&output)?;
    fs::write(output, rendered)?;
    Ok(())
}

fn doctor(args: DoctorArgs) -> Result<()> {
    if !matches!(args.format.as_str(), "text" | "json") {
        return Err("--format must be text or json".into());
    }
    let manifest = load_manifest(&args.repo_root)?.ok_or(".landmark.yml is missing")?;
    let mut errors = validate_manifest(&manifest);
    errors.extend(validate_manifest_completeness(&manifest));
    if errors.is_empty() {
        if args.format == "json" {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "verdict": "passed",
                    "schema": "schemas/landmark-manifest.v1.schema.json",
                    "manifest": ".landmark.yml"
                }))?
            );
        } else {
            println!("manifest ok (schema schemas/landmark-manifest.v1.schema.json)");
        }
        Ok(())
    } else {
        Err(errors.join("\n").into())
    }
}

fn manifest_defaults(args: ManifestDefaultsArgs) -> Result<()> {
    let manifest = load_manifest(&args.repo_root)?.unwrap_or_default();
    let mut values: Vec<(&str, String)> = Vec::new();
    if let Some(value) = manifest.product.name.as_deref().and_then(trimmed_option) {
        values.push(("product_name", sanitize_text(&value)));
    }
    if let Some(value) = manifest
        .product
        .description
        .as_deref()
        .and_then(trimmed_option)
    {
        values.push(("product_description", sanitize_text(&value)));
    }
    if let Some(value) = manifest.audience.as_deref().and_then(trimmed_option) {
        values.push(("audience", sanitize_text(&value)));
    }
    if let Some(value) = manifest.voice.as_deref().and_then(trimmed_option) {
        values.push(("voice_guide", sanitize_text(&value)));
    }
    if let Some(value) = manifest
        .changelog
        .source
        .as_deref()
        .and_then(trimmed_option)
    {
        values.push(("changelog_source", sanitize_text(&value)));
    }
    if let Some(value) = manifest
        .artifacts
        .markdown
        .as_deref()
        .and_then(trimmed_option)
    {
        values.push(("notes_output_file", sanitize_text(&value)));
    }
    if let Some(value) = manifest
        .artifacts
        .plaintext
        .as_deref()
        .and_then(trimmed_option)
    {
        values.push(("notes_output_text_file", sanitize_text(&value)));
    }
    if let Some(value) = manifest.artifacts.html.as_deref().and_then(trimmed_option) {
        values.push(("notes_output_html_file", sanitize_text(&value)));
    }
    if let Some(value) = manifest.artifacts.json.as_deref().and_then(trimmed_option) {
        values.push(("notes_output_json", sanitize_text(&value)));
    }
    if let Some(value) = manifest.artifacts.rss.as_deref().and_then(trimmed_option) {
        values.push(("rss_feed_file", sanitize_text(&value)));
    }
    if let Some(value) = manifest.model.policy.as_deref().and_then(trimmed_option) {
        values.push(("model_policy", sanitize_text(&value)));
    }
    if let Some(value) = manifest
        .model
        .primary
        .as_deref()
        .and_then(trimmed_option)
        .or_else(|| policy_default_model(manifest.model.policy.as_deref()))
    {
        values.push(("llm_model", sanitize_text(&value)));
    }
    if !manifest.model.fallbacks.is_empty() {
        values.push((
            "llm_fallback_models",
            sanitize_text(&manifest.model.fallbacks.join(",")),
        ));
    }
    if is_requested_path(Path::new(&args.github_output)) {
        write_outputs(Path::new(&args.github_output), &values)?;
    } else {
        let json: BTreeMap<_, _> = values.into_iter().collect();
        println!("{}", serde_json::to_string_pretty(&json)?);
    }
    Ok(())
}

fn infer_manifest(root: &Path) -> LandmarkManifest {
    let package = read_package_json(root);
    let package_name = package
        .as_ref()
        .and_then(|value| value["name"].as_str())
        .map(display_name_from_package);
    let readme_name = readme_title(root);
    let product_name = readme_name.or(package_name).or_else(|| {
        root.file_name()
            .and_then(|name| name.to_str())
            .map(display_name_from_package)
    });
    let description = package
        .as_ref()
        .and_then(|value| value["description"].as_str())
        .and_then(trimmed_option)
        .or_else(|| readme_description(root));
    let mut signals = Vec::new();
    let release_tool = detect_release_tool(root, package.as_ref(), &mut signals);
    LandmarkManifest {
        product: ProductManifest {
            name: product_name,
            description,
        },
        audience: Some(infer_audience(root, package.as_ref()).into()),
        voice: Some("clear, specific, user-facing".into()),
        changelog: ChangelogManifest {
            source: Some("auto".into()),
        },
        artifacts: ArtifactManifest {
            markdown: Some("docs/releases/{version}.md".into()),
            plaintext: None,
            html: None,
            json: Some("docs/releases/releases.json".into()),
            rss: None,
        },
        release: ReleaseManifest {
            profile: Some(if release_tool == "semantic-release" {
                "full".into()
            } else {
                "synthesis-only".into()
            }),
        },
        model: ModelManifest {
            policy: Some("balanced".into()),
            primary: None,
            fallbacks: Vec::new(),
        },
        budget: BudgetManifest {
            max_input_tokens: Some(12000),
            max_output_tokens: Some(1200),
            max_usd: None,
        },
    }
}

fn render_manifest_yaml(manifest: &LandmarkManifest) -> Result<String> {
    Ok(serde_yaml::to_string(manifest)?)
}

fn load_manifest(root: &Path) -> Result<Option<LandmarkManifest>> {
    let path = root.join(".landmark.yml");
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    let raw: serde_yaml::Value = serde_yaml::from_str(&text)?;
    let shape_errors = validate_manifest_yaml_shape(&raw);
    if !shape_errors.is_empty() {
        return Err(shape_errors.join("\n").into());
    }
    let manifest: LandmarkManifest = serde_yaml::from_str(&text)?;
    let errors = validate_manifest(&manifest);
    if errors.is_empty() {
        Ok(Some(manifest))
    } else {
        Err(errors.join("\n").into())
    }
}

fn validate_manifest_yaml_shape(raw: &serde_yaml::Value) -> Vec<String> {
    let mut errors = Vec::new();
    for (label, _, allowed) in manifest_schema_key_contracts() {
        validate_yaml_mapping_keys(yaml_value_at_label(raw, label), label, allowed, &mut errors);
    }
    errors
}

fn manifest_schema_key_contracts() -> Vec<(&'static str, &'static str, &'static [&'static str])> {
    vec![
        (
            "manifest",
            "/properties",
            &[
                "product",
                "audience",
                "voice",
                "changelog",
                "artifacts",
                "release",
                "model",
                "budget",
            ],
        ),
        (
            "manifest.product",
            "/properties/product/properties",
            &["name", "description"],
        ),
        (
            "manifest.changelog",
            "/properties/changelog/properties",
            &["source"],
        ),
        (
            "manifest.artifacts",
            "/properties/artifacts/properties",
            &["markdown", "plaintext", "html", "json", "rss"],
        ),
        (
            "manifest.release",
            "/properties/release/properties",
            &["profile"],
        ),
        (
            "manifest.model",
            "/properties/model/properties",
            &["policy", "primary", "fallbacks"],
        ),
        (
            "manifest.budget",
            "/properties/budget/properties",
            &["max_input_tokens", "max_output_tokens", "max_usd"],
        ),
    ]
}

fn yaml_value_at_label<'a>(raw: &'a serde_yaml::Value, label: &str) -> &'a serde_yaml::Value {
    match label {
        "manifest.product" => &raw["product"],
        "manifest.changelog" => &raw["changelog"],
        "manifest.artifacts" => &raw["artifacts"],
        "manifest.release" => &raw["release"],
        "manifest.model" => &raw["model"],
        "manifest.budget" => &raw["budget"],
        _ => raw,
    }
}

fn validate_yaml_mapping_keys(
    raw: &serde_yaml::Value,
    label: &str,
    allowed: &[&str],
    errors: &mut Vec<String>,
) {
    if raw.is_null() {
        return;
    }
    let Some(mapping) = raw.as_mapping() else {
        errors.push(format!("{label} must be a mapping"));
        return;
    };
    for key in mapping.keys() {
        let Some(key) = key.as_str() else {
            errors.push(format!("{label} keys must be strings"));
            continue;
        };
        if !allowed.contains(&key) {
            errors.push(format!("{label} contains unknown key `{key}`"));
        }
    }
}

fn validate_manifest(manifest: &LandmarkManifest) -> Vec<String> {
    let mut errors = Vec::new();
    for (name, value) in manifest_scalar_fields(manifest) {
        if value.contains('\n') || value.contains('\r') {
            errors.push(format!("manifest {name} must be a single-line scalar"));
        }
    }
    if let Some(audience) = manifest.audience.as_deref().and_then(trimmed_option)
        && !matches!(
            audience.as_str(),
            "general" | "developer" | "end-user" | "enterprise"
        )
    {
        errors.push(format!(
            "manifest audience must be general, developer, end-user, or enterprise; got {audience}"
        ));
    }
    if let Some(source) = manifest
        .changelog
        .source
        .as_deref()
        .and_then(trimmed_option)
        && !matches!(
            source.as_str(),
            "auto" | "changelog" | "release-body" | "prs"
        )
    {
        errors.push(format!(
            "manifest changelog.source must be auto, changelog, release-body, or prs; got {source}"
        ));
    }
    if let Some(profile) = manifest.release.profile.as_deref().and_then(trimmed_option)
        && !matches!(profile.as_str(), "full" | "synthesis-only")
    {
        errors.push(format!(
            "manifest release.profile must be full or synthesis-only; got {profile}"
        ));
    }
    if let Some(policy) = manifest.model.policy.as_deref().and_then(trimmed_option)
        && !matches!(policy.as_str(), "cheap" | "balanced" | "rich" | "off")
    {
        errors.push(format!(
            "manifest model.policy must be cheap, balanced, rich, or off; got {policy}"
        ));
    }
    errors
}

fn validate_manifest_completeness(manifest: &LandmarkManifest) -> Vec<String> {
    let mut errors = Vec::new();
    if manifest
        .product
        .name
        .as_deref()
        .and_then(trimmed_option)
        .is_none()
    {
        errors.push("manifest product.name is required for a complete Landmark manifest".into());
    }
    if manifest
        .product
        .description
        .as_deref()
        .and_then(trimmed_option)
        .is_none()
    {
        errors.push("manifest product.description is required for contextual release notes".into());
    }
    errors
}

fn manifest_scalar_fields(manifest: &LandmarkManifest) -> Vec<(&'static str, &str)> {
    let mut fields = Vec::new();
    push_scalar(
        &mut fields,
        "product.name",
        manifest.product.name.as_deref(),
    );
    push_scalar(
        &mut fields,
        "product.description",
        manifest.product.description.as_deref(),
    );
    push_scalar(&mut fields, "audience", manifest.audience.as_deref());
    push_scalar(&mut fields, "voice", manifest.voice.as_deref());
    push_scalar(
        &mut fields,
        "changelog.source",
        manifest.changelog.source.as_deref(),
    );
    push_scalar(
        &mut fields,
        "artifacts.markdown",
        manifest.artifacts.markdown.as_deref(),
    );
    push_scalar(
        &mut fields,
        "artifacts.plaintext",
        manifest.artifacts.plaintext.as_deref(),
    );
    push_scalar(
        &mut fields,
        "artifacts.html",
        manifest.artifacts.html.as_deref(),
    );
    push_scalar(
        &mut fields,
        "artifacts.json",
        manifest.artifacts.json.as_deref(),
    );
    push_scalar(
        &mut fields,
        "artifacts.rss",
        manifest.artifacts.rss.as_deref(),
    );
    push_scalar(
        &mut fields,
        "release.profile",
        manifest.release.profile.as_deref(),
    );
    push_scalar(
        &mut fields,
        "model.policy",
        manifest.model.policy.as_deref(),
    );
    push_scalar(
        &mut fields,
        "model.primary",
        manifest.model.primary.as_deref(),
    );
    for fallback in &manifest.model.fallbacks {
        fields.push(("model.fallbacks[]", fallback.as_str()));
    }
    fields
}

fn push_scalar<'a>(
    fields: &mut Vec<(&'static str, &'a str)>,
    name: &'static str,
    value: Option<&'a str>,
) {
    if let Some(value) = value {
        fields.push((name, value));
    }
}

fn readme_title(root: &Path) -> Option<String> {
    let readme = fs::read_to_string(root.join("README.md")).ok()?;
    readme
        .lines()
        .find_map(|line| line.trim().strip_prefix("# "))
        .and_then(trimmed_option)
}

fn readme_description(root: &Path) -> Option<String> {
    let readme = fs::read_to_string(root.join("README.md")).ok()?;
    readme
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .and_then(trimmed_option)
}

fn display_name_from_package(name: &str) -> String {
    let name = name.rsplit('/').next().unwrap_or(name);
    name.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn infer_audience(root: &Path, package: Option<&Value>) -> &'static str {
    if root.join("Cargo.toml").is_file()
        || root.join("pyproject.toml").is_file()
        || root.join("go.mod").is_file()
        || package.is_some()
    {
        "developer"
    } else {
        "general"
    }
}

fn trimmed_option(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[derive(Serialize)]
struct SetupReport {
    diagnosis: SetupDiagnosis,
    recommendation: SetupRecommendation,
    required_permissions: BTreeMap<String, String>,
    required_secrets: Vec<String>,
    workflows: BTreeMap<String, WorkflowCandidate>,
    manifest: Option<LandmarkManifest>,
    backfill: String,
}

#[derive(Serialize)]
struct SetupDiagnosis {
    release_tool: String,
    default_branch: String,
    tag_format: String,
    conventional_commits: String,
    monorepo: bool,
    packages: Vec<String>,
    signals: Vec<String>,
}

#[derive(Serialize)]
struct SetupRecommendation {
    mode: String,
    workflow: String,
    rationale: Vec<String>,
}

#[derive(Serialize)]
struct WorkflowCandidate {
    path: String,
    release_tool: String,
    mode: String,
    rationale: Vec<String>,
    content: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FleetScan {
    generated_at: String,
    owners: Vec<String>,
    repositories: Vec<FleetRepository>,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FleetRepository {
    owner: String,
    name: String,
    name_with_owner: String,
    #[serde(default)]
    repository_kind: String,
    #[serde(default)]
    release_surface: String,
    private: bool,
    archived: bool,
    pushed_at: String,
    default_branch: String,
    branch_protected: String,
    release_tool: String,
    tag_format: String,
    package_topology: Vec<String>,
    release_files: Vec<String>,
    workflows: Vec<String>,
    #[serde(default)]
    workflow_files: Vec<FleetWorkflowFile>,
    existing_landmark: bool,
    required_secrets: Vec<FleetSecretStatus>,
    signals: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct FleetWorkflowFile {
    path: String,
    #[serde(default)]
    release_tool: String,
    #[serde(default)]
    release_job: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    content: String,
    #[serde(default)]
    content_redacted: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FleetSecretStatus {
    name: String,
    status: String,
    detail: String,
}

#[derive(Clone, Debug, Default)]
struct FleetSecretNames {
    names: BTreeSet<String>,
    repo_names: BTreeSet<String>,
    org_names: BTreeSet<String>,
    org_unavailable: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FleetPlan {
    generated_at: String,
    source: String,
    summary: BTreeMap<String, usize>,
    repositories: Vec<FleetRepositoryPlan>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FleetRepositoryPlan {
    repository: String,
    repository_kind: String,
    release_surface: String,
    rank: u64,
    default_branch: String,
    recommended_mode: String,
    integration_mode: String,
    integration_rationale: Vec<String>,
    workflow: String,
    status: String,
    skip_reason: String,
    risk_flags: Vec<String>,
    required_secrets: Vec<String>,
    missing_secrets: Vec<String>,
    unavailable_secret_metadata: Vec<String>,
    migration_notes: Vec<String>,
    #[serde(default)]
    initial_version_recommendation: String,
    #[serde(default)]
    initial_tag_recommendation: String,
    #[serde(default)]
    artifact_paths: Vec<String>,
    #[serde(default)]
    historical_preview_command: String,
    #[serde(default)]
    rollback_guidance: String,
    #[serde(default)]
    workflow_patches: Vec<FleetWorkflowPatch>,
    manifest: LandmarkManifest,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct FleetWorkflowPatch {
    path: String,
    description: String,
    content: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FleetPrPlan {
    generated_at: String,
    dry_run: bool,
    repositories: Vec<FleetRepositoryPrPlan>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FleetRepositoryPrPlan {
    repository: String,
    branch: String,
    title: String,
    commit_message: String,
    files: Vec<String>,
    skipped: bool,
    reason: String,
    disposition: String,
    rollback: String,
    monitor_status: String,
    evidence_dir: String,
}

fn setup(args: SetupArgs) -> Result<()> {
    let diagnosis = diagnose_setup(&args.repo_root);
    let manifest = load_manifest(&args.repo_root)?;
    let recommendation = recommend_setup(&diagnosis, manifest.as_ref());
    let workflows = setup_workflows(&diagnosis, manifest.as_ref());
    if !args.dry_run && !args.output_dir.trim().is_empty() {
        let output_dir = args.repo_root.join(args.output_dir.trim());
        fs::create_dir_all(&output_dir)?;
        for candidate in workflows.values() {
            let filename = Path::new(&candidate.path)
                .file_name()
                .unwrap_or_else(|| OsStr::new("landmark-release.yml"));
            fs::write(output_dir.join(filename), &candidate.content)?;
        }
    }
    let mut required_permissions = BTreeMap::new();
    required_permissions.insert("contents".into(), "write".into());
    required_permissions.insert("issues".into(), "write".into());
    required_permissions.insert("pull-requests".into(), "write".into());
    let report = SetupReport {
        diagnosis,
        recommendation,
        required_permissions,
        required_secrets: vec!["GH_RELEASE_TOKEN".into(), "OPENROUTER_API_KEY".into()],
        workflows,
        manifest,
        backfill: "available: run `landmark backfill --repo-root . --since <tag> --dry-run` to plan historical artifacts; use `--mode artifacts-only` for safe migration output and preview `--mode release-body --dry-run` before any release-body update".into(),
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn fleet(args: FleetArgs) -> Result<()> {
    match args.command {
        FleetCommand::Scan(args) => fleet_scan(args),
        FleetCommand::Plan(args) => fleet_plan(args),
        FleetCommand::OpenPrs(args) => fleet_open_prs(args),
    }
}

fn fleet_scan(args: FleetScanArgs) -> Result<()> {
    if !matches!(args.format.as_str(), "text" | "json") {
        return Err("--format must be text or json".into());
    }
    if !args.fixture.trim().is_empty() {
        let scan: FleetScan = serde_json::from_str(&fs::read_to_string(&args.fixture)?)?;
        write_json_if_requested(&args.output, &scan)?;
        print_fleet_scan_result(&args.output, &scan, &args.format)?;
        return Ok(());
    }
    if args.owner.is_empty() {
        return Err("fleet scan requires at least one --owner".into());
    }
    let mut warnings = Vec::new();
    let token = trimmed_option(&args.github_token)
        .or_else(|| {
            env::var("GITHUB_TOKEN")
                .ok()
                .and_then(|value| trimmed_option(&value))
        })
        .or_else(|| {
            env::var("GH_TOKEN")
                .ok()
                .and_then(|value| trimmed_option(&value))
        });

    let mut repo_values = Vec::new();
    for owner in &args.owner {
        let repos = match gh_repo_list(owner, args.max_repos, token.as_deref()) {
            Ok(repos) => repos,
            Err(error) => {
                warnings.push(format!(
                    "owner {owner}: repository list unavailable: {error}"
                ));
                continue;
            }
        };
        for repo in repos {
            if args.active_only && repo["isArchived"].as_bool().unwrap_or(false) {
                continue;
            }
            repo_values.push(repo);
        }
    }
    let (mut repositories, scan_warnings) = scan_fleet_repositories_bounded(
        repo_values,
        &args.api_base_url,
        token.as_deref(),
        args.deep_checks,
        args.concurrency,
    );
    warnings.extend(scan_warnings);

    repositories.sort_by(|left, right| left.name_with_owner.cmp(&right.name_with_owner));
    let scan = FleetScan {
        generated_at: Utc::now().to_rfc3339(),
        owners: args.owner,
        repositories,
        warnings,
    };
    write_json_if_requested(&args.output, &scan)?;
    print_fleet_scan_result(&args.output, &scan, &args.format)?;
    Ok(())
}

fn fleet_plan(args: FleetPlanArgs) -> Result<()> {
    if !matches!(args.format.as_str(), "text" | "json") {
        return Err("--format must be text or json".into());
    }
    let scan: FleetScan = serde_json::from_str(&fs::read_to_string(&args.input)?)?;
    fs::create_dir_all(&args.output_dir)?;
    let mut repositories: Vec<_> = scan
        .repositories
        .iter()
        .map(plan_fleet_repository)
        .collect();
    repositories.sort_by(|left, right| {
        right
            .rank
            .cmp(&left.rank)
            .then_with(|| left.repository.cmp(&right.repository))
    });
    let mut summary = BTreeMap::new();
    for repo in &repositories {
        *summary.entry(repo.status.clone()).or_insert(0) += 1;
    }
    let plan = FleetPlan {
        generated_at: Utc::now().to_rfc3339(),
        source: args.input.display().to_string(),
        summary,
        repositories,
    };
    let plan_path = args.output_dir.join("plan.json");
    fs::write(&plan_path, serde_json::to_string_pretty(&plan)? + "\n")?;
    fs::write(
        args.output_dir.join("README.md"),
        render_fleet_plan_markdown(&plan),
    )?;
    if args.format == "json" {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        println!(
            "fleet plan wrote {} and {} ({} repositories)",
            plan_path.display(),
            args.output_dir.join("README.md").display(),
            plan.repositories.len()
        );
    }
    Ok(())
}

fn fleet_open_prs(args: FleetOpenPrsArgs) -> Result<()> {
    if !matches!(args.format.as_str(), "text" | "json") {
        return Err("--format must be text or json".into());
    }
    if !args.dry_run && !args.confirm_remote {
        return Err(
            "fleet open-prs non-dry-run requires --confirm-remote; refusing unconfirmed rollout"
                .into(),
        );
    }
    if !args.dry_run && args.max_prs != 1 {
        return Err(
            "fleet open-prs confirmed rollout requires --max-prs 1 so downstream monitoring gates the next repository"
                .into(),
        );
    }
    let plan_path = args.plan_dir.join("plan.json");
    let plan: FleetPlan = serde_json::from_str(&fs::read_to_string(&plan_path)?)?;
    fs::create_dir_all(&args.output_dir)?;
    let mut rendered = Vec::new();
    let mut opened = 0usize;
    for repo in &plan.repositories {
        if args.max_prs > 0 && opened >= args.max_prs {
            break;
        }
        let slug = repo.repository.replace('/', "__");
        let repo_dir = args.output_dir.join(&slug);
        fs::create_dir_all(&repo_dir)?;
        if repo.status == "skipped" || repo.status == "blocked" {
            let reason = if repo.skip_reason.is_empty() {
                repo.status.clone()
            } else {
                repo.skip_reason.clone()
            };
            fs::write(repo_dir.join("SKIPPED.md"), format!("{reason}\n"))?;
            rendered.push(FleetRepositoryPrPlan {
                repository: repo.repository.clone(),
                branch: String::new(),
                title: String::new(),
                commit_message: String::new(),
                files: vec!["SKIPPED.md".into()],
                skipped: true,
                reason,
                disposition: "skipped".into(),
                rollback: String::new(),
                monitor_status: "not-applicable".into(),
                evidence_dir: repo_dir.display().to_string(),
            });
            continue;
        }
        opened += 1;
        let manifest = render_manifest_yaml(&repo.manifest)?;
        fs::write(repo_dir.join(".landmark.yml"), &manifest)?;
        let mut workflow_files = Vec::new();
        for patch in &repo.workflow_patches {
            fs::create_dir_all(
                repo_dir.join(Path::new(&patch.path).parent().unwrap_or(Path::new("."))),
            )?;
            fs::write(repo_dir.join(&patch.path), &patch.content)?;
            workflow_files.push((patch.path.clone(), patch.content.clone()));
        }
        if fleet_pr_should_write_workflow(repo) {
            let workflow = fleet_workflow_for_plan(repo);
            fs::create_dir_all(repo_dir.join(".github/workflows"))?;
            fs::write(
                repo_dir.join(".github/workflows/landmark-release.yml"),
                &workflow,
            )?;
            workflow_files.push((".github/workflows/landmark-release.yml".into(), workflow));
        }
        let diff = render_fleet_pr_diff(repo, &manifest, &workflow_files);
        fs::write(repo_dir.join("diff.md"), diff)?;
        let mut files = vec![".landmark.yml".into()];
        files.extend(workflow_files.iter().map(|(path, _)| path.clone()));
        files.push("diff.md".into());
        let branch = format!("landmark/adopt-{}", repo.repository.replace('/', "-"));
        let title: String = match repo.recommended_mode.as_str() {
            "manifest-only" => "chore(release): configure Landmark manifest".into(),
            "backfill-first" => "chore(release): configure Landmark backfill artifacts".into(),
            _ => "chore(release): adopt Landmark".into(),
        };
        let commit_message = match repo.recommended_mode.as_str() {
            "manifest-only" => "chore(release): configure Landmark manifest".into(),
            "backfill-first" => "chore(release): configure Landmark backfill-first".into(),
            _ => format!("chore(release): adopt Landmark {}", repo.integration_mode),
        };
        if !args.dry_run {
            fs::write(
                repo_dir.join("APPLY.md"),
                render_fleet_apply_markdown(repo, &branch, &title, &commit_message, &files),
            )?;
            files.push("APPLY.md".into());
        }
        rendered.push(FleetRepositoryPrPlan {
            repository: repo.repository.clone(),
            branch: branch.clone(),
            title,
            commit_message,
            files,
            skipped: false,
            reason: String::new(),
            disposition: if args.dry_run {
                "dry-run-rendered".into()
            } else {
                "confirmed-operator-apply-required; APPLY.md contains the branch, commit, push, PR, rollback, and monitor commands".into()
            },
            rollback: if repo.rollback_guidance.is_empty() {
                format!("if applied, close the PR and delete branch {}", branch)
            } else {
                repo.rollback_guidance.clone()
            },
            monitor_status: "pending: merge one repository, monitor downstream release, then continue rollout".into(),
            evidence_dir: repo_dir.display().to_string(),
        });
    }
    let pr_plan = FleetPrPlan {
        generated_at: Utc::now().to_rfc3339(),
        dry_run: args.dry_run,
        repositories: rendered,
    };
    fs::write(
        args.output_dir.join("open-prs.json"),
        serde_json::to_string_pretty(&pr_plan)? + "\n",
    )?;
    if args.format == "json" {
        println!("{}", serde_json::to_string_pretty(&pr_plan)?);
    } else {
        println!(
            "fleet {} wrote {} ({} repositories)",
            if pr_plan.dry_run {
                "dry-run"
            } else {
                "rollout receipt"
            },
            args.output_dir.join("open-prs.json").display(),
            pr_plan.repositories.len()
        );
    }
    Ok(())
}

fn print_fleet_scan_result(path: &Path, scan: &FleetScan, format: &str) -> Result<()> {
    if format == "json" || !is_requested_path(path) {
        println!("{}", serde_json::to_string_pretty(scan)?);
    } else {
        println!(
            "fleet scan wrote {} ({} repositories, {} warnings)",
            path.display(),
            scan.repositories.len(),
            scan.warnings.len()
        );
    }
    Ok(())
}

fn gh_repo_list(owner: &str, max_repos: usize, token: Option<&str>) -> Result<Vec<Value>> {
    let limit = if max_repos == 0 {
        "1000".to_string()
    } else {
        max_repos.to_string()
    };
    let output = run_gh_ok(
        vec![
            "repo".into(),
            "list".into(),
            owner.into(),
            "--limit".into(),
            limit,
            "--json".into(),
            "name,nameWithOwner,isArchived,isPrivate,pushedAt,defaultBranchRef".into(),
        ],
        token,
    )?;
    Ok(serde_json::from_str(&output)?)
}

fn scan_fleet_repositories_bounded(
    repos: Vec<Value>,
    api_base_url: &str,
    token: Option<&str>,
    deep_checks: bool,
    concurrency: usize,
) -> (Vec<FleetRepository>, Vec<String>) {
    if repos.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let worker_count = concurrency.clamp(1, 16).min(repos.len());
    let queue = Arc::new(Mutex::new(VecDeque::from(repos)));
    let results = Arc::new(Mutex::new(Vec::new()));
    let warnings = Arc::new(Mutex::new(Vec::new()));
    let api_base_url = api_base_url.to_string();
    let token = token.map(str::to_string);

    thread::scope(|scope| {
        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            let results = Arc::clone(&results);
            let warnings = Arc::clone(&warnings);
            let api_base_url = api_base_url.clone();
            let token = token.clone();
            scope.spawn(move || {
                loop {
                    let repo = {
                        let mut queue = queue.lock().unwrap();
                        queue.pop_front()
                    };
                    let Some(repo) = repo else {
                        break;
                    };
                    match scan_fleet_repository(&repo, &api_base_url, token.as_deref(), deep_checks)
                    {
                        Ok(repository) => results.lock().unwrap().push(repository),
                        Err(error) => warnings.lock().unwrap().push(format!(
                            "{}: scan degraded: {error}",
                            repo["nameWithOwner"].as_str().unwrap_or("<unknown>")
                        )),
                    }
                }
            });
        }
    });

    let repositories = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
    let warnings = Arc::try_unwrap(warnings).unwrap().into_inner().unwrap();
    (repositories, warnings)
}

fn scan_fleet_repository(
    repo: &Value,
    api_base_url: &str,
    token: Option<&str>,
    deep_checks: bool,
) -> Result<FleetRepository> {
    let name_with_owner = repo["nameWithOwner"]
        .as_str()
        .ok_or("gh repo list response missing nameWithOwner")?
        .to_string();
    let (owner, name) = name_with_owner
        .split_once('/')
        .ok_or("repository must be owner/name")?;
    let default_branch = repo["defaultBranchRef"]["name"]
        .as_str()
        .or_else(|| repo["defaultBranchRef"].as_str())
        .unwrap_or("main")
        .to_string();
    let archived = repo["isArchived"].as_bool().unwrap_or(false);
    let private = repo["isPrivate"].as_bool().unwrap_or(false);
    let pushed_at = repo["pushedAt"].as_str().unwrap_or("").to_string();
    let provider = GitHubProvider::new(api_base_url, token);
    let paths = provider.tree_paths(&name_with_owner, &default_branch)?;
    let path_set: BTreeSet<_> = paths.iter().map(String::as_str).collect();
    let workflows = paths
        .iter()
        .filter_map(|path| path.strip_prefix(".github/workflows/"))
        .filter(|name| name.ends_with(".yml") || name.ends_with(".yaml"))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let workflow_texts = provider.workflow_texts(&name_with_owner, &default_branch, &workflows);
    let workflow_files = workflow_texts
        .iter()
        .filter_map(|(workflow, text)| {
            fleet_workflow_file(&format!(".github/workflows/{workflow}"), text)
        })
        .collect::<Vec<_>>();
    let mut release_files = Vec::new();
    let mut package_topology = Vec::new();
    let mut signals = Vec::new();
    for file in [
        "package.json",
        "Cargo.toml",
        "pyproject.toml",
        "go.mod",
        ".releaserc",
        ".releaserc.json",
        "release-please-config.json",
        ".landmark.yml",
    ] {
        if path_set.contains(file) {
            if matches!(
                file,
                "package.json" | "Cargo.toml" | "pyproject.toml" | "go.mod"
            ) {
                package_topology.push(file.to_string());
            } else {
                release_files.push(file.to_string());
            }
            signals.push(format!("{file} present"));
        }
    }
    if path_set.contains(".changeset") || paths.iter().any(|path| path.starts_with(".changeset/")) {
        release_files.push(".changeset/".into());
        signals.push(".changeset directory present".into());
    }
    let tags = provider.tags(&name_with_owner)?;
    let tag_format = fleet_tag_format(&tags, &package_topology);
    let release_tool = fleet_release_tool(&release_files, &workflows, &workflow_texts, &tags);
    let repository_kind = if archived {
        "archived".into()
    } else {
        classify_fleet_repository_kind(name, &package_topology, &release_files)
    };
    let release_surface = classify_fleet_release_surface(&release_tool, &tags, &workflow_texts);
    let existing_landmark = release_files.iter().any(|file| file == ".landmark.yml")
        || workflows
            .iter()
            .any(|workflow| workflow.to_ascii_lowercase().contains("landmark"))
        || workflow_texts
            .iter()
            .any(|(_, text)| workflow_invokes_landmark(text));
    for (workflow, text) in &workflow_texts {
        if workflow_invokes_landmark(text) {
            signals.push(format!("{workflow} invokes Landmark action"));
        }
    }
    let branch_protected = if deep_checks {
        provider.branch_protection_status(&name_with_owner, &default_branch)
    } else {
        "unavailable: pass --deep-checks to query branch protection metadata".into()
    };
    let required_secrets = if deep_checks {
        provider.secret_statuses(
            &name_with_owner,
            &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
        )
    } else {
        unavailable_secret_statuses(
            &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
            "pass --deep-checks to query Actions secret metadata",
        )
    };
    Ok(FleetRepository {
        owner: owner.to_string(),
        name: name.to_string(),
        name_with_owner,
        repository_kind,
        release_surface,
        private,
        archived,
        pushed_at,
        default_branch,
        branch_protected,
        release_tool,
        tag_format,
        package_topology,
        release_files,
        workflows,
        workflow_files,
        existing_landmark,
        required_secrets,
        signals,
    })
}

fn run_gh_ok(args: Vec<String>, token: Option<&str>) -> Result<String> {
    let mut command = Command::new("gh");
    command.args(args).current_dir(Path::new("."));
    if let Some(token) = token {
        command.env("GH_TOKEN", token);
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!("gh failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn unavailable_secret_statuses(required: &[&str], detail: &str) -> Vec<FleetSecretStatus> {
    required
        .iter()
        .map(|name| FleetSecretStatus {
            name: (*name).into(),
            status: "unavailable".into(),
            detail: detail.into(),
        })
        .collect()
}

fn secret_names_from_array(value: &Value) -> BTreeSet<String> {
    value["secrets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|secret| secret["name"].as_str())
        .map(str::to_string)
        .collect()
}

fn org_secret_names_for_repo(value: &Value, repository: &str, repo_name: &str) -> BTreeSet<String> {
    value["secrets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|secret| match secret["visibility"].as_str().unwrap_or("") {
            "all" => true,
            "selected" => secret["selected_repositories"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|selected| {
                    selected["full_name"].as_str() == Some(repository)
                        || selected["name"].as_str() == Some(repo_name)
                }),
            _ => false,
        })
        .filter_map(|secret| secret["name"].as_str())
        .map(str::to_string)
        .collect()
}

fn workflow_invokes_landmark(text: &str) -> bool {
    workflow_invokes_landmark_action(text)
}

fn workflow_invokes_landmark_action(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("misty-step/landmark") || lower.contains("misty-step/landmark")
}

fn fleet_workflow_file(path: &str, text: &str) -> Option<FleetWorkflowFile> {
    let lower = text.to_ascii_lowercase();
    let (release_tool, marker, default_job) = if lower.contains("googleapis/release-please-action")
    {
        (
            "release-please",
            "googleapis/release-please-action",
            "release-please",
        )
    } else if lower.contains("changesets/action") {
        ("changesets", "changesets/action", "release")
    } else if lower.contains("semantic-release") {
        ("semantic-release", "semantic-release", "release")
    } else if lower.contains("gh release create") {
        ("manual-tag", "gh release create", "release")
    } else {
        return None;
    };
    let release_job =
        workflow_job_invoking(text, marker).unwrap_or_else(|| default_job.to_string());
    let redacted = redact_context(text);
    let content_redacted = redacted != text;
    Some(FleetWorkflowFile {
        path: path.into(),
        release_tool: release_tool.into(),
        release_job,
        content: if content_redacted {
            String::new()
        } else {
            text.into()
        },
        content_redacted,
    })
}

fn workflow_job_invoking(text: &str, marker: &str) -> Option<String> {
    let raw: serde_yaml::Value = serde_yaml::from_str(text).ok()?;
    let jobs = raw.get("jobs")?.as_mapping()?;
    let marker = marker.to_ascii_lowercase();
    for (key, job) in jobs {
        let job_id = key.as_str()?;
        let job_text = serde_yaml::to_string(job).ok()?.to_ascii_lowercase();
        if job_text.contains(&marker) {
            return Some(job_id.to_string());
        }
    }
    None
}

fn fleet_release_tool(
    files: &[String],
    workflows: &[String],
    workflow_texts: &[(String, String)],
    tags: &[String],
) -> String {
    let haystack = files
        .iter()
        .chain(workflows)
        .map(|value| value.as_str())
        .chain(workflow_texts.iter().map(|(_, text)| text.as_str()))
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("\n");
    if haystack.contains("release-please") {
        "release-please".into()
    } else if haystack.contains("changeset") {
        "changesets".into()
    } else if haystack.contains(".releaserc") || haystack.contains("semantic") {
        "semantic-release".into()
    } else if !tags.is_empty() || haystack.contains("release") {
        "manual-tag".into()
    } else {
        "no-release-tool".into()
    }
}

fn fleet_tag_format(tags: &[String], packages: &[String]) -> String {
    let package_re = Regex::new(r"^[A-Za-z0-9_.-]+@v?[0-9]+\.[0-9]+\.[0-9]+").unwrap();
    let v_re = Regex::new(r"^v[0-9]+\.[0-9]+\.[0-9]+").unwrap();
    let bare_re = Regex::new(r"^[0-9]+\.[0-9]+\.[0-9]+").unwrap();
    if tags.iter().any(|tag| package_re.is_match(tag)) || packages.len() > 1 {
        "package@{version}".into()
    } else if tags.iter().any(|tag| v_re.is_match(tag)) || tags.is_empty() {
        "v{version}".into()
    } else if tags.iter().any(|tag| bare_re.is_match(tag)) {
        "{version}".into()
    } else {
        "custom".into()
    }
}

fn classify_fleet_repository_kind(
    name: &str,
    package_topology: &[String],
    release_files: &[String],
) -> String {
    let lower = name.to_ascii_lowercase();
    if lower.contains("docs")
        || lower.contains("documentation")
        || lower.contains("config")
        || lower.contains("dotfiles")
        || lower.contains("template")
        || lower.contains("prompt")
        || (package_topology.is_empty() && release_files.is_empty())
    {
        "non-release".into()
    } else if lower.contains("experiment")
        || lower.contains("prototype")
        || lower.contains("scratch")
        || lower.contains("demo")
    {
        "experiment".into()
    } else if lower.contains("infra")
        || lower.contains("terraform")
        || lower.contains("deploy")
        || lower.contains("ops")
    {
        "infrastructure".into()
    } else if lower.contains("lib")
        || lower.contains("sdk")
        || lower.contains("crate")
        || package_topology.iter().any(|path| path == "Cargo.toml")
            && !release_files.iter().any(|file| file.contains("semantic"))
    {
        "library".into()
    } else {
        "application".into()
    }
}

fn classify_fleet_release_surface(
    release_tool: &str,
    tags: &[String],
    workflow_texts: &[(String, String)],
) -> String {
    if workflow_texts.iter().any(|(_, text)| {
        let lower = text.to_ascii_lowercase();
        lower.contains("cargo publish") || lower.contains("npm publish")
    }) {
        "package-registry".into()
    } else if release_tool == "no-release-tool" && tags.is_empty() {
        "none".into()
    } else if release_tool == "semantic-release" {
        "github-release+semantic-release".into()
    } else if matches!(release_tool, "release-please" | "changesets" | "manual-tag") {
        "github-release".into()
    } else if !tags.is_empty() {
        "git-tags".into()
    } else {
        "local-artifacts".into()
    }
}

fn normalized_repo_kind(repo: &FleetRepository) -> String {
    trimmed_option(&repo.repository_kind)
        .unwrap_or_else(|| {
            classify_fleet_repository_kind(&repo.name, &repo.package_topology, &repo.release_files)
        })
        .to_ascii_lowercase()
}

fn normalized_release_surface(repo: &FleetRepository) -> String {
    trimmed_option(&repo.release_surface)
        .unwrap_or_else(|| classify_fleet_release_surface(&repo.release_tool, &[], &[]))
        .to_ascii_lowercase()
}

fn fleet_workflow_file_for_tool<'a>(
    repo: &'a FleetRepository,
    release_tool: &str,
) -> Option<&'a FleetWorkflowFile> {
    repo.workflow_files
        .iter()
        .find(|workflow| workflow.release_tool == release_tool)
}

fn workflow_content_blocker(
    repo: &FleetRepository,
    integration_mode: &str,
    workflow: &str,
) -> Option<String> {
    if integration_mode != "github-synthesis-only" {
        return None;
    }
    let workflow_file = match workflow {
        "release-please" => fleet_workflow_file_for_tool(repo, "release-please"),
        "changesets" => fleet_workflow_file_for_tool(repo, "changesets"),
        _ => None,
    }?;
    if workflow_file.content_redacted {
        Some(format!(
            "{} contains secret-like literals; refusing to render workflow patch",
            workflow_file.path
        ))
    } else if !workflow_file.content.is_empty()
        && !workflow_has_jobs_mapping(&workflow_file.content)
    {
        Some(format!(
            "{} does not contain a patchable jobs mapping; refusing to render workflow patch",
            workflow_file.path
        ))
    } else {
        None
    }
}

fn workflow_has_jobs_mapping(text: &str) -> bool {
    let Ok(raw) = serde_yaml::from_str::<serde_yaml::Value>(text) else {
        return false;
    };
    raw.get("jobs")
        .and_then(serde_yaml::Value::as_mapping)
        .is_some()
}

fn fleet_integration_mode(
    repo: &FleetRepository,
    repository_kind: &str,
) -> (String, String, Vec<String>, u64) {
    if repo.archived {
        return (
            "skipped".into(),
            "none".into(),
            vec!["repository is archived".into()],
            0,
        );
    }
    if repo.existing_landmark {
        return (
            "manifest-only".into(),
            "manual-tag".into(),
            vec!["existing Landmark manifest or workflow detected".into()],
            65,
        );
    }
    if repository_kind == "non-release" {
        return (
            "skipped".into(),
            "none".into(),
            vec!["repository kind should not publish releases".into()],
            0,
        );
    }
    if repository_kind == "infrastructure" {
        return (
            "generic-ci".into(),
            "generic-ci".into(),
            vec!["infrastructure repository should publish portable local artifacts before GitHub release mutation".into()],
            55,
        );
    }
    if repository_kind == "experiment" {
        return (
            "local".into(),
            "local".into(),
            vec!["experiment repository should start with zero-secret local previews".into()],
            45,
        );
    }
    match repo.release_tool.as_str() {
        "semantic-release" if fleet_workflow_file_for_tool(repo, "semantic-release").is_some() => (
            "blocked".into(),
            "semantic-release".into(),
            vec!["existing semantic-release workflow requires explicit operator choice before Landmark full-mode replacement".into()],
            35,
        ),
        "semantic-release" => (
            "github-full".into(),
            "semantic-release".into(),
            vec!["semantic-release can let Landmark own full GitHub Action release mode".into()],
            100,
        ),
        "release-please" => (
            "github-synthesis-only".into(),
            "release-please".into(),
            vec!["release-please already owns versioning; Landmark should synthesize after release creation".into()],
            85,
        ),
        "changesets" => (
            "github-synthesis-only".into(),
            "changesets".into(),
            vec!["Changesets already owns package publication; Landmark should synthesize per published release".into()],
            80,
        ),
        "manual-tag" => (
            "github-synthesis-only".into(),
            "manual-tag".into(),
            vec!["manual GitHub releases should trigger one synthesis-only Landmark run".into()],
            60,
        ),
        "no-release-tool" => (
            "backfill-first".into(),
            "manual-tag".into(),
            vec!["no release automation detected; plan historical artifacts before adding release mutation".into()],
            20,
        ),
        _ => (
            "blocked".into(),
            "manual-tag".into(),
            vec!["unknown release tooling".into()],
            10,
        ),
    }
}

fn fleet_required_secret_names(integration_mode: &str) -> Vec<String> {
    match integration_mode {
        "github-full" | "github-synthesis-only" => {
            vec!["GH_RELEASE_TOKEN".into(), "OPENROUTER_API_KEY".into()]
        }
        _ => Vec::new(),
    }
}

fn fleet_initial_version(recommended_mode: &str, status: &str) -> String {
    if recommended_mode == "backfill-first" && status == "ready" {
        "0.1.0".into()
    } else {
        String::new()
    }
}

fn fleet_initial_tag(repo: &FleetRepository, version: &str) -> String {
    if version.is_empty() {
        return String::new();
    }
    match repo.tag_format.as_str() {
        "{version}" => version.into(),
        "package@{version}" => format!("{}@{}", repo.name, version),
        "custom" => format!("{version} (custom tag format requires operator approval)"),
        _ => format!("v{version}"),
    }
}

fn fleet_manifest_artifact_paths(manifest: &LandmarkManifest) -> Vec<String> {
    let mut paths = Vec::new();
    for path in [
        manifest.artifacts.markdown.as_deref(),
        manifest.artifacts.plaintext.as_deref(),
        manifest.artifacts.html.as_deref(),
        manifest.artifacts.json.as_deref(),
        manifest.artifacts.rss.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if !path.trim().is_empty() {
            paths.push(path.to_string());
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn fleet_historical_preview_command(tag: &str) -> String {
    if tag.is_empty() {
        String::new()
    } else {
        format!("landmark backfill --repo-root . --since {tag} --mode artifacts-only --dry-run")
    }
}

fn fleet_rollback_guidance(recommended_mode: &str) -> String {
    if recommended_mode == "backfill-first" {
        "close the PR and delete the adoption branch; remove .landmark.yml and any previewed local artifact files, and remove the operator-approved initial tag only if it was created before any release was published".into()
    } else {
        String::new()
    }
}

fn plan_fleet_repository(repo: &FleetRepository) -> FleetRepositoryPlan {
    let mut risk_flags = Vec::new();
    let mut migration_notes = Vec::new();
    let repository_kind = normalized_repo_kind(repo);
    let release_surface = normalized_release_surface(repo);
    let (integration_mode, workflow, mut integration_rationale, rank_base) =
        fleet_integration_mode(repo, &repository_kind);
    let required_secrets = fleet_required_secret_names(&integration_mode);
    let observed_secret_names = repo
        .required_secrets
        .iter()
        .map(|secret| secret.name.clone())
        .collect::<BTreeSet<_>>();
    let mut missing_secrets = repo
        .required_secrets
        .iter()
        .filter(|secret| {
            required_secrets
                .iter()
                .any(|required| required == &secret.name)
        })
        .filter(|secret| secret.status == "missing")
        .map(|secret| secret.name.clone())
        .collect::<Vec<_>>();
    let mut unavailable_secret_metadata = repo
        .required_secrets
        .iter()
        .filter(|secret| {
            required_secrets
                .iter()
                .any(|required| required == &secret.name)
        })
        .filter(|secret| secret.status == "unavailable")
        .map(|secret| secret.name.clone())
        .collect::<Vec<_>>();
    for required in &required_secrets {
        if !observed_secret_names.contains(required) {
            unavailable_secret_metadata.push(required.clone());
        }
    }
    missing_secrets.sort();
    missing_secrets.dedup();
    unavailable_secret_metadata.sort();
    unavailable_secret_metadata.dedup();
    if repo.private {
        risk_flags
            .push("private repository; verify token and secret policy before opening PR".into());
    }
    if repo.branch_protected == "protected" {
        risk_flags.push(format!(
            "default branch {} is protected",
            repo.default_branch
        ));
    }
    if !missing_secrets.is_empty() {
        risk_flags.push(format!(
            "missing required Actions secrets: {}",
            missing_secrets.join(", ")
        ));
    }
    if !unavailable_secret_metadata.is_empty() {
        risk_flags
            .push("secret metadata unavailable; operator must verify required secrets".into());
    }

    let secret_blocker = if !missing_secrets.is_empty() {
        Some(format!(
            "missing required Actions secrets: {}",
            missing_secrets.join(", ")
        ))
    } else if !unavailable_secret_metadata.is_empty() {
        Some(format!(
            "secret metadata unavailable for required secrets: {}",
            unavailable_secret_metadata.join(", ")
        ))
    } else {
        None
    };
    let workflow_blocker = workflow_content_blocker(repo, &integration_mode, &workflow);

    let (status, recommended_mode, skip_reason, rank) = if repo.archived {
        ("skipped", "skipped", "repository is archived", 0u64)
    } else if repository_kind == "non-release" {
        (
            "skipped",
            "skipped",
            "repository is classified as non-release",
            0u64,
        )
    } else if let Some(reason) = secret_blocker.as_deref() {
        ("blocked", integration_mode.as_str(), reason, 15u64)
    } else if let Some(reason) = workflow_blocker.as_deref() {
        ("blocked", integration_mode.as_str(), reason, 15u64)
    } else if repo.existing_landmark {
        migration_notes.push(
            "Landmark-like workflow or manifest already exists; inspect before replacing".into(),
        );
        ("ready", "manifest-only", "", 65u64)
    } else {
        match integration_mode.as_str() {
            "github-full" | "github-synthesis-only" | "generic-ci" | "local" => {
                ("ready", integration_mode.as_str(), "", rank_base)
            }
            "blocked" if workflow == "semantic-release" => (
                "blocked",
                "blocked",
                "existing semantic-release workflow requires explicit operator choice",
                rank_base,
            ),
            "backfill-first" => ("ready", "backfill-first", "", rank_base),
            _ => ("blocked", "blocked", "unknown release tooling", 10u64),
        }
    };
    if recommended_mode == "backfill-first" {
        migration_notes.push(
            "Create the operator-approved initial tag only after reviewing version policy; preview artifacts before enabling any release-body mutation".into(),
        );
        migration_notes.push(
            "Backfill-first adoption is manifest-only and local-artifacts first; do not add a GitHub release workflow in this PR".into(),
        );
    } else if repo.release_tool == "no-release-tool" {
        migration_notes
            .push("Choose release semantics before installing release automation".into());
    } else if !repo.existing_landmark && repo.release_tool != "unknown" {
        migration_notes.push(
            "Preview historical migration with `landmark backfill --repo-root . --since <oldest-managed-tag> --dry-run`; write artifacts before considering release-body updates".into(),
        );
    }
    if repo.package_topology.len() > 1 {
        risk_flags.push("multi-package repository; validate tag format and artifact paths".into());
    }
    migration_notes.push(format!(
        "Detected kind: {}; release surface: {}; release tool: {}; tag format: {}; default branch: {}",
        repository_kind, release_surface, repo.release_tool, repo.tag_format, repo.default_branch
    ));
    integration_rationale.push(format!("repository kind: {repository_kind}"));
    integration_rationale.push(format!("release surface: {release_surface}"));
    let manifest = fleet_manifest(repo, recommended_mode);
    let initial_version_recommendation = fleet_initial_version(recommended_mode, status);
    let initial_tag_recommendation = fleet_initial_tag(repo, &initial_version_recommendation);
    let artifact_paths = if recommended_mode == "backfill-first" && status == "ready" {
        fleet_manifest_artifact_paths(&manifest)
    } else {
        Vec::new()
    };
    let historical_preview_command = fleet_historical_preview_command(&initial_tag_recommendation);
    let rollback_guidance = fleet_rollback_guidance(recommended_mode);
    let workflow_patches = if status == "ready" {
        fleet_workflow_patches(repo, &manifest, recommended_mode, &workflow)
    } else {
        Vec::new()
    };
    FleetRepositoryPlan {
        repository: repo.name_with_owner.clone(),
        repository_kind,
        release_surface,
        rank,
        default_branch: repo.default_branch.clone(),
        recommended_mode: recommended_mode.into(),
        integration_mode: integration_mode.clone(),
        integration_rationale,
        workflow,
        status: status.into(),
        skip_reason: skip_reason.into(),
        risk_flags,
        required_secrets,
        missing_secrets,
        unavailable_secret_metadata,
        migration_notes,
        initial_version_recommendation,
        initial_tag_recommendation,
        artifact_paths,
        historical_preview_command,
        rollback_guidance,
        workflow_patches,
        manifest,
    }
}

fn fleet_workflow_patches(
    repo: &FleetRepository,
    manifest: &LandmarkManifest,
    recommended_mode: &str,
    workflow: &str,
) -> Vec<FleetWorkflowPatch> {
    if recommended_mode != "github-synthesis-only" {
        return Vec::new();
    }
    match workflow {
        "release-please" => fleet_workflow_file_for_tool(repo, "release-please")
            .map(|workflow_file| FleetWorkflowPatch {
                path: workflow_file.path.clone(),
                description: "update existing release-please workflow with Landmark synthesis job"
                    .into(),
                content: patch_release_please_workflow(
                    workflow_file,
                    &repo.default_branch,
                    manifest,
                ),
            })
            .into_iter()
            .collect(),
        "changesets" => fleet_workflow_file_for_tool(repo, "changesets")
            .map(|workflow_file| FleetWorkflowPatch {
                path: workflow_file.path.clone(),
                description: "update existing changesets workflow with Landmark synthesis job"
                    .into(),
                content: patch_changesets_workflow(
                    workflow_file,
                    &repo.default_branch,
                    repo.package_topology.len() > 1,
                    manifest,
                ),
            })
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn patch_release_please_workflow(
    workflow_file: &FleetWorkflowFile,
    branch: &str,
    manifest: &LandmarkManifest,
) -> String {
    if workflow_file.content.is_empty() {
        return workflow_release_please_for_job(branch, Some(manifest), &workflow_file.release_job);
    }
    let job = release_please_synthesis_job(&workflow_file.release_job, manifest);
    workflow_with_synthesis_job(&workflow_file.content, &job).unwrap_or_else(|| {
        workflow_release_please_for_job(branch, Some(manifest), &workflow_file.release_job)
    })
}

fn patch_changesets_workflow(
    workflow_file: &FleetWorkflowFile,
    branch: &str,
    monorepo: bool,
    manifest: &LandmarkManifest,
) -> String {
    if workflow_file.content.is_empty() {
        return workflow_changesets_for_job(
            branch,
            monorepo,
            Some(manifest),
            &workflow_file.release_job,
        );
    }
    let job = changesets_synthesis_job(&workflow_file.release_job, monorepo, manifest);
    workflow_with_synthesis_job(&workflow_file.content, &job).unwrap_or_else(|| {
        workflow_changesets_for_job(branch, monorepo, Some(manifest), &workflow_file.release_job)
    })
}

fn workflow_with_synthesis_job(content: &str, synthesis_job: &str) -> Option<String> {
    let mut workflow: serde_yaml::Value = serde_yaml::from_str(content).ok()?;
    let jobs_key = serde_yaml::Value::String("jobs".into());
    let synthesize_key = serde_yaml::Value::String("synthesize".into());
    let jobs = workflow
        .as_mapping_mut()?
        .get_mut(&jobs_key)?
        .as_mapping_mut()?;
    let synthesis: serde_yaml::Value = serde_yaml::from_str(synthesis_job).ok()?;
    let synthesis = synthesis.as_mapping()?.get(&synthesize_key)?.clone();
    jobs.insert(synthesize_key, synthesis);
    let mut rendered = serde_yaml::to_string(&workflow).ok()?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Some(rendered)
}

fn release_please_synthesis_job(release_job: &str, manifest: &LandmarkManifest) -> String {
    let manifest_inputs = render_manifest_action_inputs(Some(manifest), 8, Some("release-body"));
    format!(
        r#"synthesize:
  needs: {release_job}
  if: needs.{release_job}.outputs.release_created == 'true'
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
      with:
        fetch-depth: 0
    - uses: misty-step/landmark@v1
      with:
        mode: synthesis-only
        healthcheck: 'true'
        release-tag: ${{{{ needs.{release_job}.outputs.tag_name }}}}
        github-token: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
        llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
    )
}

fn changesets_synthesis_job(
    release_job: &str,
    monorepo: bool,
    manifest: &LandmarkManifest,
) -> String {
    let manifest_inputs = render_manifest_action_inputs(Some(manifest), 8, Some("release-body"));
    if monorepo {
        format!(
            r#"synthesize:
  needs: {release_job}
  if: needs.{release_job}.outputs.published == 'true'
  strategy:
    matrix:
      package: ${{{{ fromJson(needs.{release_job}.outputs.published_packages) }}}}
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
      with:
        fetch-depth: 0
    - uses: misty-step/landmark@v1
      with:
        mode: synthesis-only
        healthcheck: 'true'
        release-tag: ${{{{ matrix.package.name }}}}@${{{{ matrix.package.version }}}}
        github-token: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
        llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
        )
    } else {
        format!(
            r#"synthesize:
  needs: {release_job}
  if: needs.{release_job}.outputs.published == 'true'
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
      with:
        fetch-depth: 0
    - uses: misty-step/landmark@v1
      with:
        mode: synthesis-only
        healthcheck: 'true'
        release-tag: v${{{{ fromJson(needs.{release_job}.outputs.published_packages)[0].version }}}}
        github-token: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
        llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
        )
    }
}

fn fleet_manifest(repo: &FleetRepository, mode: &str) -> LandmarkManifest {
    LandmarkManifest {
        product: ProductManifest {
            name: Some(display_name_from_package(&repo.name)),
            description: Some(format!(
                "Release notes and changelog automation for {}.",
                repo.name_with_owner
            )),
        },
        audience: Some("developer".into()),
        voice: Some("clear, concrete, and specific to shipped behavior".into()),
        changelog: ChangelogManifest {
            source: Some("auto".into()),
        },
        artifacts: ArtifactManifest {
            markdown: Some("docs/releases/{version}.md".into()),
            plaintext: None,
            html: None,
            json: Some("docs/releases/releases.json".into()),
            rss: None,
        },
        release: ReleaseManifest {
            profile: Some(
                if mode == "full" || mode == "github-full" {
                    "full"
                } else {
                    "synthesis-only"
                }
                .into(),
            ),
        },
        model: ModelManifest {
            policy: Some("balanced".into()),
            primary: None,
            fallbacks: Vec::new(),
        },
        budget: BudgetManifest {
            max_input_tokens: Some(12000),
            max_output_tokens: Some(1200),
            max_usd: None,
        },
    }
}

fn render_fleet_plan_markdown(plan: &FleetPlan) -> String {
    let mut out = String::from("# Landmark Fleet Adoption Plan\n\n");
    out.push_str("## Summary\n\n");
    for (status, count) in &plan.summary {
        out.push_str(&format!("- {status}: {count}\n"));
    }
    out.push_str("\n## Repositories\n\n");
    for repo in &plan.repositories {
        out.push_str(&format!(
            "### {}\n\n- Rank: {}\n- Status: {}\n- Repository kind: {}\n- Release surface: {}\n- Integration mode: {}\n- Workflow: {}\n",
            repo.repository,
            repo.rank,
            repo.status,
            repo.repository_kind,
            repo.release_surface,
            repo.integration_mode,
            repo.workflow
        ));
        if !repo.skip_reason.is_empty() {
            out.push_str(&format!("- Skip reason: {}\n", repo.skip_reason));
        }
        if !repo.required_secrets.is_empty() {
            out.push_str(&format!(
                "- Required secrets: {}\n",
                repo.required_secrets.join(", ")
            ));
        }
        if !repo.risk_flags.is_empty() {
            out.push_str(&format!("- Risk flags: {}\n", repo.risk_flags.join("; ")));
        }
        if !repo.integration_rationale.is_empty() {
            out.push_str(&format!(
                "- Rationale: {}\n",
                repo.integration_rationale.join("; ")
            ));
        }
        if !repo.workflow_patches.is_empty() {
            out.push_str(&format!(
                "- Workflow patches: {}\n",
                repo.workflow_patches
                    .iter()
                    .map(|patch| patch.path.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !repo.initial_version_recommendation.is_empty() {
            out.push_str(&format!(
                "- Initial version recommendation: `{}`\n",
                repo.initial_version_recommendation
            ));
        }
        if !repo.initial_tag_recommendation.is_empty() {
            out.push_str(&format!(
                "- Initial tag recommendation: `{}`\n",
                repo.initial_tag_recommendation
            ));
        }
        if !repo.artifact_paths.is_empty() {
            out.push_str(&format!(
                "- Artifact paths: {}\n",
                repo.artifact_paths.join(", ")
            ));
        }
        if !repo.historical_preview_command.is_empty() {
            out.push_str(&format!(
                "- Historical preview command: `{}`\n",
                repo.historical_preview_command
            ));
        }
        if !repo.rollback_guidance.is_empty() {
            out.push_str(&format!("- Rollback: {}\n", repo.rollback_guidance));
        }
        out.push('\n');
    }
    let blocked_or_skipped = plan
        .repositories
        .iter()
        .filter(|repo| matches!(repo.status.as_str(), "blocked" | "skipped"))
        .collect::<Vec<_>>();
    if !blocked_or_skipped.is_empty() {
        out.push_str("## Blocked And Skipped Repositories\n\n");
        for repo in blocked_or_skipped {
            let reason = if repo.skip_reason.is_empty() {
                repo.status.as_str()
            } else {
                repo.skip_reason.as_str()
            };
            out.push_str(&format!(
                "- {}: {} ({})\n",
                repo.repository, reason, repo.integration_mode
            ));
        }
        out.push('\n');
    }
    out
}

fn fleet_workflow_for_plan(repo: &FleetRepositoryPlan) -> String {
    let diagnosis = SetupDiagnosis {
        release_tool: repo.workflow.clone(),
        default_branch: repo.default_branch.clone(),
        tag_format: "v{version}".into(),
        conventional_commits: "unknown: fleet plan generated without local git history".into(),
        monorepo: repo
            .risk_flags
            .iter()
            .any(|flag| flag.contains("multi-package")),
        packages: Vec::new(),
        signals: repo.migration_notes.clone(),
    };
    let workflows = setup_workflows(&diagnosis, Some(&repo.manifest));
    let preferred = match repo.workflow.as_str() {
        "semantic-release" => "semantic-release",
        "release-please" => "release-please",
        "changesets" => {
            if diagnosis.monorepo {
                "changesets-monorepo"
            } else {
                "changesets"
            }
        }
        _ => "manual-tag",
    };
    workflows
        .get(preferred)
        .or_else(|| workflows.values().next())
        .map(|candidate| candidate.content.clone())
        .unwrap_or_else(|| workflow_manual_tag(Some(&repo.manifest)))
}

fn fleet_pr_should_write_workflow(repo: &FleetRepositoryPlan) -> bool {
    if !repo.workflow_patches.is_empty() {
        return false;
    }
    matches!(
        repo.integration_mode.as_str(),
        "github-full" | "github-synthesis-only"
    ) && repo.recommended_mode != "manifest-only"
}

fn render_fleet_apply_markdown(
    repo: &FleetRepositoryPlan,
    branch: &str,
    title: &str,
    commit_message: &str,
    files: &[String],
) -> String {
    let mut out = format!(
        "# Apply Landmark Fleet PR\n\nRepository: `{}`\nBranch: `{}`\nBase: `{}`\nTitle: `{}`\n\n",
        repo.repository, branch, repo.default_branch, title
    );
    out.push_str(
        "Run these commands from a disposable directory after inspecting `diff.md`. They intentionally do not print secret values.\n\n",
    );
    out.push_str("```bash\n");
    out.push_str(&format!(
        "gh repo clone {} repo\n",
        shell_quote(&repo.repository)
    ));
    out.push_str("cd repo\n");
    out.push_str(&format!(
        "git checkout -b {} origin/{}\n",
        shell_quote(branch),
        shell_quote(&repo.default_branch)
    ));
    for file in files.iter().filter(|file| file.as_str() != "diff.md") {
        out.push_str(&format!("# copy rendered `{file}` into this checkout\n"));
    }
    let add_files = files
        .iter()
        .filter(|file| !matches!(file.as_str(), "diff.md" | "APPLY.md"))
        .map(|file| shell_quote(file))
        .collect::<Vec<_>>()
        .join(" ");
    out.push_str(&format!("git add {add_files}\n"));
    out.push_str(&format!("git commit -m {}\n", shell_quote(commit_message)));
    out.push_str(&format!("git push -u origin {}\n", shell_quote(branch)));
    out.push_str(&format!(
        "gh pr create --repo {} --base {} --head {} --title {} --body 'Adopt Landmark using the reviewed fleet rollout receipt. Merge this PR, monitor the downstream release run, then continue the fleet rollout.'\n",
        shell_quote(&repo.repository),
        shell_quote(&repo.default_branch),
        shell_quote(branch),
        shell_quote(title)
    ));
    out.push_str("gh pr checks --watch\n");
    out.push_str("```\n\n");
    if !repo.initial_version_recommendation.is_empty() {
        out.push_str(&format!(
            "Initial version recommendation: `{}`\n\n",
            repo.initial_version_recommendation
        ));
    }
    if !repo.initial_tag_recommendation.is_empty() {
        out.push_str(&format!(
            "Initial tag recommendation: `{}`\n\n",
            repo.initial_tag_recommendation
        ));
    }
    if !repo.historical_preview_command.is_empty() {
        out.push_str(&format!(
            "Preview command: `{}`\n\n",
            repo.historical_preview_command
        ));
    }
    if repo.rollback_guidance.is_empty() {
        out.push_str(
            "Rollback: close the PR and delete the remote branch before continuing the fleet.\n",
        );
    } else {
        out.push_str(&format!("Rollback: {}\n", repo.rollback_guidance));
    }
    out
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn render_fleet_pr_diff(
    repo: &FleetRepositoryPlan,
    manifest: &str,
    workflow_files: &[(String, String)],
) -> String {
    let mut out = format!(
        "# {}\n\nDry-run branch: `landmark/adopt-{}`\n\n## Files\n\n### .landmark.yml\n\n```yaml\n{}\n```\n\n",
        repo.repository,
        repo.repository.replace('/', "-"),
        manifest
    );
    for (path, workflow) in workflow_files {
        out.push_str(&format!("### {}\n\n```yaml\n{}\n```\n\n", path, workflow));
    }
    if !repo.initial_version_recommendation.is_empty()
        || !repo.initial_tag_recommendation.is_empty()
        || !repo.artifact_paths.is_empty()
        || !repo.historical_preview_command.is_empty()
        || !repo.rollback_guidance.is_empty()
    {
        out.push_str("## Operator Guidance\n\n");
        if !repo.initial_version_recommendation.is_empty() {
            out.push_str(&format!(
                "- Initial version recommendation: `{}`\n",
                repo.initial_version_recommendation
            ));
        }
        if !repo.initial_tag_recommendation.is_empty() {
            out.push_str(&format!(
                "- Initial tag recommendation: `{}`\n",
                repo.initial_tag_recommendation
            ));
        }
        if !repo.artifact_paths.is_empty() {
            out.push_str(&format!(
                "- Artifact paths: {}\n",
                repo.artifact_paths.join(", ")
            ));
        }
        if !repo.historical_preview_command.is_empty() {
            out.push_str(&format!(
                "- Historical preview command: `{}`\n",
                repo.historical_preview_command
            ));
        }
        if !repo.rollback_guidance.is_empty() {
            out.push_str(&format!("- Rollback: {}\n", repo.rollback_guidance));
        }
        out.push('\n');
    }
    out.push_str(&format!(
        "## Notes\n\n{}\n",
        repo.migration_notes
            .iter()
            .map(|note| format!("- {note}"))
            .collect::<Vec<_>>()
            .join("\n")
    ));
    out
}

fn diagnose_setup(root: &Path) -> SetupDiagnosis {
    let mut signals = Vec::new();
    let package = read_package_json(root);
    let release_tool = detect_release_tool(root, package.as_ref(), &mut signals);
    let packages = detect_packages(root, package.as_ref(), &mut signals);
    let monorepo = packages.len() > 1 || root.join(".changeset").is_dir();
    if root.join(".changeset").is_dir() {
        signals.push(".changeset directory present".into());
    }
    SetupDiagnosis {
        release_tool,
        default_branch: detect_default_branch(root),
        tag_format: detect_tag_format(root, &packages),
        conventional_commits: diagnose_conventional_commits(root),
        monorepo,
        packages,
        signals,
    }
}

fn read_package_json(root: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(root.join("package.json")).ok()?).ok()
}

fn detect_release_tool(root: &Path, package: Option<&Value>, signals: &mut Vec<String>) -> String {
    let workflows = read_dir_text(root.join(".github/workflows"));
    if workflows.contains("googleapis/release-please-action")
        || root.join("release-please-config.json").is_file()
    {
        signals.push("release-please workflow or config present".into());
        return "release-please".into();
    }
    if root.join(".changeset").is_dir() || workflows.contains("changesets/action") {
        signals.push("changesets workflow or .changeset directory present".into());
        return "changesets".into();
    }
    if root.join(".releaserc").is_file()
        || root.join(".releaserc.json").is_file()
        || package_has_dependency(package, "semantic-release")
    {
        signals.push("semantic-release config or dependency present".into());
        return "semantic-release".into();
    }
    if workflows.contains("gh release create") {
        signals.push("manual GitHub release command detected".into());
    }
    "manual-tag".into()
}

fn read_dir_text(path: PathBuf) -> String {
    let mut text = String::new();
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.path().is_file()
                && let Ok(file_text) = fs::read_to_string(entry.path())
            {
                text.push_str(&file_text);
                text.push('\n');
            }
        }
    }
    text
}

fn package_has_dependency(package: Option<&Value>, name: &str) -> bool {
    let Some(package) = package else {
        return false;
    };
    ["dependencies", "devDependencies"]
        .iter()
        .any(|key| package[*key].get(name).is_some())
}

fn detect_packages(root: &Path, package: Option<&Value>, signals: &mut Vec<String>) -> Vec<String> {
    let mut packages = Vec::new();
    if let Some(name) = package.and_then(|value| value["name"].as_str()) {
        packages.push(name.to_string());
    }
    if let Some(workspaces) = package.and_then(|value| value["workspaces"].as_array()) {
        signals.push("package.json workspaces present".into());
        packages.extend(
            workspaces
                .iter()
                .filter_map(|value| value.as_str())
                .map(str::to_string),
        );
    }
    for manifest in ["Cargo.toml", "pyproject.toml", "go.mod"] {
        if root.join(manifest).is_file() {
            signals.push(format!("{manifest} present"));
        }
    }
    packages.sort();
    packages.dedup();
    packages
}

fn detect_default_branch(root: &Path) -> String {
    if let Ok(output) = run_ok(
        "git",
        ["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
        root,
    ) && let Some(branch) = output.trim().strip_prefix("origin/")
    {
        return branch.to_string();
    }
    for branch in ["main", "master"] {
        if run_ok("git", ["rev-parse", "--verify", branch], root).is_ok() {
            return branch.to_string();
        }
    }
    "main".into()
}

fn detect_tag_format(root: &Path, packages: &[String]) -> String {
    let tags = run_ok("git", ["tag", "--list"], root).unwrap_or_default();
    let package_re = Regex::new(r"^[A-Za-z0-9_.-]+@v?[0-9]+\.[0-9]+\.[0-9]+").unwrap();
    let v_re = Regex::new(r"^v[0-9]+\.[0-9]+\.[0-9]+").unwrap();
    let bare_re = Regex::new(r"^[0-9]+\.[0-9]+\.[0-9]+").unwrap();
    let package_tag = tags.lines().any(|tag| package_re.is_match(tag));
    let v_tag = tags.lines().any(|tag| v_re.is_match(tag));
    let bare_tag = tags.lines().any(|tag| bare_re.is_match(tag));
    if package_tag || packages.len() > 1 {
        "package@{version}".into()
    } else if v_tag || !bare_tag {
        "v{version}".into()
    } else {
        "{version}".into()
    }
}

fn diagnose_conventional_commits(root: &Path) -> String {
    let log = run_ok("git", ["log", "-n", "30", "--pretty=%s"], root).unwrap_or_default();
    let subjects: Vec<_> = log.lines().collect();
    if subjects.is_empty() {
        return "unknown: no git history visible".into();
    }
    let conventional =
        Regex::new(r"^(feat|fix|docs|chore|refactor|test|ci|build|perf)(\(.+\))?!?: ").unwrap();
    let matches = subjects
        .iter()
        .filter(|subject| conventional.is_match(subject))
        .count();
    if matches * 2 >= subjects.len() {
        format!(
            "ready: {matches}/{} recent commits look conventional",
            subjects.len()
        )
    } else {
        format!(
            "needs-review: {matches}/{} recent commits look conventional",
            subjects.len()
        )
    }
}

fn recommend_setup(
    diagnosis: &SetupDiagnosis,
    manifest: Option<&LandmarkManifest>,
) -> SetupRecommendation {
    let workflow = match diagnosis.release_tool.as_str() {
        "semantic-release" => "semantic-release",
        "release-please" => "release-please",
        "changesets" if diagnosis.monorepo => "changesets-monorepo",
        "changesets" => "changesets",
        _ => "manual-tag",
    };
    let manifest_profile = manifest
        .and_then(|manifest| manifest.release.profile.as_deref())
        .and_then(trimmed_option);
    let mode = if let Some(profile) = manifest_profile.as_deref() {
        profile
    } else if workflow == "semantic-release" {
        "full"
    } else {
        "synthesis-only"
    };
    let mut rationale = vec![format!("detected release tool: {}", diagnosis.release_tool)];
    rationale.push(format!("default branch: {}", diagnosis.default_branch));
    rationale.push(format!("tag format: {}", diagnosis.tag_format));
    if let Some(profile) = manifest_profile.as_deref() {
        rationale.push(format!("manifest release profile: {profile}"));
    }
    if diagnosis.monorepo {
        rationale.push("monorepo outputs enabled".into());
    }
    SetupRecommendation {
        mode: mode.into(),
        workflow: workflow.into(),
        rationale,
    }
}

fn setup_workflows(
    diagnosis: &SetupDiagnosis,
    manifest: Option<&LandmarkManifest>,
) -> BTreeMap<String, WorkflowCandidate> {
    let branch = &diagnosis.default_branch;
    let mut workflows = BTreeMap::new();
    for (name, tool, mode, content) in [
        (
            "semantic-release",
            "semantic-release",
            "full",
            workflow_semantic_release(branch, manifest),
        ),
        (
            "release-please",
            "release-please",
            "synthesis-only",
            workflow_release_please(branch, manifest),
        ),
        (
            "changesets",
            "changesets",
            "synthesis-only",
            workflow_changesets(branch, false, manifest),
        ),
        (
            "changesets-monorepo",
            "changesets",
            "synthesis-only",
            workflow_changesets(branch, true, manifest),
        ),
        (
            "manual-tag",
            "manual-tag",
            "synthesis-only",
            workflow_manual_tag(manifest),
        ),
    ] {
        workflows.insert(
            name.to_string(),
            WorkflowCandidate {
                path: format!(".github/workflows/landmark-{name}.yml"),
                release_tool: tool.into(),
                mode: mode.into(),
                rationale: vec![
                    "includes Landmark healthcheck before the release attempt".into(),
                    "declares contents/issues/pull-requests write permissions".into(),
                ],
                content,
            },
        );
    }
    workflows
}

fn workflow_semantic_release(branch: &str, manifest: Option<&LandmarkManifest>) -> String {
    let manifest_inputs = render_manifest_action_inputs(manifest, 10, None);
    format!(
        r#"name: Release

on:
  push:
    branches: [{branch}]
  workflow_dispatch:

permissions:
  contents: write
  issues: write
  pull-requests: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
          persist-credentials: false
      - uses: misty-step/landmark@v1
        with:
          mode: full
          healthcheck: 'true'
          github-token: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
          llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
    )
}

fn workflow_release_please(branch: &str, manifest: Option<&LandmarkManifest>) -> String {
    workflow_release_please_for_job(branch, manifest, "release-please")
}

fn workflow_release_please_for_job(
    branch: &str,
    manifest: Option<&LandmarkManifest>,
    release_job: &str,
) -> String {
    let manifest_inputs = render_manifest_action_inputs(manifest, 10, Some("release-body"));
    format!(
        r#"name: Release

on:
  push:
    branches: [{branch}]

permissions:
  contents: write
  issues: write
  pull-requests: write

jobs:
  {release_job}:
    runs-on: ubuntu-latest
    outputs:
      release_created: ${{{{ steps.release.outputs.release_created }}}}
      tag_name: ${{{{ steps.release.outputs.tag_name }}}}
    steps:
      - uses: googleapis/release-please-action@v4
        id: release

  synthesize:
    needs: {release_job}
    if: needs.{release_job}.outputs.release_created == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: misty-step/landmark@v1
        with:
          mode: synthesis-only
          healthcheck: 'true'
          release-tag: ${{{{ needs.{release_job}.outputs.tag_name }}}}
          github-token: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
          llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
    )
}

fn workflow_changesets(
    branch: &str,
    monorepo: bool,
    manifest: Option<&LandmarkManifest>,
) -> String {
    workflow_changesets_for_job(branch, monorepo, manifest, "release")
}

fn workflow_changesets_for_job(
    branch: &str,
    monorepo: bool,
    manifest: Option<&LandmarkManifest>,
    release_job: &str,
) -> String {
    let manifest_inputs = render_manifest_action_inputs(manifest, 10, Some("release-body"));
    if monorepo {
        format!(
            r#"name: Release

on:
  push:
    branches: [{branch}]

permissions:
  contents: write
  issues: write
  pull-requests: write

jobs:
  {release_job}:
    runs-on: ubuntu-latest
    outputs:
      published: ${{{{ steps.changesets.outputs.published }}}}
      published_packages: ${{{{ steps.changesets.outputs.publishedPackages }}}}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 24
      - run: npm ci
      - uses: changesets/action@v1
        id: changesets
        with:
          publish: npm run release
        env:
          GITHUB_TOKEN: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
          NPM_TOKEN: ${{{{ secrets.NPM_TOKEN }}}}

  synthesize:
    needs: {release_job}
    if: needs.{release_job}.outputs.published == 'true'
    strategy:
      matrix:
        package: ${{{{ fromJson(needs.{release_job}.outputs.published_packages) }}}}
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: misty-step/landmark@v1
        with:
          mode: synthesis-only
          healthcheck: 'true'
          release-tag: ${{{{ matrix.package.name }}}}@${{{{ matrix.package.version }}}}
          github-token: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
          llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
        )
    } else {
        format!(
            r#"name: Release

on:
  push:
    branches: [{branch}]

permissions:
  contents: write
  issues: write
  pull-requests: write

jobs:
  {release_job}:
    runs-on: ubuntu-latest
    outputs:
      published: ${{{{ steps.changesets.outputs.published }}}}
      published_packages: ${{{{ steps.changesets.outputs.publishedPackages }}}}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 24
      - run: npm ci
      - uses: changesets/action@v1
        id: changesets
        with:
          publish: npm run release
        env:
          GITHUB_TOKEN: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
          NPM_TOKEN: ${{{{ secrets.NPM_TOKEN }}}}

  synthesize:
    needs: {release_job}
    if: needs.{release_job}.outputs.published == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: misty-step/landmark@v1
        with:
          mode: synthesis-only
          healthcheck: 'true'
          release-tag: v${{{{ fromJson(needs.{release_job}.outputs.published_packages)[0].version }}}}
          github-token: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
          llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
        )
    }
}

fn workflow_manual_tag(manifest: Option<&LandmarkManifest>) -> String {
    let manifest_inputs = render_manifest_action_inputs(manifest, 10, Some("auto"));
    format!(
        r#"name: Synthesize Release Notes

on:
  release:
    types: [published]

permissions:
  contents: write
  issues: write
  pull-requests: write

jobs:
  synthesize:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: misty-step/landmark@v1
        with:
          mode: synthesis-only
          healthcheck: 'true'
          release-tag: ${{{{ github.event.release.tag_name }}}}
          github-token: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
          llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
    )
}

fn render_manifest_action_inputs(
    manifest: Option<&LandmarkManifest>,
    indent: usize,
    default_changelog_source: Option<&str>,
) -> String {
    let mut lines = Vec::new();
    if let Some(manifest) = manifest {
        if let Some(value) = manifest
            .product
            .description
            .as_deref()
            .and_then(trimmed_option)
        {
            lines.push(("product-description", value));
        }
        if let Some(value) = manifest.audience.as_deref().and_then(trimmed_option) {
            lines.push(("audience", value));
        }
        if let Some(value) = manifest.voice.as_deref().and_then(trimmed_option) {
            lines.push(("voice-guide", value));
        }
        if let Some(value) = manifest
            .changelog
            .source
            .as_deref()
            .and_then(trimmed_option)
            .or_else(|| default_changelog_source.map(str::to_string))
        {
            lines.push(("changelog-source", value));
        }
        if let Some(value) = manifest
            .artifacts
            .markdown
            .as_deref()
            .and_then(trimmed_option)
        {
            lines.push(("notes-output-file", value));
        }
        if let Some(value) = manifest
            .artifacts
            .plaintext
            .as_deref()
            .and_then(trimmed_option)
        {
            lines.push(("notes-output-text-file", value));
        }
        if let Some(value) = manifest.artifacts.html.as_deref().and_then(trimmed_option) {
            lines.push(("notes-output-html-file", value));
        }
        if let Some(value) = manifest.artifacts.json.as_deref().and_then(trimmed_option) {
            lines.push(("notes-output-json", value));
        }
        if let Some(value) = manifest.artifacts.rss.as_deref().and_then(trimmed_option) {
            lines.push(("rss-feed-file", value));
        }
        if let Some(value) = manifest.model.primary.as_deref().and_then(trimmed_option) {
            lines.push(("llm-model", value));
        }
        if !manifest.model.fallbacks.is_empty() {
            lines.push(("llm-fallback-models", manifest.model.fallbacks.join(",")));
        }
    } else if let Some(value) = default_changelog_source {
        lines.push(("changelog-source", value.to_string()));
    }

    let padding = " ".repeat(indent);
    lines
        .into_iter()
        .map(|(key, value)| format!("{padding}{key}: {}", yaml_scalar(&value)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn yaml_scalar(value: &str) -> String {
    serde_yaml::to_string(value)
        .ok()
        .and_then(|rendered| {
            rendered
                .lines()
                .find(|line| !line.trim().is_empty() && !line.trim().starts_with("---"))
                .map(|line| line.trim().to_string())
        })
        .unwrap_or_else(|| format!("{value:?}"))
}

fn prepare_self_release(args: PrepareSelfReleaseArgs) -> Result<()> {
    validate_repo(&args.repository)?;
    let latest_version = latest_repo_version(&args.repo_root)?;
    let package_version = package_version(&args.repo_root)?;
    if semver_key(&package_version)? > semver_key(&latest_version)? {
        let plan = SelfReleasePlan {
            released: false,
            reason: format!(
                "metadata version {package_version} is ahead of latest tag {latest_version}; waiting for publish"
            ),
            latest_version,
            next_version: package_version,
            release_tag: String::new(),
            release_branch: args.release_branch,
            pull_request_title: String::new(),
            commit_message: String::new(),
            changed_files: Vec::new(),
            changelog: String::new(),
            commits: Vec::new(),
        };
        return emit_self_release_plan(&plan, &args.github_output);
    }

    let commits = self_release_commits(&args.repo_root, &format!("v{latest_version}"))?;
    let bump = release_bump(&commits);
    let Some(bump) = bump else {
        let plan = SelfReleasePlan {
            released: false,
            reason: "no release-worthy conventional commits since latest tag".into(),
            latest_version,
            next_version: package_version,
            release_tag: String::new(),
            release_branch: args.release_branch,
            pull_request_title: String::new(),
            commit_message: String::new(),
            changed_files: Vec::new(),
            changelog: String::new(),
            commits,
        };
        return emit_self_release_plan(&plan, &args.github_output);
    };

    let next_version = bump_version(&latest_version, bump)?;
    let release_tag = format!("v{next_version}");
    let changelog = render_self_release_changelog(
        &args.repository,
        &latest_version,
        &next_version,
        &release_tag,
        &commits,
    );
    prepend_changelog(&args.repo_root.join("CHANGELOG.md"), &changelog)?;
    update_version_metadata(UpdateVersionArgs {
        version: next_version.clone(),
        repo_root: args.repo_root.clone(),
    })?;
    update_lock_package_version(
        &args.repo_root.join("Cargo.lock"),
        "landmark",
        &next_version,
    )?;
    refresh_self_release_dist(&args.repo_root, &args.dist_target)?;

    let plan = SelfReleasePlan {
        released: true,
        reason: "prepared release pull request changes".into(),
        latest_version,
        next_version: next_version.clone(),
        release_tag,
        release_branch: args.release_branch,
        pull_request_title: format!("chore(release): {next_version}"),
        commit_message: format!("chore(release): {next_version}"),
        changed_files: vec![
            "CHANGELOG.md".into(),
            "package.json".into(),
            "crates/landmark/Cargo.toml".into(),
            "Cargo.lock".into(),
            "dist/landmark".into(),
            "dist/landmark.sha256".into(),
        ],
        changelog,
        commits,
    };
    emit_self_release_plan(&plan, &args.github_output)
}

fn refresh_self_release_dist(repo_root: &Path, target: &str) -> Result<()> {
    validate_nonblank(target, "dist-target")?;
    let binary = build_action_binary(repo_root, target)?;
    let dist_dir = repo_root.join("dist");
    fs::create_dir_all(&dist_dir)?;
    let dest = dist_dir.join("landmark");
    let temp = dist_dir.join(format!(
        ".landmark-{}-{}.tmp",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    fs::copy(&binary, &temp)?;
    fs::set_permissions(&temp, fs::metadata(&binary)?.permissions())?;
    fs::rename(&temp, &dest)?;

    let digest = hex::encode(Sha256::digest(fs::read(&dest)?));
    fs::write(
        dist_dir.join("landmark.sha256"),
        format!("{digest}  dist/landmark\n"),
    )?;
    Ok(())
}

fn build_action_binary(repo_root: &Path, target: &str) -> Result<PathBuf> {
    if target == LINUX_ACTION_TARGET && !rustc_host_target()?.contains("linux") {
        return Err(
            "refusing to build checked-in Linux action binary from a non-Linux host; run the release workflow or `bin/build-linux-action --write` so dist/landmark is produced in Linux, or pass --dist-target only for replay fixtures"
                .to_string()
        .into());
    }
    let output = Command::new("cargo")
        .args(["build", "--locked", "--release", "--target", target])
        .current_dir(repo_root)
        .output()
        .map_err(|error| {
            format!("failed to launch cargo for self-release binary build: {error}")
        })?;
    if !output.status.success() {
        return Err(format!(
            "failed to build Landmark self-release action binary for {target}; install the Rust target and linker for {target}, then retry: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let binary = repo_root
        .join("target")
        .join(target)
        .join("release")
        .join("landmark");
    if !binary.is_file() {
        return Err(format!(
            "cargo build completed but {} was not created",
            binary.display()
        )
        .into());
    }
    Ok(binary)
}

fn emit_self_release_plan(plan: &SelfReleasePlan, github_output: &str) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(plan)?);
    if !github_output.is_empty() {
        write_outputs(
            Path::new(github_output),
            &[
                ("released", plan.released.to_string()),
                ("reason", sanitize_text(&plan.reason)),
                ("release_tag", plan.release_tag.clone()),
                ("next_version", plan.next_version.clone()),
                ("release_branch", plan.release_branch.clone()),
                ("pull_request_title", plan.pull_request_title.clone()),
                ("commit_message", plan.commit_message.clone()),
            ],
        )?;
    }
    Ok(())
}

fn publish_self_release(args: PublishSelfReleaseArgs) -> Result<()> {
    validate_repo(&args.repository)?;
    validate_nonblank(&args.target_sha, "target-sha")?;
    let latest_version = latest_repo_version(&args.repo_root)?;
    let package_version = package_version(&args.repo_root)?;
    let cargo = cargo_version(&args.repo_root.join("crates/landmark/Cargo.toml"))
        .ok_or("crates/landmark/Cargo.toml missing package version")?;
    if cargo != package_version {
        return Err(format!(
            "package.json has {package_version}, crates/landmark/Cargo.toml has {cargo}"
        )
        .into());
    }
    if semver_key(&package_version)? <= semver_key(&latest_version)? {
        let publish = SelfReleasePublish {
            published: false,
            reason: "metadata is not ahead of latest release tag".into(),
            latest_version,
            version: package_version,
            release_tag: String::new(),
            release_url: String::new(),
        };
        return emit_self_release_publish(&publish, &args.github_output);
    }

    let release_tag = format!("v{package_version}");
    let provider = GitHubProvider::required(&args.api_base_url, &args.github_token);
    if let Some(value) = provider.release_by_tag(&args.repository, &release_tag)? {
        let publish = SelfReleasePublish {
            published: false,
            reason: "release already exists".into(),
            latest_version,
            version: package_version,
            release_tag,
            release_url: value["html_url"].as_str().unwrap_or("").to_string(),
        };
        return emit_self_release_publish(&publish, &args.github_output);
    }

    let body = changelog_section(&args.repo_root.join("CHANGELOG.md"), &package_version)?;
    let release_url =
        provider.create_release(&args.repository, &release_tag, &args.target_sha, &body)?;
    let publish = SelfReleasePublish {
        published: true,
        reason: "published release from landed release pull request".into(),
        latest_version,
        version: package_version,
        release_tag,
        release_url,
    };
    emit_self_release_publish(&publish, &args.github_output)
}

fn emit_self_release_publish(publish: &SelfReleasePublish, github_output: &str) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(publish)?);
    if !github_output.is_empty() {
        write_outputs(
            Path::new(github_output),
            &[
                ("published", publish.published.to_string()),
                ("reason", sanitize_text(&publish.reason)),
                ("release_tag", publish.release_tag.clone()),
                ("release_url", publish.release_url.clone()),
                ("version", publish.version.clone()),
            ],
        )?;
    }
    Ok(())
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
enum ReleaseBump {
    Patch,
    Minor,
    Major,
}

fn latest_repo_version(repo_root: &Path) -> Result<String> {
    let tags = run_ok("git", ["tag", "--merged", "HEAD"], repo_root)?;
    latest_semver_version(tags.lines()).ok_or("no semver tags found".into())
}

fn package_version(repo_root: &Path) -> Result<String> {
    let package: Value =
        serde_json::from_str(&fs::read_to_string(repo_root.join("package.json"))?)?;
    package["version"]
        .as_str()
        .map(str::to_string)
        .ok_or("package.json missing version".into())
}

fn self_release_commits(repo_root: &Path, tag: &str) -> Result<Vec<SelfReleaseCommit>> {
    let range = format!("{tag}..HEAD");
    let log = run_ok(
        "git",
        ["log", "--reverse", "--format=%H%x00%s%x00%b%x1e", &range],
        repo_root,
    )?;
    let mut commits = Vec::new();
    for record in log.split('\x1e') {
        let record = record.trim_matches('\n');
        if record.trim().is_empty() {
            continue;
        }
        let mut parts = record.splitn(3, '\0');
        let hash = parts.next().unwrap_or("").trim().to_string();
        let subject = parts.next().unwrap_or("").trim().to_string();
        let body = parts.next().unwrap_or("").trim().to_string();
        if subject.starts_with("chore(release):") {
            continue;
        }
        if let Some(commit) = classify_release_commit(&hash, &subject, &body) {
            commits.push(commit);
        }
    }
    Ok(commits)
}

fn classify_release_commit(hash: &str, subject: &str, body: &str) -> Option<SelfReleaseCommit> {
    let re = Regex::new(r"^([A-Za-z]+)(?:\(([^)]+)\))?(!)?: (.+)$").unwrap();
    let caps = re.captures(subject)?;
    let kind = caps.get(1)?.as_str().to_ascii_lowercase();
    let scope = caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
    let breaking = caps.get(3).is_some() || body.contains("BREAKING CHANGE:");
    let category = if breaking {
        "breaking"
    } else {
        match kind.as_str() {
            "feat" => "features",
            "fix" => "fixes",
            "perf" => "performance",
            _ => return None,
        }
    };
    Some(SelfReleaseCommit {
        hash: hash.to_string(),
        short_hash: hash.chars().take(7).collect(),
        subject: subject.to_string(),
        category: category.to_string(),
        scope,
        description: caps.get(4)?.as_str().to_string(),
        breaking,
    })
}

fn release_bump(commits: &[SelfReleaseCommit]) -> Option<ReleaseBump> {
    let mut bump: Option<ReleaseBump> = None;
    for commit in commits {
        let candidate = if commit.breaking {
            ReleaseBump::Major
        } else {
            match commit.category.as_str() {
                "features" => ReleaseBump::Minor,
                "fixes" | "performance" => ReleaseBump::Patch,
                _ => continue,
            }
        };
        bump = Some(bump.map_or(candidate, |current| current.max(candidate)));
    }
    bump
}

fn bump_version(version: &str, bump: ReleaseBump) -> Result<String> {
    let (major, minor, patch) = semver_key(version)?;
    Ok(match bump {
        ReleaseBump::Major => format!("{}.0.0", major + 1),
        ReleaseBump::Minor => format!("{major}.{}.0", minor + 1),
        ReleaseBump::Patch => format!("{major}.{minor}.{}", patch + 1),
    })
}

fn semver_key(version: &str) -> Result<(u64, u64, u64)> {
    let (_, normalized) = semver_from_tag(version).ok_or_else(|| {
        format!(
            "invalid semver version {}",
            version.trim().trim_start_matches('v')
        )
    })?;
    let mut parts = normalized.split('.');
    Ok((
        parts.next().unwrap_or("0").parse()?,
        parts.next().unwrap_or("0").parse()?,
        parts.next().unwrap_or("0").parse()?,
    ))
}

fn render_self_release_changelog(
    repository: &str,
    latest_version: &str,
    next_version: &str,
    release_tag: &str,
    commits: &[SelfReleaseCommit],
) -> String {
    let mut out = format!(
        "# [{next_version}](https://github.com/{repository}/compare/v{latest_version}...{release_tag}) ({})\n\n",
        Utc::now().format("%Y-%m-%d")
    );
    let sections = [
        ("breaking", "### BREAKING CHANGES"),
        ("features", "### Features"),
        ("fixes", "### Bug Fixes"),
        ("performance", "### Performance Improvements"),
    ];
    for (category, heading) in sections {
        let entries: Vec<_> = commits
            .iter()
            .filter(|commit| commit.category == category)
            .collect();
        if entries.is_empty() {
            continue;
        }
        out.push_str(heading);
        out.push_str("\n\n");
        for commit in entries {
            let scope = if commit.scope.is_empty() {
                String::new()
            } else {
                format!("**{}:** ", commit.scope)
            };
            out.push_str(&format!(
                "* {scope}{} ([{}](https://github.com/{repository}/commit/{}))\n",
                commit.description, commit.short_hash, commit.hash
            ));
        }
        out.push('\n');
    }
    out
}

fn prepend_changelog(path: &Path, entry: &str) -> Result<()> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    fs::write(
        path,
        format!("{}\n{}", entry.trim_end(), existing.trim_start()),
    )?;
    Ok(())
}

fn update_lock_package_version(path: &Path, package_name: &str, version: &str) -> Result<()> {
    let text = fs::read_to_string(path)?;
    let mut in_package = false;
    let mut saw_name = false;
    let mut replaced = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        if line.trim() == "[[package]]" {
            in_package = true;
            saw_name = false;
        } else if in_package && line.starts_with("name = ") {
            saw_name = line == format!("name = \"{package_name}\"");
        } else if in_package && saw_name && line.starts_with("version = ") {
            lines.push(format!("version = \"{version}\""));
            in_package = false;
            replaced = true;
            continue;
        }
        lines.push(line.to_string());
    }
    if !replaced {
        return Err(format!("Cargo.lock package {package_name} not found").into());
    }
    fs::write(path, lines.join("\n") + "\n")?;
    Ok(())
}

fn changelog_section(path: &Path, version: &str) -> Result<String> {
    let text = fs::read_to_string(path)?;
    let marker = format!("[{version}]");
    let mut started = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        if !started {
            if line.contains(&marker) {
                started = true;
                lines.push(line.to_string());
            }
            continue;
        }
        if line.starts_with('#') && line.contains('[') {
            break;
        }
        lines.push(line.to_string());
    }
    let section = lines.join("\n").trim().to_string();
    if section.is_empty() {
        Err(format!("CHANGELOG.md missing section for {version}").into())
    } else {
        Ok(section)
    }
}

fn release_candidate_changelog_exists(path: &Path, version: &str) -> bool {
    changelog_section(path, version)
        .map(|section| {
            section
                .lines()
                .map(str::trim)
                .any(|line| line.starts_with("* ") || line.starts_with("- "))
        })
        .unwrap_or(false)
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    body: String,
}

#[derive(Clone, Copy, Debug)]
struct HttpPolicy {
    connect_timeout_seconds: u64,
    max_time_seconds: u64,
    attempts: usize,
    retry_delay_ms: u64,
}

impl Default for HttpPolicy {
    fn default() -> Self {
        Self {
            connect_timeout_seconds: 5,
            max_time_seconds: 30,
            attempts: 3,
            retry_delay_ms: 250,
        }
    }
}

#[derive(Debug)]
struct CurlInvocation {
    args: Vec<String>,
    config: String,
}

fn curl_json(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<&Value>,
) -> Result<HttpResponse> {
    curl_json_with_policy(method, url, token, body, HttpPolicy::default())
}

fn curl_json_with_policy(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<&Value>,
    policy: HttpPolicy,
) -> Result<HttpResponse> {
    let attempts = policy.attempts.max(1);
    let mut last_error = String::new();
    for attempt in 1..=attempts {
        match curl_json_once(method, url, token, body, policy) {
            Ok(response) if !http_status_retryable(response.status) || attempt == attempts => {
                return Ok(response);
            }
            Ok(response) => {
                last_error = format!("HTTP {}", response.status);
            }
            Err(error) if attempt == attempts => return Err(error),
            Err(error) => {
                last_error = error.to_string();
            }
        }
        thread::sleep(Duration::from_millis(policy.retry_delay_ms));
    }
    Err(last_error.into())
}

fn curl_json_once(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<&Value>,
    policy: HttpPolicy,
) -> Result<HttpResponse> {
    let invocation = build_curl_invocation(method, url, token, body, policy);
    let mut child = Command::new("curl")
        .args(&invocation.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("failed to open curl stdin")?
        .write_all(invocation.config.as_bytes())?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(redact_known_secrets(&String::from_utf8_lossy(&output.stderr)).into());
    }
    let raw = String::from_utf8(output.stdout)?;
    let (body, status) = raw.rsplit_once('\n').ok_or("curl status marker missing")?;
    Ok(HttpResponse {
        status: status.parse()?,
        body: body.to_string(),
    })
}

fn build_curl_invocation(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<&Value>,
    policy: HttpPolicy,
) -> CurlInvocation {
    let args = vec![
        "-sS".to_string(),
        "-L".to_string(),
        "--connect-timeout".to_string(),
        policy.connect_timeout_seconds.to_string(),
        "--max-time".to_string(),
        policy.max_time_seconds.to_string(),
        "-K".to_string(),
        "-".to_string(),
    ];
    let mut config = String::new();
    push_curl_config(&mut config, "request", method);
    push_curl_config(&mut config, "header", "Accept: application/vnd.github+json");
    push_curl_config(&mut config, "header", "User-Agent: landmark");
    push_curl_config(&mut config, "write-out", "\n%{http_code}");
    push_curl_config(&mut config, "url", url);
    if let Some(token) = token {
        push_curl_config(
            &mut config,
            "header",
            &format!("Authorization: Bearer {token}"),
        );
    }
    if let Some(body) = body {
        push_curl_config(&mut config, "header", "Content-Type: application/json");
        push_curl_config(&mut config, "data", &body.to_string());
    }
    CurlInvocation { args, config }
}

fn push_curl_config(config: &mut String, key: &str, value: &str) {
    config.push_str(key);
    config.push_str(" = \"");
    config.push_str(&escape_curl_config_value(value));
    config.push_str("\"\n");
}

fn escape_curl_config_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn http_status_retryable(status: u16) -> bool {
    status == 408 || status == 425 || status == 429 || (500..600).contains(&status)
}

#[derive(Clone, Debug)]
struct GitHubProvider {
    api_base_url: String,
    token: Option<String>,
}

impl GitHubProvider {
    fn new(api_base_url: &str, token: Option<&str>) -> Self {
        Self {
            api_base_url: api_base_url.trim_end_matches('/').to_string(),
            token: token.map(str::to_string),
        }
    }

    fn required(api_base_url: &str, token: &str) -> Self {
        Self::new(api_base_url, Some(token))
    }

    fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    fn release_by_tag(&self, repository: &str, tag: &str) -> Result<Option<Value>> {
        validate_repo(repository)?;
        let response = curl_json(
            "GET",
            &self.release_by_tag_url(repository, tag),
            self.token(),
            None,
        )?;
        if response.status == 404 {
            return Ok(None);
        }
        if !(200..300).contains(&response.status) {
            return Err(
                format!("GitHub release fetch failed with HTTP {}", response.status).into(),
            );
        }
        Ok(Some(serde_json::from_str(&response.body)?))
    }

    fn update_release_body(&self, repository: &str, tag: &str, notes: &str) -> Result<String> {
        let release = self
            .release_by_tag(repository, tag)?
            .ok_or_else(|| format!("release {tag} not found"))?;
        let id = release["id"]
            .as_i64()
            .ok_or("release response missing id")?;
        let existing = release["body"].as_str().unwrap_or("");
        let update = curl_json(
            "PATCH",
            &self.release_by_id_url(repository, id),
            self.token(),
            Some(&json!({ "body": compose_release_body(notes, existing) })),
        )?;
        if !(200..300).contains(&update.status) {
            return Err(format!("GitHub release update failed with HTTP {}", update.status).into());
        }
        Ok(release["html_url"]
            .as_str()
            .unwrap_or(&format!(
                "https://github.com/{repository}/releases/tag/{tag}"
            ))
            .to_string())
    }

    fn create_release(
        &self,
        repository: &str,
        tag: &str,
        target_commitish: &str,
        body: &str,
    ) -> Result<String> {
        validate_repo(repository)?;
        let response = curl_json(
            "POST",
            &format!("{}/repos/{repository}/releases", self.api_base_url),
            self.token(),
            Some(&json!({
                "tag_name": tag,
                "target_commitish": target_commitish,
                "name": tag,
                "body": body,
                "draft": false,
                "prerelease": false
            })),
        )?;
        if !(200..300).contains(&response.status) {
            return Err(format!(
                "GitHub release creation failed with HTTP {}",
                response.status
            )
            .into());
        }
        let value: Value = serde_json::from_str(&response.body)?;
        Ok(value["html_url"].as_str().unwrap_or("").to_string())
    }

    fn closed_pull_requests(&self, repository: &str) -> Result<Vec<Value>> {
        validate_repo(repository)?;
        let response = curl_json(
            "GET",
            &format!(
                "{}/repos/{repository}/pulls?state=closed&per_page=100",
                self.api_base_url
            ),
            self.token(),
            None,
        )?;
        if !(200..300).contains(&response.status) {
            return Err(format!("GitHub PR fetch failed with HTTP {}", response.status).into());
        }
        Ok(serde_json::from_str(&response.body)?)
    }

    fn tree_paths(&self, repository: &str, branch: &str) -> Result<Vec<String>> {
        let output = run_gh_ok(
            vec![
                "api".into(),
                format!(
                    "repos/{repository}/git/trees/{}?recursive=1",
                    urlencoding::encode(branch)
                ),
                "--jq".into(),
                "[.tree[].path]".into(),
            ],
            self.token(),
        )?;
        Ok(serde_json::from_str(&output)?)
    }

    fn tags(&self, repository: &str) -> Result<Vec<String>> {
        let output = run_gh_ok(
            vec![
                "api".into(),
                format!("repos/{repository}/tags?per_page=30"),
                "--jq".into(),
                "[.[].name]".into(),
            ],
            self.token(),
        )?;
        Ok(serde_json::from_str(&output)?)
    }

    fn workflow_texts(
        &self,
        repository: &str,
        branch: &str,
        workflows: &[String],
    ) -> Vec<(String, String)> {
        workflows
            .iter()
            .filter_map(|workflow| {
                let output = run_gh_ok(
                    vec![
                        "api".into(),
                        format!(
                            "repos/{repository}/contents/.github/workflows/{}?ref={}",
                            urlencoding::encode(workflow),
                            urlencoding::encode(branch)
                        ),
                        "--header".into(),
                        "Accept: application/vnd.github.raw".into(),
                    ],
                    self.token(),
                )
                .ok()?;
                Some((workflow.clone(), output))
            })
            .collect()
    }

    fn branch_protection_status(&self, repository: &str, branch: &str) -> String {
        let Some(token) = self.token() else {
            return "unavailable: no GitHub token supplied".into();
        };
        let url = format!(
            "{}/repos/{repository}/branches/{}/protection",
            self.api_base_url,
            urlencoding::encode(branch)
        );
        match curl_json("GET", &url, Some(token), None) {
            Ok(response) if response.status == 200 => "protected".into(),
            Ok(response) if response.status == 404 => "unprotected-or-unavailable".into(),
            Ok(response) => format!("unavailable: HTTP {}", response.status),
            Err(error) => format!("unavailable: {error}"),
        }
    }

    fn secret_statuses(&self, repository: &str, required: &[&str]) -> Vec<FleetSecretStatus> {
        let Some(token) = self.token() else {
            return unavailable_secret_statuses(
                required,
                "secret metadata requires a GitHub token with repository access",
            );
        };
        let response = match self.secret_names(repository, token) {
            Ok(response) => response,
            Err(error) => return unavailable_secret_statuses(required, &error.to_string()),
        };
        required
            .iter()
            .map(|name| FleetSecretStatus {
                name: (*name).to_string(),
                status: if response.names.contains(*name) {
                    "present".into()
                } else if response.org_unavailable.is_some() {
                    "unavailable".into()
                } else {
                    "missing".into()
                },
                detail: if response.repo_names.contains(*name) {
                    "repo secret metadata only; value not read".into()
                } else if response.org_names.contains(*name) {
                    "org secret metadata only; value not read".into()
                } else if let Some(error) = &response.org_unavailable {
                    format!("org secret metadata unavailable: {error}")
                } else {
                    "required secret name is absent from Actions secret metadata".into()
                },
            })
            .collect()
    }

    fn secret_names(&self, repository: &str, token: &str) -> Result<FleetSecretNames> {
        let repo_names = self.repo_secret_names(repository, token)?;
        let (org_names, org_unavailable) = match self.org_secret_names(repository, token) {
            Ok(Some(names)) => (names, None),
            Ok(None) => (BTreeSet::new(), None),
            Err(error) => (BTreeSet::new(), Some(error.to_string())),
        };
        let names = repo_names.union(&org_names).cloned().collect();
        Ok(FleetSecretNames {
            names,
            repo_names,
            org_names,
            org_unavailable,
        })
    }

    fn repo_secret_names(&self, repository: &str, token: &str) -> Result<BTreeSet<String>> {
        let response = curl_json(
            "GET",
            &format!(
                "{}/repos/{repository}/actions/secrets?per_page=100",
                self.api_base_url
            ),
            Some(token),
            None,
        )
        .map_err(|error| format!("secret metadata unavailable: {error}"))?;
        if !(200..300).contains(&response.status) {
            return Err(format!(
                "GitHub returned HTTP {} for secret metadata",
                response.status
            )
            .into());
        }
        let value: Value = serde_json::from_str(&response.body)
            .map_err(|error| format!("secret metadata parse failed: {error}"))?;
        Ok(secret_names_from_array(&value))
    }

    fn org_secret_names(&self, repository: &str, token: &str) -> Result<Option<BTreeSet<String>>> {
        let (owner, repo_name) = repository
            .split_once('/')
            .ok_or_else(|| format!("invalid repository {repository}"))?;
        let response = curl_json(
            "GET",
            &format!(
                "{}/orgs/{owner}/actions/secrets?per_page=100",
                self.api_base_url
            ),
            Some(token),
            None,
        )?;
        if response.status == 404 {
            return Ok(None);
        }
        if !(200..300).contains(&response.status) {
            return Err(format!(
                "GitHub returned HTTP {} for org secret metadata",
                response.status
            )
            .into());
        }
        let value: Value = serde_json::from_str(&response.body)?;
        Ok(Some(org_secret_names_for_repo(
            &value, repository, repo_name,
        )))
    }

    fn find_failure_issues(&self, repository: &str, release_tag: &str) -> Result<Vec<Value>> {
        validate_repo(repository)?;
        let response = curl_json(
            "GET",
            &format!(
                "{}/repos/{repository}/issues?state=open&labels=landmark,release-notes&per_page=100",
                self.api_base_url
            ),
            self.token(),
            None,
        )?;
        if !(200..300).contains(&response.status) {
            return Err(format!("issue search failed with HTTP {}", response.status).into());
        }
        let issues: Vec<Value> = serde_json::from_str(&response.body)?;
        let title = failure_issue_title(release_tag);
        Ok(issues
            .into_iter()
            .filter(|issue| issue["title"].as_str() == Some(&title))
            .collect())
    }

    fn create_failure_issue(&self, repository: &str, title: &str, body: &str) -> Result<()> {
        let response = curl_json(
            "POST",
            &format!("{}/repos/{repository}/issues", self.api_base_url),
            self.token(),
            Some(&json!({"title": title, "body": body, "labels": ["landmark", "release-notes"]})),
        )?;
        if (200..300).contains(&response.status) {
            Ok(())
        } else {
            Err(format!("issue creation failed with HTTP {}", response.status).into())
        }
    }

    fn comment_issue(&self, repository: &str, number: i64, body: &str) -> Result<()> {
        let _ = curl_json(
            "POST",
            &format!(
                "{}/repos/{repository}/issues/{number}/comments",
                self.api_base_url
            ),
            self.token(),
            Some(&json!({"body": body})),
        )?;
        Ok(())
    }

    fn close_issue(&self, repository: &str, number: i64) -> Result<()> {
        let _ = curl_json(
            "PATCH",
            &format!("{}/repos/{repository}/issues/{number}", self.api_base_url),
            self.token(),
            Some(&json!({"state": "closed"})),
        )?;
        Ok(())
    }

    fn release_by_tag_url(&self, repository: &str, tag: &str) -> String {
        format!(
            "{}/repos/{}/releases/tags/{}",
            self.api_base_url,
            repository,
            urlencoding::encode(tag)
        )
    }

    fn release_by_id_url(&self, repository: &str, id: i64) -> String {
        format!("{}/repos/{repository}/releases/{id}", self.api_base_url)
    }
}

fn healthcheck(args: HealthcheckArgs) -> Result<()> {
    let payload = json!({
        "model": args.model,
        "messages": [{"role": "user", "content": "Reply with ok."}],
        "max_tokens": 8
    });
    match curl_json("POST", &args.api_url, Some(&args.api_key), Some(&payload)) {
        Ok(response) if (200..300).contains(&response.status) => {
            let value: Value = serde_json::from_str(&response.body)?;
            let content = value["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .trim();
            if content.is_empty() {
                return healthcheck_fail(args.warn_only, "LLM healthcheck returned empty content");
            }
            Ok(())
        }
        Ok(response) => healthcheck_fail(
            args.warn_only,
            &format!("LLM healthcheck failed with HTTP {}", response.status),
        ),
        Err(error) => healthcheck_fail(
            args.warn_only,
            &format!("LLM healthcheck request failed: {error}"),
        ),
    }
}

fn healthcheck_fail(warn_only: bool, message: &str) -> Result<()> {
    if warn_only {
        eprintln!("::warning::{message}");
        Ok(())
    } else {
        Err(message.to_string().into())
    }
}

fn preflight_tags() -> Result<()> {
    let tags = run_ok("git", ["tag", "--list", "v*"], Path::new("."))?;
    let orphaned: Vec<_> = tags
        .lines()
        .filter(|tag| semver_from_tag(tag).is_some())
        .filter(|tag| {
            let status = Command::new("git")
                .args(["rev-list", "-n", "1", tag])
                .status()
                .map(|status| status.success())
                .unwrap_or(false);
            !status
        })
        .collect();
    if orphaned.is_empty() {
        Ok(())
    } else {
        Err(format!("orphaned release tags: {}", orphaned.join(", ")).into())
    }
}

fn fetch_release_body(args: FetchReleaseBodyArgs) -> Result<()> {
    let provider = GitHubProvider::required(&args.api_base_url, &args.github_token);
    let value = provider.release_by_tag(&args.repository, &args.release_tag)?;
    ensure_parent(&args.output_file)?;
    fs::write(
        args.output_file,
        value
            .as_ref()
            .and_then(|release| release["body"].as_str())
            .unwrap_or(""),
    )?;
    Ok(())
}

fn extract_prs(args: ExtractPrsArgs) -> Result<()> {
    let provider = GitHubProvider::required(&args.api_base_url, &args.github_token);
    let prs = provider.closed_pull_requests(&args.repository)?;
    let mut rendered = String::new();
    for pr in prs.iter().filter(|pr| !pr["merged_at"].is_null()) {
        let number = pr["number"].as_i64().unwrap_or_default();
        let title = pr["title"].as_str().unwrap_or("Untitled");
        let user = pr["user"]["login"].as_str().unwrap_or("unknown");
        rendered.push_str(&format!("- {title} (#{number}) by @{user}\n"));
    }
    if rendered.is_empty() {
        rendered.push_str(&format!("Release {}\n", args.release_tag));
    }
    ensure_parent(&args.output_file)?;
    fs::write(args.output_file, rendered)?;
    Ok(())
}

fn synthesize(args: SynthesizeArgs) -> Result<()> {
    let config = resolve_synthesis_config(&args)?;
    let technical = resolve_technical_changelog(&args, &config)?;
    let prompt = render_prompt(&args, &config, &technical)?;
    let context = synthesis_context_packet(&args, &config, &technical, &prompt);
    write_json_if_requested(&args.context_metadata_file, &context)?;
    if args.dry_run_cost {
        println!("{}", serde_json::to_string_pretty(&context)?);
        return Ok(());
    }
    if context.cost.skip {
        write_json_if_requested(
            &args.attempts_file,
            &vec![json!({
                "model": context.cost.model,
                "succeeded": false,
                "quality": "skipped",
                "message": context.cost.skip_reason,
                "cost": context.cost.clone(),
                "classification": context.classification.clone(),
                "decision": context.decision.clone(),
            })],
        )?;
        ensure_parent(&args.quality_file)?;
        fs::write(&args.quality_file, "skipped")?;
        return Ok(());
    }
    validate_nonblank(&args.api_key, "api-key")?;
    validate_nonblank(&context.cost.model, "model")?;
    let mut models = vec![context.cost.model.clone()];
    models.extend(
        config
            .fallback_models
            .split(',')
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .map(str::to_string),
    );
    let mut last_error = String::new();
    let mut attempts = Vec::new();
    for model in models {
        match request_synthesis(&args.api_url, &args.api_key, &model, &prompt) {
            Ok(notes) if !notes.trim().is_empty() => {
                let quality = if validate_notes(&notes) {
                    "valid"
                } else {
                    "degraded"
                };
                attempts.push(json!({
                    "model": model,
                    "succeeded": true,
                    "quality": quality,
                    "message": "",
                    "cost": context.cost.clone(),
                    "classification": context.classification.clone(),
                    "decision": context.decision.clone(),
                }));
                write_json_if_requested(&args.attempts_file, &attempts)?;
                ensure_parent(&args.quality_file)?;
                fs::write(&args.quality_file, quality)?;
                println!("{}", notes.trim());
                return Ok(());
            }
            Ok(_) => {
                last_error = format!("model {model} returned empty content");
                attempts.push(json!({
                    "model": model,
                    "succeeded": false,
                    "quality": "failed",
                    "message": last_error,
                    "cost": context.cost.clone(),
                    "classification": context.classification.clone(),
                    "decision": context.decision.clone(),
                }));
            }
            Err(error) => {
                last_error = format!("model {model} failed: {error}");
                attempts.push(json!({
                    "model": model,
                    "succeeded": false,
                    "quality": "failed",
                    "message": last_error,
                    "cost": context.cost.clone(),
                    "classification": context.classification.clone(),
                    "decision": context.decision.clone(),
                }));
            }
        }
    }
    write_json_if_requested(&args.attempts_file, &attempts)?;
    Err(last_error.into())
}

fn resolve_synthesis_config(args: &SynthesizeArgs) -> Result<EffectiveSynthesisConfig> {
    let manifest = load_manifest(&args.repo_root)?.unwrap_or_default();
    let product_name = nonblank_or(
        &args.product_name,
        manifest.product.name.as_deref(),
        "product-name",
    )?;
    let product_description = nonblank_or_default(
        &args.product_description,
        manifest.product.description.as_deref(),
    );
    let voice_guide = nonblank_or_default(&args.voice_guide, manifest.voice.as_deref());
    let audience = optional_or_default(
        args.audience.as_deref(),
        manifest.audience.as_deref(),
        "general",
    );
    let changelog_source = optional_or_default(
        args.changelog_source.as_deref(),
        manifest.changelog.source.as_deref(),
        "auto",
    );
    let model_policy = optional_or_default(
        Some(args.model_policy.as_str()),
        manifest.model.policy.as_deref(),
        "balanced",
    );
    let model = trimmed_option(&args.model)
        .or_else(|| manifest.model.primary.as_deref().and_then(trimmed_option))
        .or_else(|| policy_default_model(Some(&model_policy)))
        .unwrap_or_default();
    let fallback_models = if !args.fallback_models.trim().is_empty() {
        args.fallback_models.trim().to_string()
    } else {
        manifest.model.fallbacks.join(",")
    };
    Ok(EffectiveSynthesisConfig {
        product_name,
        product_description,
        voice_guide,
        audience,
        changelog_source,
        model_policy,
        model,
        fallback_models,
        max_input_tokens: manifest.budget.max_input_tokens,
        max_output_tokens: manifest.budget.max_output_tokens,
        max_usd: manifest.budget.max_usd,
    })
}

fn nonblank_or(value: &str, manifest: Option<&str>, name: &str) -> Result<String> {
    if let Some(value) = trimmed_option(value) {
        return Ok(value);
    }
    if let Some(value) = manifest.and_then(trimmed_option) {
        return Ok(value);
    }
    Err(format!("{name} must not be blank").into())
}

fn nonblank_or_default(value: &str, manifest: Option<&str>) -> String {
    trimmed_option(value)
        .or_else(|| manifest.and_then(trimmed_option))
        .unwrap_or_default()
}

fn optional_or_default(value: Option<&str>, manifest: Option<&str>, default: &str) -> String {
    value
        .and_then(trimmed_option)
        .or_else(|| manifest.and_then(trimmed_option))
        .unwrap_or_else(|| default.to_string())
}

fn policy_default_model(policy: Option<&str>) -> Option<String> {
    match policy.and_then(trimmed_option).as_deref() {
        Some("off") => Some("off".into()),
        Some("cheap") => Some("openai/gpt-4o-mini".into()),
        Some("balanced") => Some("anthropic/claude-sonnet-4".into()),
        Some("rich") => Some("anthropic/claude-sonnet-4".into()),
        _ => Some("anthropic/claude-sonnet-4".into()),
    }
}

fn resolve_technical_changelog(
    args: &SynthesizeArgs,
    config: &EffectiveSynthesisConfig,
) -> Result<String> {
    let source = config.changelog_source.to_ascii_lowercase();
    let from_changelog = if args.changelog_file.is_file() {
        let text = fs::read_to_string(&args.changelog_file)?;
        if text.trim().is_empty() {
            None
        } else {
            Some(extract_release_section(&text, &args.version))
        }
    } else {
        None
    };
    let from_release_body = read_optional_file(&args.release_body_file)?;
    let from_prs = read_optional_file(&args.pr_changelog_file)?;
    match source.as_str() {
        "auto" => from_changelog
            .or(from_release_body)
            .or(from_prs)
            .ok_or_else(|| "no changelog source found".into()),
        "changelog" => from_changelog.ok_or_else(|| "CHANGELOG.md is missing or empty".into()),
        "release-body" => {
            from_release_body.ok_or_else(|| "release body source is missing or empty".into())
        }
        "prs" => from_prs.ok_or_else(|| "PR changelog source is missing or empty".into()),
        _ => Err(format!("invalid changelog-source {source}").into()),
    }
}

fn read_optional_file(path: &Path) -> Result<Option<String>> {
    if path.as_os_str().is_empty() || !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    if text.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

fn extract_release_section(text: &str, version: &str) -> String {
    let normalized =
        normalize_version(version).unwrap_or_else(|_| version.trim_start_matches('v').to_string());
    let heading = Regex::new(r"(?m)^##\s+\[?v?([0-9]+\.[0-9]+\.[0-9][^\]\s]*)\]?.*$").unwrap();
    let matches: Vec<_> = heading.find_iter(text).collect();
    if matches.is_empty() {
        return text.to_string();
    }
    for (index, mat) in matches.iter().enumerate() {
        let line = text[mat.start()..mat.end()].to_string();
        if line.contains(&normalized) || line.contains(version) {
            let end = matches
                .get(index + 1)
                .map(|next| next.start())
                .unwrap_or(text.len());
            return text[mat.start()..end].trim().to_string();
        }
    }
    let first = matches[0];
    let end = matches
        .get(1)
        .map(|next| next.start())
        .unwrap_or(text.len());
    text[first.start()..end].trim().to_string()
}

fn render_prompt(
    args: &SynthesizeArgs,
    config: &EffectiveSynthesisConfig,
    technical: &str,
) -> Result<String> {
    let template = if args.prompt_template.is_file() {
        fs::read_to_string(&args.prompt_template)?
    } else {
        let filename = match config.audience.as_str() {
            "developer" | "end-user" | "enterprise" | "general" => {
                format!("{}.md", config.audience)
            }
            _ => return Err(format!("invalid audience {}", config.audience).into()),
        };
        let path = args.templates_dir.join(filename);
        if path.is_file() {
            fs::read_to_string(path)?
        } else {
            fs::read_to_string("templates/synthesis-prompt.md")?
        }
    };
    let product_context = if config.product_description.trim().is_empty() {
        String::new()
    } else {
        format!("Product context: {}\n", config.product_description.trim())
    };
    let voice_guide = if config.voice_guide.trim().is_empty() {
        String::new()
    } else {
        format!("Voice guide: {}\n", config.voice_guide.trim())
    };
    Ok(template
        .replace("{{PRODUCT_NAME}}", &config.product_name)
        .replace("{{VERSION}}", &args.version)
        .replace("{{TECHNICAL_CHANGELOG}}", technical)
        .replace("{{PRODUCT_CONTEXT}}", &product_context)
        .replace("{{VOICE_GUIDE}}", &voice_guide)
        .replace("{{BULLET_TARGET}}", "4")
        .replace("{{BREAKING_CHANGES}}", &render_breaking_changes(technical)))
}

fn synthesis_context_packet(
    args: &SynthesizeArgs,
    config: &EffectiveSynthesisConfig,
    technical: &str,
    prompt: &str,
) -> SynthesisContextPacket {
    let sources = synthesis_context_sources(args, config, technical, prompt);
    let classification = classify_release_context(technical, &sources);
    let cost = estimate_synthesis_cost(config, prompt, &classification, &sources);
    let decision = synthesis_decision(config, &cost, &classification);
    SynthesisContextPacket {
        product: ContextProduct {
            name: config.product_name.clone(),
            audience: config.audience.clone(),
            description: config.product_description.clone(),
        },
        release: ContextRelease {
            version: args.version.clone(),
            changelog_source: config.changelog_source.clone(),
            model_policy: config.model_policy.clone(),
        },
        deterministic: deterministic_release_context(args, config),
        sources,
        classification,
        cost,
        decision,
    }
}

fn deterministic_release_context(
    args: &SynthesizeArgs,
    config: &EffectiveSynthesisConfig,
) -> DeterministicReleaseContext {
    let repo_root = &args.repo_root;
    DeterministicReleaseContext {
        commits: context_commits(repo_root, &args.version),
        tags: context_tags(repo_root),
        changed_files: context_changed_files(repo_root, &args.version),
        manifest: ContextManifestSummary {
            present: repo_root.join(".landmark.yml").is_file(),
            product_name: config.product_name.clone(),
            audience: config.audience.clone(),
            model_policy: config.model_policy.clone(),
        },
        docs: context_documents(repo_root),
        package: context_package(repo_root),
        prior_releases: context_prior_releases(repo_root),
        pr_metadata: context_optional_source(&args.pr_changelog_file),
        release_body: context_optional_source(&args.release_body_file),
        artifacts: ContextArtifactAudiences {
            internal_technical_changelog: "landmark.internal-technical-changelog.v1".into(),
            public_release_notes: format!("landmark.public-release-notes.v1:{}", config.audience),
        },
    }
}

fn context_commits(repo_root: &Path, version: &str) -> Vec<ContextCommit> {
    let (previous, target) = context_git_range(repo_root, version);
    local_release_commits(repo_root, &previous, &target)
        .unwrap_or_default()
        .into_iter()
        .take(30)
        .map(|commit| ContextCommit {
            conventional_type: conventional_commit_type(&commit.subject)
                .unwrap_or("")
                .to_string(),
            breaking: is_breaking_commit(&commit),
            subject: commit.subject,
            short_hash: commit.short_hash,
        })
        .collect()
}

fn context_changed_files(repo_root: &Path, version: &str) -> Vec<String> {
    let (previous, target) = context_git_range(repo_root, version);
    let range = if previous.trim().is_empty() {
        format!("{}..{target}", empty_git_tree())
    } else {
        format!("{previous}..{target}")
    };
    run_ok("git", ["diff", "--name-only", range.as_str()], repo_root)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(100)
        .map(str::to_string)
        .collect()
}

fn context_git_range(repo_root: &Path, version: &str) -> (String, String) {
    let tags = backfill_tags(repo_root).unwrap_or_default();
    let normalized = version.trim();
    let target = if tags.iter().any(|tag| tag.tag == normalized) {
        normalized.to_string()
    } else {
        "HEAD".into()
    };
    let previous = tags
        .iter()
        .find(|tag| tag.tag == normalized)
        .and_then(|tag| previous_backfill_tag(&tags, tag))
        .or_else(|| {
            tags.iter()
                .rfind(|tag| !tag.prerelease)
                .map(|tag| tag.tag.clone())
        })
        .filter(|tag| tag != &target)
        .unwrap_or_default();
    (previous, target)
}

fn context_tags(repo_root: &Path) -> Vec<String> {
    backfill_tags(repo_root)
        .unwrap_or_default()
        .into_iter()
        .rev()
        .take(10)
        .map(|tag| tag.tag)
        .collect()
}

fn context_documents(repo_root: &Path) -> Vec<ContextDocument> {
    ["README.md", "docs/README.md"]
        .iter()
        .filter_map(|path| {
            let full = repo_root.join(path);
            let text = fs::read_to_string(&full).ok()?;
            let title = text
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("")
                .trim()
                .trim_start_matches('#')
                .trim()
                .to_string();
            Some(ContextDocument {
                path: (*path).into(),
                title,
                estimated_tokens: estimate_tokens(&text),
            })
        })
        .collect()
}

fn context_package(repo_root: &Path) -> Option<ContextPackage> {
    if let Some(package) = read_package_json(repo_root) {
        return Some(ContextPackage {
            manager: "npm".into(),
            name: package["name"].as_str().unwrap_or("").to_string(),
            description: package["description"].as_str().unwrap_or("").to_string(),
        });
    }
    let cargo = fs::read_to_string(repo_root.join("Cargo.toml")).ok()?;
    let name = Regex::new(r#"(?m)^name\s*=\s*"([^"]+)""#)
        .ok()?
        .captures(&cargo)
        .and_then(|caps| caps.get(1))
        .map(|value| value.as_str().to_string())
        .unwrap_or_default();
    Some(ContextPackage {
        manager: "cargo".into(),
        name,
        description: String::new(),
    })
}

fn context_prior_releases(repo_root: &Path) -> Vec<String> {
    let changelog = fs::read_to_string(repo_root.join("CHANGELOG.md")).unwrap_or_default();
    Regex::new(r"(?m)^##\s+(.+)$")
        .unwrap()
        .captures_iter(&changelog)
        .filter_map(|caps| caps.get(1).map(|value| value.as_str().trim().to_string()))
        .take(5)
        .collect()
}

fn context_optional_source(path: &Path) -> ContextOptionalSource {
    let text = read_optional_file(path).ok().flatten().unwrap_or_default();
    ContextOptionalSource {
        present: !text.trim().is_empty(),
        estimated_tokens: if text.trim().is_empty() {
            0
        } else {
            estimate_tokens(&text)
        },
    }
}

fn synthesis_decision(
    config: &EffectiveSynthesisConfig,
    cost: &CostEstimate,
    classification: &ReleaseClassification,
) -> SynthesisDecision {
    let (action, reason, llm_required) = if cost.skip {
        ("skipped", cost.skip_reason.clone(), false)
    } else if config.model_policy.trim().eq_ignore_ascii_case("balanced")
        && cost.model_tier == "rich"
        && classification.significance == "high"
    {
        (
            "escalated",
            "high-significance release uses rich model tier".into(),
            true,
        )
    } else {
        (
            "used",
            format!(
                "{} policy uses {} model tier",
                config.model_policy, cost.model_tier
            ),
            true,
        )
    };
    SynthesisDecision {
        action: action.into(),
        reason,
        llm_required,
        model_tier: cost.model_tier.clone(),
    }
}

fn empty_git_tree() -> &'static str {
    "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
}

fn synthesis_context_sources(
    args: &SynthesizeArgs,
    config: &EffectiveSynthesisConfig,
    technical: &str,
    prompt: &str,
) -> Vec<ContextSource> {
    let mut sources = vec![
        context_source("prompt_template", "prompt", prompt),
        context_source("technical_changelog", &config.changelog_source, technical),
    ];
    if !config.product_description.trim().is_empty() {
        sources.push(context_source(
            "product_manifest",
            "manifest",
            &config.product_description,
        ));
    }
    if !config.voice_guide.trim().is_empty() {
        sources.push(context_source(
            "voice_guide",
            "manifest",
            &config.voice_guide,
        ));
    }
    if let Ok(Some(body)) = read_optional_file(&args.release_body_file) {
        sources.push(context_source("release_body", "release-body", &body));
    }
    if let Ok(Some(prs)) = read_optional_file(&args.pr_changelog_file) {
        sources.push(context_source("pull_requests", "prs", &prs));
    }
    sources
}

fn context_source(name: &str, kind: &str, text: &str) -> ContextSource {
    ContextSource {
        name: name.to_string(),
        kind: kind.to_string(),
        bytes: text.len(),
        estimated_tokens: estimate_tokens(text),
        included: !text.trim().is_empty(),
    }
}

fn estimate_tokens(text: &str) -> u64 {
    let chars = text.chars().count() as u64;
    chars.div_ceil(4).max(1)
}

fn classify_release_context(technical: &str, sources: &[ContextSource]) -> ReleaseClassification {
    let lower = technical.to_ascii_lowercase();
    let mut categories = BTreeSet::new();
    let mut reasons = Vec::new();
    let docs = lower.contains("docs:")
        || lower.contains("documentation")
        || lower.contains("readme")
        || lower.contains(".md");
    let chore = lower.contains("chore:")
        || lower.contains("ci:")
        || lower.contains("build:")
        || lower.contains("test:")
        || lower.contains("refactor:");
    let dependencies = lower.contains("dependabot")
        || lower.contains("dependency")
        || lower.contains("dependencies")
        || lower.contains("package-lock")
        || lower.contains("cargo.lock");
    let internal = lower.contains("workflow")
        || lower.contains(".github/")
        || lower.contains("script")
        || lower.contains("harness")
        || lower.contains("replay");
    let mut user_visible = lower.contains("feat:")
        || lower.contains("fix:")
        || lower.contains("user")
        || lower.contains("public")
        || lower.contains("cli")
        || lower.contains("action input")
        || lower.contains("release notes");
    let breaking = lower.contains("breaking change")
        || Regex::new(r"(?m)^[*-]?\s*[a-z]+(\([^)]*\))?!:")
            .unwrap()
            .is_match(technical);
    let security = lower.contains("security")
        || lower.contains("vulnerability")
        || lower.contains("cve-")
        || lower.contains("secret");
    let migration_heavy = lower.contains("migration")
        || lower.contains("migrate")
        || lower.contains("deprecat")
        || lower.contains("manifest")
        || lower.contains("configuration");

    if docs {
        categories.insert("docs-only");
        reasons.push("documentation signals detected".to_string());
    }
    if chore {
        categories.insert("chore-only");
        reasons.push("chore/build/test/refactor signals detected".to_string());
    }
    if dependencies {
        categories.insert("dependency-only");
        reasons.push("dependency update signals detected".to_string());
    }
    if internal {
        categories.insert("internal-tooling");
        reasons.push("internal tooling or workflow signals detected".to_string());
    }
    if user_visible {
        categories.insert("user-visible");
        reasons.push("feature, fix, CLI, or public-surface signals detected".to_string());
    }
    if breaking {
        categories.insert("breaking");
        reasons.push("breaking-change signals detected".to_string());
    }
    if security {
        categories.insert("security");
        reasons.push("security-sensitive signals detected".to_string());
    }
    if migration_heavy {
        categories.insert("migration-heavy");
        reasons.push("migration or configuration signals detected".to_string());
    }
    if sources
        .iter()
        .any(|source| source.name == "pull_requests" && source.included)
    {
        reasons.push("PR metadata contributed to context".to_string());
    }
    if categories.is_empty() {
        categories.insert("user-visible");
        user_visible = true;
        reasons.push("no low-value-only signals found; defaulting to user-visible".to_string());
    }

    let low_value_only = !user_visible && !breaking && !security && !migration_heavy;
    let significance = if breaking || security || migration_heavy {
        "high"
    } else if low_value_only {
        "low"
    } else {
        "medium"
    }
    .to_string();

    ReleaseClassification {
        categories: categories.into_iter().map(str::to_string).collect(),
        significance,
        user_visible,
        breaking,
        security,
        migration_heavy,
        reasons,
    }
}

fn estimate_synthesis_cost(
    config: &EffectiveSynthesisConfig,
    prompt: &str,
    classification: &ReleaseClassification,
    _sources: &[ContextSource],
) -> CostEstimate {
    let policy = config.model_policy.trim().to_ascii_lowercase();
    let (model_tier, model, mut skip, mut skip_reason) =
        selected_model_plan(config, classification);
    let input_tokens = estimate_tokens(prompt);
    let output_tokens = config
        .max_output_tokens
        .unwrap_or(match model_tier.as_str() {
            "cheap" => 700,
            "rich" => 1400,
            _ => 1000,
        });
    if !skip
        && let Some(max_input) = config.max_input_tokens
        && input_tokens > max_input
    {
        skip = true;
        skip_reason =
            format!("estimated input tokens {input_tokens} exceed manifest budget {max_input}");
    }
    let estimated_usd = estimate_model_cost_usd(&model_tier, input_tokens, output_tokens);
    if !skip
        && let Some(max_usd) = config.max_usd
        && estimated_usd > max_usd
    {
        skip = true;
        skip_reason = format!(
            "estimated synthesis cost ${estimated_usd:.4} exceeds manifest budget ${max_usd:.4}"
        );
    }
    if policy == "off" {
        skip = true;
        skip_reason = "model.policy=off disables LLM synthesis".into();
    }
    CostEstimate {
        input_tokens,
        output_tokens,
        model_tier,
        model,
        estimated_usd,
        skip,
        skip_reason,
    }
}

fn selected_model_plan(
    config: &EffectiveSynthesisConfig,
    classification: &ReleaseClassification,
) -> (String, String, bool, String) {
    match config.model_policy.trim().to_ascii_lowercase().as_str() {
        "off" => (
            "off".into(),
            "off".into(),
            true,
            "model.policy=off disables LLM synthesis".into(),
        ),
        "cheap" => ("cheap".into(), cheap_model(config), false, String::new()),
        "rich" => ("rich".into(), rich_model(config), false, String::new()),
        _ if classification.significance == "low" => (
            "off".into(),
            "off".into(),
            true,
            "low-significance docs/chore/dependency release skipped by balanced policy".into(),
        ),
        _ if classification.significance == "high" => {
            ("rich".into(), rich_model(config), false, String::new())
        }
        _ => (
            "balanced".into(),
            config.model.clone(),
            false,
            String::new(),
        ),
    }
}

fn cheap_model(config: &EffectiveSynthesisConfig) -> String {
    if config.model != "off" && !config.model.trim().is_empty() {
        config.model.clone()
    } else {
        "openai/gpt-4o-mini".into()
    }
}

fn rich_model(config: &EffectiveSynthesisConfig) -> String {
    if config.model != "off" && !config.model.trim().is_empty() {
        config.model.clone()
    } else {
        "anthropic/claude-sonnet-4".into()
    }
}

fn estimate_model_cost_usd(tier: &str, input_tokens: u64, output_tokens: u64) -> f64 {
    let (input_per_million, output_per_million) = match tier {
        "cheap" => (0.15, 0.60),
        "rich" => (3.00, 15.00),
        "off" => (0.0, 0.0),
        _ => (1.00, 5.00),
    };
    ((input_tokens as f64 / 1_000_000.0) * input_per_million)
        + ((output_tokens as f64 / 1_000_000.0) * output_per_million)
}

fn render_breaking_changes(technical: &str) -> String {
    let mut changes = BTreeSet::new();
    let breaking_commit = Regex::new(r"^[a-z]+(\([^)]*\))?!:").unwrap();
    for line in technical.lines() {
        let trimmed = line.trim().trim_start_matches("- ").trim();
        if trimmed.to_ascii_lowercase().contains("breaking change")
            || breaking_commit.is_match(trimmed)
        {
            changes.insert(trimmed.to_string());
        }
    }
    if changes.is_empty() {
        String::new()
    } else {
        let mut rendered = String::from("Breaking changes:\n");
        for change in changes {
            rendered.push_str(&format!("- {change}\n"));
        }
        rendered
    }
}

fn request_synthesis(api_url: &str, api_key: &str, model: &str, prompt: &str) -> Result<String> {
    let payload = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "You write concise user-facing release notes."},
            {"role": "user", "content": prompt}
        ]
    });
    let response = curl_json("POST", api_url, Some(api_key), Some(&payload))?;
    if !(200..300).contains(&response.status) {
        return Err(format!("HTTP {}", response.status).into());
    }
    let value: Value = serde_json::from_str(&response.body)?;
    let content = value["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("provider response did not include choices[0].message.content")?;
    Ok(content.to_string())
}

fn validate_notes(notes: &str) -> bool {
    notes
        .lines()
        .any(|line| line.trim_start().starts_with("## "))
        && notes
            .lines()
            .any(|line| line.trim_start().starts_with("- "))
}

fn release_policy(args: ReleasePolicyArgs) -> Result<()> {
    match args.command {
        ReleasePolicyCommand::Publication(args) => publication_policy(args),
        ReleasePolicyCommand::Summary(args) => summary_policy(*args),
    }
}

fn publication_policy(args: PublicationArgs) -> Result<()> {
    let required = parse_bool(&args.synthesis_required) || parse_bool(&args.synthesis_strict);
    let succeeded = parse_bool(&args.synth_succeeded);
    let quality = normalize_quality(&args.synth_quality);
    let mut can_update_release = succeeded;
    let mut can_publish_artifacts = succeeded;
    let mut failure_stage = args.synth_failure_stage.clone();
    let mut failure_message = args.synth_failure_message.clone();
    let mut exit_failure = false;
    let policy_succeeded = if quality == "skipped" || failure_stage == "skipped" {
        can_update_release = false;
        can_publish_artifacts = false;
        true
    } else {
        succeeded
    };
    if policy_succeeded && quality == "degraded" && required {
        can_update_release = false;
        can_publish_artifacts = false;
        failure_stage = "validation".to_string();
        failure_message = "Synthesis quality is degraded and synthesis is required.".to_string();
        exit_failure = true;
    } else if !policy_succeeded && required {
        can_update_release = false;
        can_publish_artifacts = false;
        if failure_stage.is_empty() {
            failure_stage = "synthesis".to_string();
        }
        if failure_message.is_empty() {
            failure_message = "Synthesis failed and synthesis is required.".to_string();
        }
        exit_failure = true;
    }
    write_outputs(
        &args.github_output,
        &[
            ("succeeded", policy_succeeded.to_string()),
            ("quality", quality),
            ("can_update_release", can_update_release.to_string()),
            ("can_publish_artifacts", can_publish_artifacts.to_string()),
            ("failure_stage", sanitize_text(&failure_stage)),
            ("failure_message", sanitize_text(&failure_message)),
        ],
    )?;
    if exit_failure {
        Err("synthesis publication policy failed".into())
    } else {
        Ok(())
    }
}

fn summary_policy(args: SummaryArgs) -> Result<()> {
    let synthesis_enabled = parse_bool(&args.synthesis_enabled);
    let released = parse_bool(&args.released);
    let synth_succeeded = parse_bool(&args.synth_succeeded);
    let update_succeeded = parse_bool(&args.update_succeeded);
    let artifact_succeeded = parse_bool(&args.artifact_succeeded);
    let rss_enabled = parse_bool(&args.rss_enabled);
    let rss_succeeded = parse_bool(&args.rss_succeeded);
    let webhook_enabled = parse_bool(&args.webhook_enabled);
    let webhook_sent = parse_bool(&args.webhook_sent);
    let slack_enabled = parse_bool(&args.slack_enabled);
    let slack_sent = parse_bool(&args.slack_sent);
    let quality = normalize_quality(&args.synth_quality);
    let synthesis_skipped = quality == "skipped" || args.synth_failure_stage == "skipped";
    let (succeeded, failure_stage, failure_message) = if !synthesis_enabled || !released {
        (true, "", "")
    } else if synthesis_skipped {
        (
            true,
            args.synth_failure_stage.as_str(),
            args.synth_failure_message.as_str(),
        )
    } else if !synth_succeeded {
        (
            false,
            args.synth_failure_stage.as_str(),
            args.synth_failure_message.as_str(),
        )
    } else if !update_succeeded {
        (
            false,
            args.update_failure_stage.as_str(),
            args.update_failure_message.as_str(),
        )
    } else if !artifact_succeeded {
        (
            false,
            args.artifact_failure_stage.as_str(),
            args.artifact_failure_message.as_str(),
        )
    } else if rss_enabled && !rss_succeeded {
        (
            false,
            args.rss_failure_stage.as_str(),
            args.rss_failure_message.as_str(),
        )
    } else {
        (true, "", "")
    };
    let mut destinations = BTreeMap::new();
    destinations.insert(
        "release_body".to_string(),
        DestinationStatus {
            enabled: synthesis_enabled && released && synth_succeeded && !synthesis_skipped,
            succeeded: update_succeeded,
            failure_stage: sanitize_text(&args.update_failure_stage),
            failure_message: sanitize_text(&args.update_failure_message),
        },
    );
    destinations.insert(
        "artifacts".to_string(),
        DestinationStatus {
            enabled: synthesis_enabled
                && released
                && synth_succeeded
                && update_succeeded
                && !synthesis_skipped,
            succeeded: artifact_succeeded,
            failure_stage: sanitize_text(&args.artifact_failure_stage),
            failure_message: sanitize_text(&args.artifact_failure_message),
        },
    );
    destinations.insert(
        "rss".to_string(),
        DestinationStatus {
            enabled: rss_enabled,
            succeeded: rss_succeeded,
            failure_stage: sanitize_text(&args.rss_failure_stage),
            failure_message: sanitize_text(&args.rss_failure_message),
        },
    );
    destinations.insert(
        "webhook".to_string(),
        DestinationStatus {
            enabled: webhook_enabled,
            succeeded: webhook_sent,
            failure_stage: String::new(),
            failure_message: String::new(),
        },
    );
    destinations.insert(
        "slack".to_string(),
        DestinationStatus {
            enabled: slack_enabled,
            succeeded: slack_sent,
            failure_stage: String::new(),
            failure_message: String::new(),
        },
    );
    let status = SynthesisStatus {
        synthesis_enabled,
        released,
        succeeded,
        quality: quality.clone(),
        failure_stage: sanitize_text(failure_stage),
        failure_message: sanitize_text(failure_message),
        model_attempts: read_json_array_if_requested(Path::new(&args.attempts_file))?,
        context: read_json_value_if_requested(Path::new(&args.context_metadata_file))?,
        destinations,
    };
    write_outputs(
        &args.github_output,
        &[
            ("succeeded", succeeded.to_string()),
            ("quality", quality),
            ("failure_stage", sanitize_text(failure_stage)),
            ("failure_message", sanitize_text(failure_message)),
            ("status_json", serde_json::to_string(&status)?),
        ],
    )
}

fn normalize_quality(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "valid" => "valid".to_string(),
        "degraded" => "degraded".to_string(),
        "skipped" => "skipped".to_string(),
        "failed" => "failed".to_string(),
        _ => "failed".to_string(),
    }
}

fn update_release(args: UpdateReleaseArgs) -> Result<()> {
    let notes = read_nonempty(&args.notes_file)?;
    let provider = GitHubProvider::required(&args.api_base_url, &args.github_token);
    provider.update_release_body(&args.repository, &args.tag, &notes)?;
    Ok(())
}

fn compose_release_body(notes: &str, existing: &str) -> String {
    let stripped = strip_existing_whats_new(existing);
    if stripped.trim().is_empty() {
        format!("## What's New\n\n{}\n", notes.trim())
    } else {
        format!("## What's New\n\n{}\n\n{}", notes.trim(), stripped.trim())
    }
}

fn strip_existing_whats_new(body: &str) -> String {
    let mut output = Vec::new();
    let mut skipping = false;
    let mut skipped = false;
    for line in body.lines() {
        if !skipped && line.trim() == "## What's New" {
            skipping = true;
            skipped = true;
            continue;
        }
        if skipping && line.starts_with("## ") {
            skipping = false;
        }
        if !skipping {
            output.push(line);
        }
    }
    output.join("\n").trim().to_string()
}

#[derive(Clone, Serialize)]
struct ReleaseNoteArtifact {
    version: String,
    tag: String,
    notes: String,
    plaintext: String,
    html: String,
    slack: String,
    sections: Vec<NoteSection>,
    published_at: String,
}

#[derive(Clone, Serialize)]
struct NoteSection {
    title: String,
    bullets: Vec<NoteBullet>,
}

#[derive(Clone, Serialize)]
struct NoteBullet {
    text: String,
    links: Vec<NoteLink>,
}

#[derive(Clone, Serialize)]
struct NoteLink {
    label: String,
    href: String,
}

impl ReleaseNoteArtifact {
    fn from_markdown(version: &str, notes: &str) -> Self {
        let trimmed = notes.trim().to_string();
        Self {
            version: version.trim_start_matches('v').to_string(),
            tag: version.to_string(),
            plaintext: markdown_to_plaintext(&trimmed),
            html: markdown_to_html_fragment(&trimmed),
            slack: markdown_to_slack(&trimmed),
            sections: parse_note_sections(&trimmed),
            published_at: Utc::now().to_rfc3339(),
            notes: trimmed,
        }
    }

    fn json_entry(&self) -> Value {
        json!({
            "version": self.version,
            "tag": self.tag,
            "notes": self.notes,
            "markdown": self.notes,
            "html": self.html,
            "plaintext": self.plaintext,
            "slack": self.slack,
            "sections": self.sections,
            "published_at": self.published_at,
        })
    }

    fn webhook_payload(&self, repository: &str, release_url: &str) -> Value {
        json!({
            "version": self.tag,
            "repository": repository,
            "release_url": release_url,
            "notes": self.notes,
            "markdown": self.notes,
            "html": self.html,
            "plaintext": self.plaintext,
            "sections": self.sections,
            "published_at": self.published_at,
        })
    }

    fn slack_payload(&self, repository: &str, release_url: &str) -> Value {
        json!({
            "blocks": [
                {"type": "header", "text": {"type": "plain_text", "text": format!("{} {}", repository, self.tag)}},
                {"type": "section", "text": {"type": "mrkdwn", "text": self.slack}},
                {"type": "context", "elements": [{"type": "mrkdwn", "text": format!("<{}|View release>", release_url)}]}
            ]
        })
    }
}

#[derive(Serialize)]
struct SynthesisStatus {
    synthesis_enabled: bool,
    released: bool,
    succeeded: bool,
    quality: String,
    failure_stage: String,
    failure_message: String,
    model_attempts: Vec<Value>,
    context: Value,
    destinations: BTreeMap<String, DestinationStatus>,
}

#[derive(Serialize)]
struct DestinationStatus {
    enabled: bool,
    succeeded: bool,
    failure_stage: String,
    failure_message: String,
}

fn write_artifacts(args: WriteArtifactsArgs) -> Result<()> {
    let notes = read_nonempty(&args.notes_file)?;
    let artifact = ReleaseNoteArtifact::from_markdown(&args.version, &notes);
    if !args.output_file.trim().is_empty() {
        write_notes_file(&artifact.notes, &args.output_file, &args.version)?;
    }
    if !args.output_text_file.trim().is_empty() {
        write_notes_file(&artifact.plaintext, &args.output_text_file, &args.version)?;
    }
    if !args.output_html_file.trim().is_empty() {
        write_notes_file(&artifact.html, &args.output_html_file, &args.version)?;
    }
    if !args.output_json.trim().is_empty() {
        append_json_entry(&args.output_json, &artifact)?;
    }
    print!("{}", artifact.notes);
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
struct BackfillManifest {
    generated_at: String,
    mode: String,
    dry_run: bool,
    repo_root: String,
    repository: String,
    since: String,
    processed_tags: Vec<BackfillTagRecord>,
    skipped_tags: Vec<BackfillSkipRecord>,
    remaining_tags: Vec<String>,
    estimated_cost: BackfillCostEstimate,
    artifacts: Vec<BackfillArtifactRecord>,
    release_body_updates: Vec<BackfillReleaseBodyUpdate>,
}

#[derive(Clone, Debug, Serialize)]
struct BackfillTagRecord {
    tag: String,
    version: String,
    package: String,
    source: String,
    release_status: String,
    notes_sha256: String,
    estimated_prompt_tokens: usize,
}

#[derive(Clone, Debug, Serialize)]
struct BackfillSkipRecord {
    tag: String,
    reason: String,
}

#[derive(Clone, Debug, Serialize)]
struct BackfillCostEstimate {
    llm_calls: usize,
    estimated_prompt_tokens: usize,
    estimated_usd: f64,
    policy: String,
}

#[derive(Clone, Debug, Serialize)]
struct BackfillArtifactRecord {
    tag: String,
    markdown: String,
    plaintext: String,
    html: String,
    json: String,
    rss: String,
}

#[derive(Clone, Debug, Serialize)]
struct BackfillReleaseBodyUpdate {
    tag: String,
    release_id: i64,
    dry_run: bool,
    updated: bool,
    preview_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
struct RunEvidence {
    provider: String,
    generated_at: String,
    repo_root: String,
    repository: String,
    release_tag: String,
    version: String,
    previous_tag: String,
    source: String,
    technical_changelog_sha256: String,
    notes_sha256: String,
    version_decision: RunVersionDecision,
    artifacts: RunArtifactRecord,
    publication: RunPublicationRecord,
}

#[derive(Clone, Debug, Serialize)]
struct RunVersionDecision {
    latest_tag: String,
    bump: String,
    commit_count: usize,
    conventional_commit_count: usize,
    range: String,
}

#[derive(Clone, Debug, Serialize)]
struct RunArtifactRecord {
    technical_changelog: String,
    technical_changelog_audience: String,
    technical_changelog_schema: String,
    markdown: String,
    public_notes_audience: String,
    public_notes_schema: String,
    plaintext: String,
    html: String,
    json: String,
    rss: String,
    evidence: String,
}

#[derive(Clone, Debug, Serialize)]
struct RunPublicationRecord {
    provider: String,
    enabled: bool,
    release_body_updated: bool,
    release_url: String,
    status: String,
}

#[derive(Clone, Debug)]
struct RunReleaseContext {
    release_tag: String,
    previous_tag: String,
    version: String,
    decision: RunVersionDecision,
    commits: Vec<RunCommit>,
}

#[derive(Clone, Debug)]
struct RunCommit {
    subject: String,
    short_hash: String,
    body: String,
}

#[derive(Clone, Debug)]
struct BackfillTag {
    tag: String,
    version: String,
    key: (u64, u64, u64),
    package: String,
    prerelease: bool,
}

#[derive(Clone, Debug)]
struct BackfillReleaseLookup {
    status: String,
    id: Option<i64>,
    body: String,
}

#[derive(Clone, Debug)]
struct BackfillSource {
    source: String,
    notes: String,
    duplicate_changelog: bool,
}

fn run_pipeline(args: RunArgs) -> Result<()> {
    let provider = args.provider.trim().to_ascii_lowercase();
    if !matches!(provider.as_str(), "local" | "github") {
        return Err(format!(
            "unsupported provider '{provider}'; this build supports provider=local or provider=github"
        )
        .into());
    }
    if args.rss_max_entries == 0 {
        return Err("--rss-max-entries must be positive".into());
    }
    if !args.dry_run {
        fs::create_dir_all(args.repo_root.join(&args.output_dir))?;
    }
    let manifest =
        load_manifest(&args.repo_root)?.unwrap_or_else(|| infer_manifest(&args.repo_root));
    let repository = trimmed_option(&args.repository)
        .or_else(|| {
            args.repo_root
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "local".into());
    let release = resolve_local_release(&args)?;
    let technical_changelog = render_local_technical_changelog(&release);
    let notes = if let Some(notes_file) =
        run_output_path(&args.repo_root, &args.notes_file, &release.release_tag)
    {
        read_nonempty(&notes_file)?
    } else {
        render_local_public_notes(&manifest, &release)
    };
    let artifacts = write_run_artifacts(
        &args,
        &manifest,
        &repository,
        &release.release_tag,
        &release_url_base(&args, &repository),
        &technical_changelog,
        &notes,
    )?;
    let publication =
        publish_run_release_body(&args, &provider, &repository, &release.release_tag, &notes)?;
    let evidence = RunEvidence {
        provider,
        generated_at: Utc::now().to_rfc3339(),
        repo_root: args.repo_root.display().to_string(),
        repository,
        release_tag: release.release_tag.clone(),
        version: release.version.clone(),
        previous_tag: release.previous_tag.clone(),
        source: "git_range".into(),
        technical_changelog_sha256: sha256_hex(technical_changelog.as_bytes()),
        notes_sha256: sha256_hex(notes.as_bytes()),
        version_decision: release.decision,
        artifacts,
        publication,
    };
    let evidence_json = serde_json::to_string_pretty(&evidence)? + "\n";
    let evidence_path = run_output_path(&args.repo_root, &args.evidence_file, &release.release_tag)
        .ok_or("--evidence-file must not be empty")?;
    if !args.dry_run {
        ensure_parent(&evidence_path)?;
        fs::write(&evidence_path, &evidence_json)?;
    }
    println!("{evidence_json}");
    Ok(())
}

fn resolve_local_release(args: &RunArgs) -> Result<RunReleaseContext> {
    let tags = backfill_tags(&args.repo_root)?;
    let latest_tag = tags.iter().rfind(|tag| !tag.prerelease).cloned();
    let explicit_release_tag = trimmed_option(&args.release_tag);
    let explicit_tag = explicit_release_tag.as_deref().and_then(backfill_parse_tag);
    let previous_tag = trimmed_option(&args.previous_tag)
        .or_else(|| {
            explicit_tag
                .as_ref()
                .and_then(|tag| previous_backfill_tag(&tags, tag))
        })
        .or_else(|| latest_tag.as_ref().map(|tag| tag.tag.clone()))
        .unwrap_or_default();
    let target_ref = explicit_release_tag
        .as_ref()
        .filter(|tag| tags.iter().any(|existing| existing.tag == **tag))
        .cloned()
        .unwrap_or_else(|| "HEAD".into());
    let commits = local_release_commits(&args.repo_root, previous_tag.as_str(), &target_ref)?;
    let bump = decide_version_bump(&commits);
    let release_tag =
        explicit_release_tag.unwrap_or_else(|| next_release_tag(latest_tag.as_ref(), &bump));
    let version = release_tag.trim_start_matches('v').to_string();
    let range = if previous_tag.is_empty() {
        target_ref
    } else {
        format!("{previous_tag}..{target_ref}")
    };
    let conventional_commit_count = commits
        .iter()
        .filter(|commit| conventional_commit_type(&commit.subject).is_some())
        .count();
    Ok(RunReleaseContext {
        release_tag,
        previous_tag,
        version,
        decision: RunVersionDecision {
            latest_tag: latest_tag.map(|tag| tag.tag).unwrap_or_default(),
            bump,
            commit_count: commits.len(),
            conventional_commit_count,
            range,
        },
        commits,
    })
}

fn local_release_commits(
    repo_root: &Path,
    previous_tag: &str,
    target_ref: &str,
) -> Result<Vec<RunCommit>> {
    let range = if previous_tag.trim().is_empty() {
        target_ref.to_string()
    } else {
        format!("{previous_tag}..{target_ref}")
    };
    let log = run_ok(
        "git",
        [
            "log",
            "--reverse",
            "--format=%x1e%s%x1f%h%x1f%b",
            range.as_str(),
        ],
        repo_root,
    )?;
    Ok(log
        .split('\x1e')
        .filter_map(|record| {
            let record = record.trim_matches('\n');
            if record.trim().is_empty() {
                return None;
            }
            let mut parts = record.splitn(3, '\x1f');
            Some(RunCommit {
                subject: parts.next().unwrap_or("").trim().to_string(),
                short_hash: parts.next().unwrap_or("").trim().to_string(),
                body: parts.next().unwrap_or("").trim().to_string(),
            })
        })
        .collect())
}

fn decide_version_bump(commits: &[RunCommit]) -> String {
    if commits.iter().any(is_breaking_commit) {
        "major".into()
    } else if commits
        .iter()
        .any(|commit| conventional_commit_type(&commit.subject) == Some("feat"))
    {
        "minor".into()
    } else if commits.iter().any(|commit| {
        matches!(
            conventional_commit_type(&commit.subject),
            Some("fix" | "perf")
        )
    }) {
        "patch".into()
    } else if commits.is_empty() {
        "none".into()
    } else {
        "patch".into()
    }
}

fn conventional_commit_type(subject: &str) -> Option<&str> {
    let subject = subject.trim();
    let header = subject.split(':').next()?;
    let header = header.strip_suffix('!').unwrap_or(header);
    let header = header.split('(').next().unwrap_or(header);
    if header
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch == '-')
    {
        Some(header)
    } else {
        None
    }
}

fn is_breaking_commit(commit: &RunCommit) -> bool {
    commit.subject.contains("!:")
        || commit.subject.contains(")!:")
        || commit.body.lines().any(|line| {
            let line = line.trim();
            line.starts_with("BREAKING CHANGE:") || line.starts_with("BREAKING-CHANGE:")
        })
}

fn next_release_tag(latest: Option<&BackfillTag>, bump: &str) -> String {
    let Some(latest) = latest else {
        return match bump {
            "major" => "v1.0.0".into(),
            "minor" => "v0.1.0".into(),
            "none" => "v0.0.0".into(),
            _ => "v0.0.1".into(),
        };
    };
    let (mut major, mut minor, mut patch) = latest.key;
    match bump {
        "major" => {
            major += 1;
            minor = 0;
            patch = 0;
        }
        "minor" => {
            minor += 1;
            patch = 0;
        }
        "none" => {}
        _ => {
            patch += 1;
        }
    }
    if latest.package.is_empty() {
        format!("v{major}.{minor}.{patch}")
    } else {
        format!("{}@v{major}.{minor}.{patch}", latest.package)
    }
}

fn render_local_technical_changelog(release: &RunReleaseContext) -> String {
    let mut markdown = format!("## Technical Changelog {}\n\n", release.release_tag);
    if release.commits.is_empty() {
        markdown.push_str("- No commits were found in the selected release range.\n");
    } else {
        for commit in &release.commits {
            markdown.push_str("- ");
            markdown.push_str(&commit.display_line());
            markdown.push('\n');
        }
    }
    markdown
}

fn render_local_public_notes(manifest: &LandmarkManifest, release: &RunReleaseContext) -> String {
    let product = manifest
        .product
        .name
        .as_deref()
        .and_then(trimmed_option)
        .unwrap_or_else(|| "This project".into());
    let mut markdown = format!("## Improvements in {}\n\n", release.release_tag);
    if release.commits.is_empty() {
        markdown.push_str(&format!(
            "- {product} has no user-visible commit entries in this release range.\n"
        ));
    } else {
        for commit in &release.commits {
            markdown.push_str("- ");
            markdown.push_str(&humanize_commit_subject(&commit.subject));
            markdown.push('\n');
        }
    }
    markdown
}

impl RunCommit {
    fn display_line(&self) -> String {
        if self.short_hash.is_empty() {
            self.subject.clone()
        } else {
            format!("{} ({})", self.subject, self.short_hash)
        }
    }
}

fn humanize_commit_subject(subject: &str) -> String {
    let text = subject
        .split_once(':')
        .map(|(_, rest)| rest)
        .unwrap_or(subject)
        .trim();
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => subject.to_string(),
    }
}

fn write_run_artifacts(
    args: &RunArgs,
    manifest: &LandmarkManifest,
    repository: &str,
    release_tag: &str,
    release_url_base: &str,
    technical_changelog: &str,
    notes: &str,
) -> Result<RunArtifactRecord> {
    let artifact = ReleaseNoteArtifact::from_markdown(release_tag, notes);
    let technical = run_template_path(&args.repo_root, &args.technical_changelog_file, release_tag);
    if !args.dry_run && !technical.as_os_str().is_empty() {
        write_path(&technical, technical_changelog)?;
    }
    let markdown = run_template_path(&args.repo_root, &args.output_file, release_tag);
    if !args.dry_run && !markdown.as_os_str().is_empty() {
        write_path(&markdown, &artifact.notes)?;
    }
    let plaintext = run_template_path(&args.repo_root, &args.output_text_file, release_tag);
    if !args.dry_run && !plaintext.as_os_str().is_empty() {
        write_path(&plaintext, &artifact.plaintext)?;
    }
    let html = run_template_path(&args.repo_root, &args.output_html_file, release_tag);
    if !args.dry_run && !html.as_os_str().is_empty() {
        write_path(&html, &artifact.html)?;
    }
    let json_path = if args.output_json.trim().is_empty() {
        PathBuf::new()
    } else if args.dry_run {
        run_template_path(&args.repo_root, &args.output_json, release_tag)
    } else {
        backfill_append_json(&args.repo_root, &args.output_json, &artifact)?
    };
    let rss_path = if args.rss_feed_file.trim().is_empty() {
        PathBuf::new()
    } else if args.dry_run {
        args.repo_root.join(&args.rss_feed_file)
    } else {
        let path = args.repo_root.join(&args.rss_feed_file);
        let existing = fs::read_to_string(&path).unwrap_or_default();
        let mut items = parse_existing_feed_items(&existing);
        items.retain(|item| item.guid != release_tag);
        items.insert(
            0,
            FeedItem {
                title: format!("{repository} {release_tag}"),
                link: release_link(release_url_base, repository, release_tag),
                guid: release_tag.to_string(),
                description: artifact.html,
                pub_date: Utc::now().to_rfc2822(),
            },
        );
        items.truncate(args.rss_max_entries);
        ensure_parent(&path)?;
        fs::write(&path, render_feed(repository, release_url_base, &items))?;
        path
    };
    let evidence =
        run_output_path(&args.repo_root, &args.evidence_file, release_tag).unwrap_or_default();
    Ok(RunArtifactRecord {
        technical_changelog: technical.display().to_string(),
        technical_changelog_audience: "internal-developer-operator".into(),
        technical_changelog_schema: "landmark.internal-technical-changelog.v1".into(),
        markdown: markdown.display().to_string(),
        public_notes_audience: manifest
            .audience
            .as_deref()
            .and_then(trimmed_option)
            .unwrap_or_else(|| "general".into()),
        public_notes_schema: "landmark.public-release-notes.v1".into(),
        plaintext: plaintext.display().to_string(),
        html: html.display().to_string(),
        json: json_path.display().to_string(),
        rss: rss_path.display().to_string(),
        evidence: evidence.display().to_string(),
    })
}

fn release_url_base(args: &RunArgs, repository: &str) -> String {
    trimmed_option(&args.server_url)
        .map(|url| format!("{}/{}", url.trim_end_matches('/'), repository))
        .unwrap_or_else(|| default_release_url_base(repository))
}

fn release_link(base: &str, repository: &str, release_tag: &str) -> String {
    if repository.contains('/') {
        format!("{}/releases/tag/{release_tag}", base.trim_end_matches('/'))
    } else {
        format!("local://{repository}/releases/{release_tag}")
    }
}

fn publish_run_release_body(
    args: &RunArgs,
    provider: &str,
    repository: &str,
    release_tag: &str,
    notes: &str,
) -> Result<RunPublicationRecord> {
    if args.dry_run {
        return Ok(RunPublicationRecord {
            provider: provider.into(),
            enabled: args.publish_release_body,
            release_body_updated: false,
            release_url: release_link(&release_url_base(args, repository), repository, release_tag),
            status: "dry-run; release-body publication previewed but not mutated".into(),
        });
    }
    if provider == "local" {
        return Ok(RunPublicationRecord {
            provider: provider.into(),
            enabled: false,
            release_body_updated: false,
            release_url: format!("local://{repository}/releases/{release_tag}"),
            status: "local provider does not mutate remote releases".into(),
        });
    }
    let release_url = if repository.contains('/') {
        format!("https://github.com/{repository}/releases/tag/{release_tag}")
    } else {
        String::new()
    };
    if !args.publish_release_body {
        return Ok(RunPublicationRecord {
            provider: provider.into(),
            enabled: false,
            release_body_updated: false,
            release_url,
            status: "release-body publication skipped".into(),
        });
    }
    let token = trimmed_option(&args.github_token)
        .or_else(|| {
            env::var("GITHUB_TOKEN")
                .ok()
                .and_then(|value| trimmed_option(&value))
        })
        .or_else(|| {
            env::var("GH_TOKEN")
                .ok()
                .and_then(|value| trimmed_option(&value))
        })
        .ok_or("--publish-release-body requires --github-token, GITHUB_TOKEN, or GH_TOKEN")?;
    let gh_provider = GitHubProvider::required(&args.api_base_url, &token);
    let release_url = gh_provider.update_release_body(repository, release_tag, notes)?;
    Ok(RunPublicationRecord {
        provider: provider.into(),
        enabled: true,
        release_body_updated: true,
        release_url,
        status: "updated".into(),
    })
}

fn run_template_path(repo_root: &Path, template: &str, tag: &str) -> PathBuf {
    run_output_path(repo_root, template, tag).unwrap_or_default()
}

fn write_path(path: &Path, content: &str) -> Result<()> {
    ensure_parent(path)?;
    fs::write(path, content)?;
    Ok(())
}

fn run_output_path(repo_root: &Path, template: &str, tag: &str) -> Option<PathBuf> {
    trimmed_option(template).map(|value| repo_root.join(value.replace("{version}", tag)))
}

fn backfill(args: BackfillArgs) -> Result<()> {
    let mode = args.mode.trim();
    if mode != "artifacts-only" && mode != "release-body" {
        return Err("backfill --mode must be artifacts-only or release-body".into());
    }
    if args.rss_max_entries == 0 {
        return Err("--rss-max-entries must be positive".into());
    }
    let repository = if args.repository.trim().is_empty() {
        env::var("GITHUB_REPOSITORY").unwrap_or_default()
    } else {
        args.repository.trim().to_string()
    };
    if !repository.is_empty() {
        validate_repo(&repository)?;
    }
    if mode == "release-body" && !args.dry_run && !args.confirm_release_body {
        return Err(
            "backfill --mode release-body requires --dry-run or --confirm-release-body".into(),
        );
    }

    let all_tags = backfill_tags(&args.repo_root)?;
    let since = args.since.trim().to_string();
    let since_index = if since.is_empty() {
        None
    } else {
        all_tags.iter().position(|tag| tag.tag == since)
    };
    let mut skipped_tags = Vec::new();
    if !since.is_empty() && since_index.is_none() {
        skipped_tags.push(BackfillSkipRecord {
            tag: since.clone(),
            reason: "since tag not found".into(),
        });
    }

    let candidate_tags: Vec<_> = if since.is_empty() {
        all_tags.clone()
    } else if let Some(index) = since_index {
        all_tags.iter().skip(index + 1).cloned().collect()
    } else {
        Vec::new()
    };
    let mut selected_tags = Vec::new();
    let mut remaining_tags = Vec::new();
    for tag in candidate_tags {
        if args.max_tags > 0 && selected_tags.len() >= args.max_tags {
            remaining_tags.push(tag.tag);
        } else {
            selected_tags.push(tag);
        }
    }

    let mut processed_tags = Vec::new();
    let mut artifacts = Vec::new();
    let mut release_body_updates = Vec::new();
    let mut feed_items = if mode == "artifacts-only" && !args.dry_run {
        parse_existing_feed_items(
            &fs::read_to_string(args.repo_root.join(&args.rss_feed_file)).unwrap_or_default(),
        )
    } else {
        Vec::new()
    };
    let mut total_prompt_tokens = 0usize;
    let token = trimmed_option(&args.github_token);

    for tag in selected_tags {
        if tag.prerelease {
            skipped_tags.push(BackfillSkipRecord {
                tag: tag.tag,
                reason: "prerelease tags are skipped by default".into(),
            });
            continue;
        }
        let release =
            backfill_release_lookup(&args.api_base_url, &repository, &tag.tag, token.as_deref())?;
        if release.body.contains("## What's New") {
            skipped_tags.push(BackfillSkipRecord {
                tag: tag.tag,
                reason: "release body already contains Landmark notes".into(),
            });
            continue;
        }
        let source = backfill_source(&args.repo_root, &tag, release.body.as_str(), &all_tags)?;
        if mode == "release-body" && source.source == "github_release" {
            skipped_tags.push(BackfillSkipRecord {
                tag: tag.tag,
                reason: "existing GitHub Release body is the source; refusing to duplicate it in release-body mode".into(),
            });
            continue;
        }
        if source.duplicate_changelog {
            skipped_tags.push(BackfillSkipRecord {
                tag: tag.tag,
                reason: "duplicate changelog sections make release mapping ambiguous".into(),
            });
            continue;
        }
        let prompt_tokens = estimate_prompt_tokens(&source.notes);
        total_prompt_tokens += prompt_tokens;
        let record = BackfillTagRecord {
            tag: tag.tag.clone(),
            version: tag.version.clone(),
            package: tag.package.clone(),
            source: source.source.clone(),
            release_status: release.status.clone(),
            notes_sha256: sha256_hex(source.notes.as_bytes()),
            estimated_prompt_tokens: prompt_tokens,
        };

        if mode == "artifacts-only" {
            if args.dry_run {
                artifacts.push(backfill_plan_artifacts(&args, &tag));
            } else {
                artifacts.push(backfill_write_artifacts(
                    &args,
                    &repository,
                    &tag,
                    &source.notes,
                    &mut feed_items,
                )?);
            }
        } else if let Some(id) = release.id {
            let updated_body = compose_release_body(&source.notes, &release.body);
            let preview_sha256 = sha256_hex(updated_body.as_bytes());
            if !args.dry_run {
                backfill_update_release_body(&args, &repository, id, &updated_body)?;
            }
            release_body_updates.push(BackfillReleaseBodyUpdate {
                tag: tag.tag.clone(),
                release_id: id,
                dry_run: args.dry_run,
                updated: !args.dry_run,
                preview_sha256,
            });
        } else {
            skipped_tags.push(BackfillSkipRecord {
                tag: tag.tag.clone(),
                reason: format!(
                    "release-body mode requires an existing GitHub Release ({})",
                    release.status
                ),
            });
            continue;
        }

        processed_tags.push(record);
    }

    if mode == "artifacts-only" && !args.dry_run {
        backfill_write_feed(&args, &repository, feed_items)?;
    }

    let manifest = BackfillManifest {
        generated_at: Utc::now().to_rfc3339(),
        mode: mode.into(),
        dry_run: args.dry_run,
        repo_root: args.repo_root.display().to_string(),
        repository,
        since,
        processed_tags,
        skipped_tags,
        remaining_tags,
        estimated_cost: BackfillCostEstimate {
            llm_calls: 0,
            estimated_prompt_tokens: total_prompt_tokens,
            estimated_usd: 0.0,
            policy:
                "artifact backfill does not call the LLM; use the manifest to batch later synthesis"
                    .into(),
        },
        artifacts,
        release_body_updates,
    };
    println!("{}", serde_json::to_string_pretty(&manifest)?);
    if !args.dry_run {
        let resume_path = args.repo_root.join(&args.resume_file);
        ensure_parent(&resume_path)?;
        fs::write(resume_path, serde_json::to_string_pretty(&manifest)? + "\n")?;
    }
    Ok(())
}

fn backfill_tags(repo_root: &Path) -> Result<Vec<BackfillTag>> {
    let mut tags = git_tags(repo_root)?
        .into_iter()
        .filter_map(|tag| backfill_parse_tag(&tag))
        .collect::<Vec<_>>();
    tags.sort_by(|left, right| {
        left.key
            .cmp(&right.key)
            .then_with(|| left.package.cmp(&right.package))
            .then_with(|| left.tag.cmp(&right.tag))
    });
    Ok(tags)
}

fn backfill_parse_tag(tag: &str) -> Option<BackfillTag> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"^(?:(?P<package>[A-Za-z0-9_.-]+)@)?v?(?P<major>[0-9]+)\.(?P<minor>[0-9]+)\.(?P<patch>[0-9]+)(?P<pre>-[A-Za-z0-9][A-Za-z0-9.-]*)?$",
        )
        .unwrap()
    });
    let caps = re.captures(tag.trim())?;
    let major = caps.name("major")?.as_str().parse().ok()?;
    let minor = caps.name("minor")?.as_str().parse().ok()?;
    let patch = caps.name("patch")?.as_str().parse().ok()?;
    let package = caps
        .name("package")
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();
    Some(BackfillTag {
        tag: tag.trim().to_string(),
        version: format!("{major}.{minor}.{patch}"),
        key: (major, minor, patch),
        package,
        prerelease: caps.name("pre").is_some(),
    })
}

fn backfill_release_lookup(
    api_base_url: &str,
    repository: &str,
    tag: &str,
    github_token: Option<&str>,
) -> Result<BackfillReleaseLookup> {
    if repository.is_empty() {
        return Ok(BackfillReleaseLookup {
            status: "unavailable: repository not configured".into(),
            id: None,
            body: String::new(),
        });
    }
    if github_token.is_none() {
        return Ok(BackfillReleaseLookup {
            status: "unavailable: github token not configured".into(),
            id: None,
            body: String::new(),
        });
    }
    let provider = GitHubProvider::new(api_base_url, github_token);
    match provider.release_by_tag(repository, tag) {
        Ok(Some(value)) => Ok(BackfillReleaseLookup {
            status: "found".into(),
            id: value["id"].as_i64(),
            body: value["body"].as_str().unwrap_or("").to_string(),
        }),
        Ok(None) => Ok(BackfillReleaseLookup {
            status: "missing".into(),
            id: None,
            body: String::new(),
        }),
        Err(error) => Ok(BackfillReleaseLookup {
            status: format!("unavailable: {error}"),
            id: None,
            body: String::new(),
        }),
    }
}

fn backfill_source(
    repo_root: &Path,
    tag: &BackfillTag,
    release_body: &str,
    all_tags: &[BackfillTag],
) -> Result<BackfillSource> {
    if !release_body.trim().is_empty() {
        return Ok(BackfillSource {
            source: "github_release".into(),
            notes: release_body.trim().to_string(),
            duplicate_changelog: false,
        });
    }
    let changelog_path = repo_root.join("CHANGELOG.md");
    let changelog = changelog_sections(&changelog_path, &tag.version)?;
    if !changelog.sections.is_empty() {
        return Ok(BackfillSource {
            source: "changelog".into(),
            notes: changelog.sections[0].clone(),
            duplicate_changelog: changelog.duplicate,
        });
    }
    let git_notes = backfill_git_range_notes(repo_root, tag, all_tags)?;
    if !git_notes.trim().is_empty() {
        return Ok(BackfillSource {
            source: "git_range".into(),
            notes: git_notes,
            duplicate_changelog: false,
        });
    }
    let manifest = load_manifest(repo_root)?.unwrap_or_else(|| infer_manifest(repo_root));
    let product_name = manifest.product.name.as_deref().unwrap_or("the repository");
    Ok(BackfillSource {
        source: "manifest_context".into(),
        notes: format!(
            "## Historical Release {}\n\n- Historical notes were unavailable in GitHub Releases, CHANGELOG.md, and the tag range.\n- Product context: {}.\n",
            tag.tag, product_name
        ),
        duplicate_changelog: false,
    })
}

struct ChangelogSections {
    sections: Vec<String>,
    duplicate: bool,
}

fn changelog_sections(path: &Path, version: &str) -> Result<ChangelogSections> {
    if !path.is_file() {
        return Ok(ChangelogSections {
            sections: Vec::new(),
            duplicate: false,
        });
    }
    let text = fs::read_to_string(path)?;
    let marker = format!("[{version}]");
    let bare_marker = format!(" {version}");
    let mut sections = Vec::new();
    let mut current = Vec::new();
    let mut started = false;
    for line in text.lines() {
        let heading = line.starts_with('#');
        if heading
            && (line.contains(&marker)
                || line.trim_end() == format!("## {version}")
                || line.contains(&bare_marker))
        {
            if started && !current.is_empty() {
                sections.push(current.join("\n").trim().to_string());
                current.clear();
            }
            started = true;
            current.push(line.to_string());
            continue;
        }
        if started && heading && (line.starts_with("# ") || line.starts_with("## ")) {
            sections.push(current.join("\n").trim().to_string());
            current.clear();
            started = false;
            continue;
        }
        if started {
            current.push(line.to_string());
        }
    }
    if started && !current.is_empty() {
        sections.push(current.join("\n").trim().to_string());
    }
    sections.retain(|section| !section.trim().is_empty());
    Ok(ChangelogSections {
        duplicate: sections.len() > 1,
        sections,
    })
}

fn backfill_git_range_notes(
    repo_root: &Path,
    tag: &BackfillTag,
    all_tags: &[BackfillTag],
) -> Result<String> {
    let previous = previous_backfill_tag(all_tags, tag);
    let range = previous
        .map(|prev| format!("{prev}..{}", tag.tag))
        .unwrap_or_else(|| tag.tag.clone());
    let log = run_ok(
        "git",
        ["log", "--reverse", "--format=%s (%h)", range.as_str()],
        repo_root,
    )?;
    if log.trim().is_empty() {
        return Ok(String::new());
    }
    let mut notes = format!("## Historical Release {}\n\n", tag.tag);
    for line in log.lines().filter(|line| !line.trim().is_empty()) {
        notes.push_str("- ");
        notes.push_str(line.trim());
        notes.push('\n');
    }
    Ok(notes)
}

fn previous_backfill_tag(all_tags: &[BackfillTag], current: &BackfillTag) -> Option<String> {
    let mut previous = None;
    for tag in all_tags {
        if tag.package == current.package && tag.key < current.key && !tag.prerelease {
            previous = Some(tag.tag.clone());
        }
    }
    previous
}

fn backfill_write_artifacts(
    args: &BackfillArgs,
    repository: &str,
    tag: &BackfillTag,
    notes: &str,
    feed_items: &mut Vec<FeedItem>,
) -> Result<BackfillArtifactRecord> {
    let artifact = ReleaseNoteArtifact::from_markdown(&tag.tag, notes);
    let markdown = backfill_write_template_if_requested(
        &args.repo_root,
        &args.output_file,
        &tag.tag,
        &artifact.notes,
    )?;
    let plaintext = backfill_write_template_if_requested(
        &args.repo_root,
        &args.output_text_file,
        &tag.tag,
        &artifact.plaintext,
    )?;
    let html = backfill_write_template_if_requested(
        &args.repo_root,
        &args.output_html_file,
        &tag.tag,
        &artifact.html,
    )?;
    let json_path = if args.output_json.trim().is_empty() {
        PathBuf::new()
    } else {
        backfill_append_json(&args.repo_root, &args.output_json, &artifact)?
    };
    let release_url = if repository.is_empty() {
        String::new()
    } else {
        format!("https://github.com/{repository}/releases/tag/{}", tag.tag)
    };
    if !args.rss_feed_file.trim().is_empty() {
        feed_items.retain(|item| item.guid != tag.tag);
        feed_items.insert(
            0,
            FeedItem {
                title: if repository.is_empty() {
                    tag.tag.clone()
                } else {
                    format!("{repository} {}", tag.tag)
                },
                link: release_url,
                guid: tag.tag.clone(),
                description: artifact.html,
                pub_date: Utc::now().to_rfc2822(),
            },
        );
        feed_items.truncate(args.rss_max_entries);
    }
    Ok(BackfillArtifactRecord {
        tag: tag.tag.clone(),
        markdown: markdown.display().to_string(),
        plaintext: plaintext.display().to_string(),
        html: html.display().to_string(),
        json: json_path.display().to_string(),
        rss: backfill_output_path(&args.repo_root, &args.rss_feed_file, &tag.tag)
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
    })
}

fn backfill_plan_artifacts(args: &BackfillArgs, tag: &BackfillTag) -> BackfillArtifactRecord {
    BackfillArtifactRecord {
        tag: tag.tag.clone(),
        markdown: backfill_output_path(&args.repo_root, &args.output_file, &tag.tag)
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        plaintext: backfill_output_path(&args.repo_root, &args.output_text_file, &tag.tag)
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        html: backfill_output_path(&args.repo_root, &args.output_html_file, &tag.tag)
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        json: backfill_output_path(&args.repo_root, &args.output_json, &tag.tag)
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        rss: backfill_output_path(&args.repo_root, &args.rss_feed_file, &tag.tag)
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
    }
}

fn backfill_write_template_if_requested(
    repo_root: &Path,
    template: &str,
    tag: &str,
    content: &str,
) -> Result<PathBuf> {
    let Some(path) = backfill_output_path(repo_root, template, tag) else {
        return Ok(PathBuf::new());
    };
    ensure_parent(&path)?;
    fs::write(&path, content)?;
    Ok(path)
}

fn backfill_output_path(repo_root: &Path, template: &str, tag: &str) -> Option<PathBuf> {
    trimmed_option(template).map(|value| repo_root.join(value.replace("{version}", tag)))
}

fn backfill_append_json(
    repo_root: &Path,
    template: &str,
    artifact: &ReleaseNoteArtifact,
) -> Result<PathBuf> {
    let path = repo_root.join(template.replace("{version}", &artifact.tag));
    let mut entries = if path.is_file() {
        serde_json::from_str::<Vec<Value>>(&fs::read_to_string(&path)?)?
    } else {
        Vec::new()
    };
    entries.retain(|entry| {
        entry["tag"].as_str() != Some(&artifact.tag)
            && entry["version"].as_str() != Some(&artifact.version)
    });
    entries.insert(0, artifact.json_entry());
    ensure_parent(&path)?;
    fs::write(&path, serde_json::to_string_pretty(&entries)? + "\n")?;
    Ok(path)
}

fn backfill_write_feed(args: &BackfillArgs, repository: &str, items: Vec<FeedItem>) -> Result<()> {
    if args.rss_feed_file.trim().is_empty() {
        return Ok(());
    }
    let path = args.repo_root.join(&args.rss_feed_file);
    ensure_parent(&path)?;
    fs::write(
        path,
        render_feed(repository, &default_release_url_base(repository), &items),
    )?;
    Ok(())
}

fn backfill_update_release_body(
    args: &BackfillArgs,
    repository: &str,
    release_id: i64,
    body: &str,
) -> Result<()> {
    let url = format!(
        "{}/repos/{}/releases/{}",
        args.api_base_url.trim_end_matches('/'),
        repository,
        release_id
    );
    let response = curl_json(
        "PATCH",
        &url,
        trimmed_option(&args.github_token).as_deref(),
        Some(&json!({ "body": body })),
    )?;
    if (200..300).contains(&response.status) {
        Ok(())
    } else {
        Err(format!(
            "GitHub release backfill update failed with HTTP {}",
            response.status
        )
        .into())
    }
}

fn estimate_prompt_tokens(text: &str) -> usize {
    text.split_whitespace().count().max(1) * 4 / 3 + 64
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write_notes_file(content: &str, template: &str, version: &str) -> Result<PathBuf> {
    let path = PathBuf::from(template.replace("{version}", version));
    ensure_parent(&path)?;
    fs::write(&path, content)?;
    Ok(path)
}

fn append_json_entry(template: &str, artifact: &ReleaseNoteArtifact) -> Result<()> {
    let path = PathBuf::from(template.replace("{version}", &artifact.tag));
    let mut entries = if path.is_file() {
        serde_json::from_str::<Vec<Value>>(&fs::read_to_string(&path)?)?
    } else {
        Vec::new()
    };
    entries.retain(|entry| {
        entry["tag"].as_str() != Some(&artifact.tag)
            && entry["version"].as_str() != Some(&artifact.version)
    });
    entries.push(artifact.json_entry());
    ensure_parent(&path)?;
    fs::write(path, serde_json::to_string_pretty(&entries)? + "\n")?;
    Ok(())
}

fn parse_note_sections(markdown: &str) -> Vec<NoteSection> {
    let mut sections = Vec::new();
    let mut current = NoteSection {
        title: "Release notes".to_string(),
        bullets: Vec::new(),
    };
    let link_re = Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") {
            if !current.bullets.is_empty() || current.title != "Release notes" {
                sections.push(current);
            }
            current = NoteSection {
                title: trimmed.trim_start_matches('#').trim().to_string(),
                bullets: Vec::new(),
            };
            continue;
        }
        if let Some(text) = trimmed.strip_prefix("- ") {
            let links = link_re
                .captures_iter(text)
                .filter_map(|caps| {
                    let href = caps.get(2)?.as_str();
                    Some(NoteLink {
                        label: caps.get(1)?.as_str().to_string(),
                        href: safe_link_href(href)?.to_string(),
                    })
                })
                .collect();
            current.bullets.push(NoteBullet {
                text: markdown_to_plaintext(text),
                links,
            });
        }
    }
    if !current.bullets.is_empty() || current.title != "Release notes" {
        sections.push(current);
    }
    sections
}

fn markdown_to_plaintext(markdown: &str) -> String {
    let mut text = String::new();
    let link_re = Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap();
    for line in markdown.lines() {
        let mut line = line
            .trim()
            .trim_start_matches('#')
            .trim()
            .trim_start_matches("- ")
            .to_string();
        line = link_re.replace_all(&line, "$1").to_string();
        line = line.replace("**", "").replace('`', "");
        if !line.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&line);
        }
    }
    text
}

fn markdown_to_html_fragment(markdown: &str) -> String {
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    let parser = MarkdownParser::new_ext(markdown, options);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    Regex::new(r#"href="([^"]+)""#)
        .unwrap()
        .replace_all(&out, |caps: &regex::Captures| {
            let href = caps.get(1).unwrap().as_str();
            if safe_link_href(href).is_some() {
                format!("href=\"{href}\"")
            } else {
                "href=\"#\"".to_string()
            }
        })
        .to_string()
}

fn safe_link_href(url: &str) -> Option<&str> {
    let lower = url.trim().to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        Some(url)
    } else {
        None
    }
}

fn update_feed(args: UpdateFeedArgs) -> Result<()> {
    if args.max_entries == 0 {
        return Err("max-entries must be positive".into());
    }
    let notes = read_nonempty(&args.notes_file)?;
    let artifact = ReleaseNoteArtifact::from_markdown(&args.release_tag, &notes);
    let path = args.workspace.join(&args.feed_file);
    let canonical_workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace.clone());
    let parent = path.parent().unwrap_or(&args.workspace);
    fs::create_dir_all(parent)?;
    let canonical_parent = parent
        .canonicalize()
        .unwrap_or_else(|_| parent.to_path_buf());
    if !canonical_parent.starts_with(&canonical_workspace) {
        return Err("feed-file must stay inside workspace".into());
    }
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let mut items = parse_existing_feed_items(&existing);
    let new_item = FeedItem {
        title: format!("{} {}", args.repository, args.release_tag),
        link: args.release_url,
        guid: args.release_tag.clone(),
        description: artifact.html,
        pub_date: Utc::now().to_rfc2822(),
    };
    items.retain(|item| item.guid != new_item.guid);
    items.insert(0, new_item);
    items.truncate(args.max_entries);
    let xml = render_feed(
        &args.repository,
        &default_release_url_base(&args.repository),
        &items,
    );
    fs::write(path, xml)?;
    Ok(())
}

#[derive(Clone)]
struct FeedItem {
    title: String,
    link: String,
    guid: String,
    description: String,
    pub_date: String,
}

fn parse_existing_feed_items(xml: &str) -> Vec<FeedItem> {
    let item_re = Regex::new(r"(?s)<item>(.*?)</item>").unwrap();
    item_re
        .captures_iter(xml)
        .map(|cap| {
            let block = cap.get(1).unwrap().as_str();
            FeedItem {
                title: xml_tag(block, "title").unwrap_or_default(),
                link: xml_tag(block, "link").unwrap_or_default(),
                guid: xml_tag(block, "guid").unwrap_or_default(),
                description: xml_tag(block, "description").unwrap_or_default(),
                pub_date: xml_tag(block, "pubDate").unwrap_or_default(),
            }
        })
        .collect()
}

fn xml_tag(block: &str, tag: &str) -> Option<String> {
    let re = Regex::new(&format!(r"(?s)<{tag}>(.*?)</{tag}>")).ok()?;
    Some(re.captures(block)?.get(1)?.as_str().to_string())
}

fn render_feed(repository: &str, channel_link: &str, items: &[FeedItem]) -> String {
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\">\n<channel>\n<title>{}</title>\n<link>{}</link>\n<description>Release notes for {}</description>\n<lastBuildDate>{}</lastBuildDate>\n",
        xml_escape(repository),
        xml_escape(channel_link),
        xml_escape(repository),
        Utc::now().to_rfc2822()
    );
    for item in items {
        xml.push_str(&format!(
            "<item><title>{}</title><link>{}</link><guid>{}</guid><description><![CDATA[{}]]></description><pubDate>{}</pubDate></item>\n",
            xml_escape(&item.title),
            xml_escape(&item.link),
            xml_escape(&item.guid),
            item.description.replace("]]>", "]]]]><![CDATA[>"),
            xml_escape(&item.pub_date)
        ));
    }
    xml.push_str("</channel>\n</rss>\n");
    xml
}

fn default_release_url_base(repository: &str) -> String {
    if repository.contains('/') {
        format!("https://github.com/{repository}")
    } else {
        format!("local://{repository}/releases")
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn notify_webhook(args: NotifyWebhookArgs) -> Result<()> {
    validate_url(&args.webhook_url)?;
    validate_repo(&args.repository)?;
    let notes = read_nonempty(&args.notes_file)?;
    let artifact = ReleaseNoteArtifact::from_markdown(&args.version, &notes);
    let payload = artifact.webhook_payload(&args.repository, &args.release_url);
    let body = payload.to_string();
    let mut command = Command::new("curl");
    command
        .args([
            "-sS",
            "-L",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
        ])
        .arg("-H")
        .arg("User-Agent: landmark")
        .arg("--data")
        .arg(&body)
        .arg(&args.webhook_url);
    if !args.webhook_secret.is_empty() {
        let sig = compute_signature(&args.webhook_secret, body.as_bytes())?;
        command.arg("-H").arg(format!("X-Signature-256: {sig}"));
    }
    let output = command.output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string().into())
    }
}

fn compute_signature(secret: &str, body: &[u8]) -> Result<String> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())?;
    mac.update(body);
    Ok(format!(
        "sha256={}",
        hex::encode(mac.finalize().into_bytes())
    ))
}

fn notify_slack(args: NotifySlackArgs) -> Result<()> {
    validate_url(&args.slack_webhook_url)?;
    if !args.slack_webhook_url.contains("hooks.slack.com/") {
        return Err("slack-webhook-url must target hooks.slack.com".into());
    }
    validate_repo(&args.repository)?;
    let notes = read_nonempty(&args.notes_file)?;
    let artifact = ReleaseNoteArtifact::from_markdown(&args.version, &notes);
    let payload = artifact.slack_payload(&args.repository, &args.release_url);
    let response = curl_json("POST", &args.slack_webhook_url, None, Some(&payload))?;
    if (200..300).contains(&response.status) {
        Ok(())
    } else {
        Err(format!("Slack webhook failed with HTTP {}", response.status).into())
    }
}

fn markdown_to_slack(markdown: &str) -> String {
    let text = Regex::new(r"\[([^\]]+)\]\(([^)]+)\)")
        .unwrap()
        .replace_all(markdown, |caps: &regex::Captures| {
            let label = caps.get(1).unwrap().as_str();
            let href = caps.get(2).unwrap().as_str();
            if safe_link_href(href).is_some() {
                format!("<{href}|{label}>")
            } else {
                label.to_string()
            }
        })
        .to_string();
    text.replace("**", "*")
}

fn parse_major_tag(release_tag: &str) -> Option<String> {
    let re = Regex::new(r"^v?([0-9]+)\.[0-9]+\.[0-9]+$").unwrap();
    let major = re.captures(release_tag)?.get(1)?.as_str();
    Some(format!("v{major}"))
}

fn close_resolved_failures(args: FailureLifecycleArgs) -> Result<()> {
    let provider = GitHubProvider::required(&args.api_base_url, &args.github_token);
    let issues = provider.find_failure_issues(&args.repository, &args.release_tag)?;
    for issue in issues {
        let number = issue["number"].as_i64().unwrap_or_default();
        provider.comment_issue(
            &args.repository,
            number,
            &format!("Landmark synthesis recovered for {}.", args.release_tag),
        )?;
        provider.close_issue(&args.repository, number)?;
    }
    Ok(())
}

fn report_synthesis_failure(args: ReportFailureArgs) -> Result<()> {
    validate_url(&args.workflow_run_url)?;
    let provider = GitHubProvider::required(&args.api_base_url, &args.github_token);
    if !provider
        .find_failure_issues(&args.repository, &args.release_tag)?
        .is_empty()
    {
        return Ok(());
    }
    let title = failure_issue_title(&args.release_tag);
    let body = format!(
        "Landmark could not synthesize user-facing release notes for `{}`.\n\n- Workflow: {}\n- Run: {}\n- Stage: {}\n- Message: {}\n",
        args.release_tag,
        args.workflow_name,
        args.workflow_run_url,
        args.failure_stage,
        args.failure_message
    );
    provider.create_failure_issue(&args.repository, &title, &body)
}

fn failure_issue_title(release_tag: &str) -> String {
    format!("Landmark release-note synthesis failed for {release_tag}")
}

fn update_version_metadata(args: UpdateVersionArgs) -> Result<()> {
    let version = normalize_version(&args.version)?;
    let package_path = args.repo_root.join("package.json");
    if package_path.is_file() {
        let mut package: Value = serde_json::from_str(&fs::read_to_string(&package_path)?)?;
        package["version"] = Value::String(version.clone());
        fs::write(
            &package_path,
            serde_json::to_string_pretty(&package)? + "\n",
        )?;
    }
    let cargo_path = args.repo_root.join("crates/landmark/Cargo.toml");
    if cargo_path.is_file() {
        replace_toml_version(&cargo_path, &version)?;
    }
    Ok(())
}

fn replace_toml_version(path: &Path, version: &str) -> Result<()> {
    let text = fs::read_to_string(path)?;
    let replaced = Regex::new(r#"(?m)^version = "[^"]+""#)
        .unwrap()
        .replacen(&text, 1, format!("version = \"{version}\""))
        .to_string();
    fs::write(path, replaced)?;
    Ok(())
}

fn check_version_sync(args: CheckVersionArgs) -> Result<()> {
    let tags = run_ok("git", ["tag", "--merged", &args.reference], &args.repo_root)?;
    let latest = latest_semver_version(tags.lines()).ok_or("no semver tags found")?;
    let package: Value =
        serde_json::from_str(&fs::read_to_string(args.repo_root.join("package.json"))?)?;
    let package_version = package["version"].as_str().unwrap_or("");
    let cargo_version =
        cargo_version(&args.repo_root.join("crates/landmark/Cargo.toml")).unwrap_or_default();
    let mut drift = Vec::new();
    if package_version != latest {
        drift.push(format!(
            "package.json has {package_version}, expected {latest}"
        ));
    }
    if !cargo_version.is_empty() && cargo_version != latest {
        drift.push(format!(
            "crates/landmark/Cargo.toml has {cargo_version}, expected {latest}"
        ));
    }
    if drift.is_empty() {
        println!("metadata matches latest tag version {latest}");
        Ok(())
    } else if args.allow_release_candidate
        && package_version == cargo_version
        && semver_key(package_version)? > semver_key(&latest)?
        && release_candidate_changelog_exists(&args.repo_root.join("CHANGELOG.md"), package_version)
    {
        println!("metadata is valid release candidate {package_version} above latest tag {latest}");
        Ok(())
    } else {
        Err(drift.join("\n").into())
    }
}

fn cargo_version(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    Regex::new(r#"(?m)^version = "([^"]+)""#)
        .ok()?
        .captures(&text)?
        .get(1)
        .map(|m| m.as_str().to_string())
}

fn latest_semver_version<'a>(tags: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut versions: Vec<_> = tags.filter_map(semver_from_tag).collect();
    versions.sort();
    versions.pop().map(|(_, value)| value)
}

fn semver_from_tag(tag: &str) -> Option<((u64, u64, u64), String)> {
    let re = Regex::new(r"^v?([0-9]+)\.([0-9]+)\.([0-9]+)$").unwrap();
    let caps = re.captures(tag.trim())?;
    let major = caps.get(1)?.as_str().parse().ok()?;
    let minor = caps.get(2)?.as_str().parse().ok()?;
    let patch = caps.get(3)?.as_str().parse().ok()?;
    Some(((major, minor, patch), format!("{major}.{minor}.{patch}")))
}

fn normalize_version(version: &str) -> Result<String> {
    let value = version.trim().trim_start_matches('v');
    if semver_from_tag(value).is_none() {
        return Err(format!("invalid semver version {version}").into());
    }
    Ok(value.to_string())
}

fn check_action_contract(args: CheckActionContractArgs) -> Result<()> {
    let action_path = args.repo_root.join("action.yml");
    let readme_path = args.repo_root.join("README.md");
    let action: serde_yaml::Value = serde_yaml::from_str(&fs::read_to_string(&action_path)?)?;
    let inputs = action["inputs"]
        .as_mapping()
        .ok_or("action.yml missing inputs")?;
    let readme = fs::read_to_string(&readme_path)?;
    let mut errors = Vec::new();
    for (name, spec) in inputs {
        let name = name.as_str().unwrap_or_default();
        if !readme.contains(&format!("`{name}`")) {
            errors.push(format!("README missing input `{name}`"));
        }
        let default = spec["default"].as_str().unwrap_or("");
        if !default.is_empty() && !readme.contains(&format!("| `{name}` |")) {
            errors.push(format!("README input table missing `{name}`"));
        }
    }
    let known: BTreeSet<_> = inputs.keys().filter_map(|key| key.as_str()).collect();
    for path in default_contract_scan_paths(&args.repo_root) {
        if !path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        errors.extend(validate_landmark_usage_inputs(&path, &text, &known));
    }
    errors.extend(validate_manifest_schema_contract(&readme));
    errors.extend(validate_manifest_action_precedence_contract(
        &fs::read_to_string(&action_path)?,
    ));
    errors.extend(validate_self_release_workflow_contract(&args.repo_root)?);
    errors.extend(validate_agent_native_contracts(&args.repo_root)?);
    errors.extend(validate_release_integrity_contract(&readme));
    errors.extend(validate_first_run_adoption_contract(&args.repo_root)?);
    if errors.is_empty() {
        println!("action contract ok");
        Ok(())
    } else {
        Err(errors.join("\n").into())
    }
}

fn validate_agent_native_contracts(repo_root: &Path) -> Result<Vec<String>> {
    let mut errors = Vec::new();
    let readme = fs::read_to_string(repo_root.join("README.md")).unwrap_or_default();
    let guide = fs::read_to_string(repo_root.join("docs/agent-integration.md")).unwrap_or_default();
    for descriptor in schema_descriptors() {
        let path = repo_root.join(descriptor.path);
        if !path.is_file() {
            errors.push(format!("missing schema `{}`", descriptor.path));
            continue;
        }
        let schema: Value = match serde_json::from_str(&fs::read_to_string(&path)?) {
            Ok(schema) => schema,
            Err(error) => {
                errors.push(format!(
                    "schema `{}` is invalid JSON: {error}",
                    descriptor.path
                ));
                continue;
            }
        };
        if schema["$id"].as_str() != Some(descriptor.id) {
            errors.push(format!("schema `{}` has wrong $id", descriptor.path));
        }
        if schema["x-landmark-artifact"].as_str() != Some(descriptor.artifact) {
            errors.push(format!(
                "schema `{}` has wrong x-landmark-artifact",
                descriptor.path
            ));
        }
        if !readme.contains(descriptor.path) {
            errors.push(format!("README missing schema path `{}`", descriptor.path));
        }
        if !guide.contains(descriptor.path) {
            errors.push(format!(
                "agent integration guide missing schema path `{}`",
                descriptor.path
            ));
        }
    }
    errors.extend(validate_command_contract_coverage());
    errors.extend(validate_manifest_schema_alignment(
        &repo_root.join("schemas/landmark-manifest.v1.schema.json"),
    )?);
    for required in [
        "landmark describe --json",
        "--error-format json",
        "replay-action --scenario agent_native_contracts",
        "stdout carries JSON payloads",
        "stderr carries logs and errors",
    ] {
        if !readme.contains(required) {
            errors.push(format!("README missing agent contract token `{required}`"));
        }
        if !guide.contains(required) {
            errors.push(format!(
                "agent integration guide missing contract token `{required}`"
            ));
        }
    }
    Ok(errors)
}

fn validate_command_contract_coverage() -> Vec<String> {
    let commands: BTreeSet<String> = Cli::command()
        .get_subcommands()
        .map(|command| command.get_name().to_string())
        .collect();
    let contracts: BTreeSet<String> = command_contracts()
        .into_iter()
        .filter_map(|contract| {
            contract
                .command
                .split_whitespace()
                .next()
                .map(str::to_string)
        })
        .collect();
    commands
        .difference(&contracts)
        .map(|command| format!("describe contract missing command `{command}`"))
        .chain(
            contracts
                .difference(&commands)
                .map(|command| format!("describe contract references unknown command `{command}`")),
        )
        .collect()
}

fn validate_release_integrity_contract(readme: &str) -> Vec<String> {
    [
        "--connect-timeout",
        "--max-time",
        "http_resilience_policy",
        "action_side_effect_coverage",
        "webhook",
        "Slack",
    ]
    .iter()
    .filter(|required| !readme.contains(**required))
    .map(|required| format!("README missing release integrity token `{required}`"))
    .collect()
}

fn validate_first_run_adoption_contract(repo_root: &Path) -> Result<Vec<String>> {
    let mut errors = Vec::new();
    let readme = fs::read_to_string(repo_root.join("README.md")).unwrap_or_default();
    let action = fs::read_to_string(repo_root.join("action.yml")).unwrap_or_default();
    let ci = fs::read_to_string(repo_root.join(".github/workflows/ci.yml")).unwrap_or_default();
    let manual_example =
        fs::read_to_string(repo_root.join("examples/manual-tag.yml")).unwrap_or_default();

    for required in [
        "## Adoption Modes",
        "### Local CLI Preview",
        "### Generic CI",
        "### GitHub Action Full Mode",
        "### GitHub Action Synthesis-Only Mode",
        "cargo run --locked -- run --provider local --repo-root .",
        "dist/landmark is the checked-in",
        "Linux x86_64 action binary",
        "Packaged binaries are not published yet",
        "replay-action --scenario first_run_local_preview",
    ] {
        if !readme.contains(required) {
            errors.push(format!(
                "README missing first-run adoption token `{required}`"
            ));
        }
    }

    if action.contains("default: \"22\"") {
        errors.push("action.yml node-version default must not remain on Node 22".into());
    }
    if !action.contains("default: \"24\"") {
        errors.push("action.yml node-version default must be 24".into());
    }
    if ci.contains("node-version: \"22\"") || ci.contains("node-version: 22") {
        errors.push("CI workflow must not pin setup-node to Node 22".into());
    }
    if !ci.contains("node-version: \"24\"") || !ci.contains("node --version | grep '^v24\\.'") {
        errors.push("CI workflow must pin and verify Node 24".into());
    }

    let diagnosis = SetupDiagnosis {
        release_tool: "manual-tag".into(),
        default_branch: "main".into(),
        tag_format: "v{version}".into(),
        conventional_commits: "ready".into(),
        monorepo: false,
        packages: Vec::new(),
        signals: Vec::new(),
    };
    let workflows = setup_workflows(&diagnosis, None);
    let manual = &workflows["manual-tag"].content;
    if manual.contains("push:\n    tags:") {
        errors
            .push("setup manual-tag workflow must not include tag-push trigger by default".into());
    }
    if !manual.contains("release:\n    types: [published]") {
        errors.push("setup manual-tag workflow must use release.published trigger".into());
    }
    for candidate in workflows.values() {
        if candidate.content.contains("node-version: 22") {
            errors.push(format!(
                "{} generated workflow still pins Node 22",
                candidate.path
            ));
        }
    }

    errors.extend(validate_docs_link_targets(repo_root, &readme));
    errors.extend(validate_readme_command_names(&readme));
    for required_model in [
        "anthropic/claude-sonnet-4",
        "openai/gpt-4o-mini",
        "google/gemini-2.5-flash",
    ] {
        if !readme.contains(required_model) {
            errors.push(format!(
                "README missing supported model id `{required_model}`"
            ));
        }
    }
    if readme.contains("misty-step/landmark@v2") {
        errors.push("README references nonexistent misty-step/landmark@v2 example".into());
    }
    if let Err(error) = serde_yaml::from_str::<serde_yaml::Value>(&manual_example) {
        errors.push(format!("examples/manual-tag.yml is invalid YAML: {error}"));
    }
    if manual_example.contains("push:\n    tags:") {
        errors.push("examples/manual-tag.yml must not include tag-push trigger".into());
    }
    if !manual_example.contains("release:\n    types: [published]") {
        errors.push("examples/manual-tag.yml must use release.published trigger".into());
    }
    if !manual_example.contains("release-tag: ${{ github.event.release.tag_name }}") {
        errors.push("examples/manual-tag.yml must use github.event.release.tag_name".into());
    }
    if manual_example.contains("github.ref_name") || manual_example.contains("ref_nameevent") {
        errors
            .push("examples/manual-tag.yml contains stale tag-push release-tag expression".into());
    }

    Ok(errors)
}

fn validate_docs_link_targets(repo_root: &Path, readme: &str) -> Vec<String> {
    let link_re = Regex::new(r"\]\((docs/[^)#]+|examples/[^)#]+|schemas/[^)#]+)\)").unwrap();
    link_re
        .captures_iter(readme)
        .filter_map(|caps| {
            let path = caps.get(1).unwrap().as_str();
            if repo_root.join(path).is_file() {
                None
            } else {
                Some(format!("README links to nonexistent file `{path}`"))
            }
        })
        .collect()
}

fn validate_readme_command_names(readme: &str) -> Vec<String> {
    let commands: BTreeSet<String> = Cli::command()
        .get_subcommands()
        .map(|command| command.get_name().to_string())
        .collect();
    let nested: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::from([
        ("fleet", BTreeSet::from(["scan", "plan", "open-prs"])),
        ("release-policy", BTreeSet::from(["publication", "summary"])),
    ]);
    let command_re =
        Regex::new(r"(?m)(?:^\s*|`)landmark\s+([a-z][a-z-]*)(?:\s+([a-z][a-z-]*))?").unwrap();
    let mut errors = Vec::new();
    for caps in command_re.captures_iter(readme) {
        let command = caps.get(1).unwrap().as_str();
        if !commands.contains(command) {
            errors.push(format!(
                "README references unknown landmark command `{command}`"
            ));
            continue;
        }
        if let Some(subcommands) = nested.get(command)
            && let Some(subcommand) = caps.get(2).map(|value| value.as_str())
            && !subcommands.contains(subcommand)
            && !subcommand.starts_with("--")
        {
            errors.push(format!(
                "README references unknown landmark subcommand `{command} {subcommand}`"
            ));
        }
    }
    errors
}

fn validate_manifest_schema_alignment(path: &Path) -> Result<Vec<String>> {
    let schema: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    let mut errors = Vec::new();
    for (label, pointer, allowed) in manifest_schema_key_contracts() {
        let actual = schema_property_keys(&schema, pointer);
        let expected: BTreeSet<String> = allowed.iter().map(|key| (*key).to_string()).collect();
        if actual != expected {
            errors.push(format!(
                "{label} schema keys drifted from runtime validation: expected {:?}, got {:?}",
                expected, actual
            ));
        }
    }
    Ok(errors)
}

fn schema_property_keys(schema: &Value, pointer: &str) -> BTreeSet<String> {
    schema
        .pointer(pointer)
        .and_then(Value::as_object)
        .map(|properties| properties.keys().cloned().collect())
        .unwrap_or_default()
}

fn validate_self_release_workflow_contract(repo_root: &Path) -> Result<Vec<String>> {
    let mut errors = Vec::new();
    let ci_path = repo_root.join(".github/workflows/ci.yml");
    let release_path = repo_root.join(".github/workflows/release.yml");
    let ci = fs::read_to_string(&ci_path)?;
    let release = fs::read_to_string(&release_path)?;

    let lines: Vec<_> = ci.lines().collect();
    if let Some(sync_line) = lines
        .iter()
        .position(|line| line.contains("cargo run --locked -- check-version-sync"))
    {
        let previous = lines[..sync_line]
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim());
        if previous != Some("git fetch --tags --force origin") {
            errors.push("CI workflow must fetch tags immediately before version sync".into());
        }
    } else {
        errors.push("CI workflow missing version sync step".into());
    }

    if let Some(step) = release.find("name: Synthesize landed release notes") {
        let landed_synthesis = &release[step..];
        if !landed_synthesis.contains("synthesis-required: \"false\"") {
            errors.push("self-release landed synthesis must be non-blocking".into());
        }
    } else {
        errors.push("release workflow missing landed synthesis step".into());
    }

    Ok(errors)
}

fn validate_manifest_schema_contract(readme: &str) -> Vec<String> {
    let required = [
        ".landmark.yml",
        "product:",
        "description:",
        "audience:",
        "voice:",
        "changelog:",
        "source:",
        "artifacts:",
        "markdown:",
        "plaintext:",
        "html:",
        "json:",
        "rss:",
        "release:",
        "profile:",
        "model:",
        "policy:",
        "primary:",
        "fallbacks:",
        "budget:",
        "max_input_tokens:",
        "max_output_tokens:",
        "max_usd:",
        "dist/landmark doctor --repo-root .",
    ];
    required
        .iter()
        .filter(|needle| !readme.contains(**needle))
        .map(|needle| format!("README missing manifest schema token `{needle}`"))
        .chain(
            [
                "cheap, balanced, rich, off",
                "full # full or synthesis-only",
                "Non-empty action inputs still win over manifest values.",
                "cheap` uses `openai/gpt-4o-mini`",
                "synthesize --dry-run-cost",
            ]
            .iter()
            .filter(|needle| !readme.contains(**needle))
            .map(|needle| format!("README missing manifest contract text `{needle}`")),
        )
        .collect()
}

fn validate_manifest_action_precedence_contract(action: &str) -> Vec<String> {
    let required = [
        "id: manifest_defaults",
        "manifest-defaults",
        "inputs.llm-model || steps.manifest_defaults.outputs.llm_model",
        "steps.manifest_defaults.outputs.model_policy",
        "MODEL_POLICY: ${{ steps.manifest_defaults.outputs.model_policy }}",
        "Landmark LLM healthcheck skipped because model.policy=off disables synthesis.",
        "inputs.llm-fallback-models || steps.manifest_defaults.outputs.llm_fallback_models",
        "inputs.audience || steps.manifest_defaults.outputs.audience",
        "inputs.changelog-source || steps.manifest_defaults.outputs.changelog_source",
        "inputs.product-description || steps.manifest_defaults.outputs.product_description",
        "inputs.voice-guide || steps.manifest_defaults.outputs.voice_guide",
        "inputs.notes-output-file || steps.manifest_defaults.outputs.notes_output_file",
        "inputs.notes-output-text-file || steps.manifest_defaults.outputs.notes_output_text_file",
        "inputs.notes-output-html-file || steps.manifest_defaults.outputs.notes_output_html_file",
        "inputs.notes-output-json || steps.manifest_defaults.outputs.notes_output_json",
        "inputs.rss-feed-file || steps.manifest_defaults.outputs.rss_feed_file",
        "--context-metadata-file",
    ];
    required
        .iter()
        .filter(|needle| !action.contains(**needle))
        .map(|needle| format!("action.yml missing manifest precedence expression `{needle}`"))
        .collect()
}

fn validate_landmark_usage_inputs(path: &Path, text: &str, known: &BTreeSet<&str>) -> Vec<String> {
    let mut errors = Vec::new();
    let key_re = Regex::new(r"^\s*([A-Za-z0-9_-]+):").unwrap();
    let mut in_landmark_step = false;
    let mut in_with = false;
    let mut with_indent = 0usize;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("uses:") || trimmed.starts_with("- uses:") {
            in_landmark_step = workflow_invokes_landmark_action(trimmed) || trimmed == "uses: ./";
            in_with = false;
            continue;
        }
        if !in_landmark_step {
            continue;
        }

        let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
        if !in_with {
            if trimmed == "with:" {
                in_with = true;
                with_indent = indent;
            }
            continue;
        }
        if !trimmed.is_empty() && indent <= with_indent {
            in_with = false;
            in_landmark_step = false;
            continue;
        }
        if let Some(caps) = key_re.captures(line) {
            let key = caps.get(1).unwrap().as_str();
            if !known.contains(key) {
                errors.push(format!(
                    "{} references unknown input `{key}`",
                    path.display()
                ));
            }
        }
    }
    errors
}

fn default_contract_scan_paths(repo_root: &Path) -> Vec<PathBuf> {
    let mut paths = vec![repo_root.join("README.md")];
    for dir in ["examples", ".github/workflows"] {
        if let Ok(entries) = fs::read_dir(repo_root.join(dir)) {
            for entry in entries.flatten() {
                paths.push(entry.path());
            }
        }
    }
    paths
}

fn replay_action(args: ReplayArgs) -> Result<()> {
    if !matches!(args.format.as_str(), "text" | "json") {
        return Err("--format must be text or json".into());
    }
    let scenarios = scenario_map();
    let selected: Vec<String> = if args.scenario.is_empty() {
        canonical_scenarios()
            .into_iter()
            .map(str::to_string)
            .collect()
    } else {
        args.scenario.clone()
    };
    for name in &selected {
        if !scenarios.contains_key(name) {
            eprintln!("unknown scenario: {name}");
            std::process::exit(2);
        }
    }
    let evidence_dir = if args.evidence_dir.is_empty() {
        env::temp_dir().join(format!("landmark-replay-{}", std::process::id()))
    } else {
        PathBuf::from(&args.evidence_dir)
    };
    fs::create_dir_all(&evidence_dir)?;
    let tmp_root = env::temp_dir().join(format!("landmark-replay-fixtures-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp_root);
    fs::create_dir_all(&tmp_root)?;
    let mut results = Vec::new();
    for name in selected {
        let scenario = scenarios.get(&name).unwrap();
        match scenario(&tmp_root) {
            Ok(mut value) => {
                value["name"] = Value::String(name);
                value["verdict"] = Value::String("passed".to_string());
                results.push(value);
            }
            Err(error) => {
                results
                    .push(json!({"name": name, "verdict": "failed", "error": error.to_string()}));
            }
        }
    }
    let verdict = if results.iter().all(|result| result["verdict"] == "passed") {
        "passed"
    } else {
        "failed"
    };
    let evidence = json!({
        "verdict": verdict,
        "scenario_count": results.len(),
        "scenarios": results,
    });
    fs::write(
        evidence_dir.join("replay-result.json"),
        serde_json::to_string_pretty(&evidence)? + "\n",
    )?;
    if verdict == "passed" {
        if args.format == "json" {
            println!("{}", serde_json::to_string_pretty(&evidence)?);
        } else {
            println!(
                "replay evidence: {}",
                evidence_dir.join("replay-result.json").display()
            );
        }
        Ok(())
    } else {
        Err("one or more replay scenarios failed".into())
    }
}

type Scenario = fn(&Path) -> Result<Value>;

fn scenario_map() -> BTreeMap<String, Scenario> {
    let mut map: BTreeMap<String, Scenario> = BTreeMap::new();
    map.insert(
        "action_static_contract".to_string(),
        scenario_action_static_contract,
    );
    map.insert(
        "consumer_degraded_required_fails".to_string(),
        scenario_consumer_degraded_required_fails,
    );
    map.insert(
        "degraded-required-fails".to_string(),
        scenario_consumer_degraded_required_fails,
    );
    map.insert(
        "consumer_floating_tag_behavior".to_string(),
        scenario_consumer_floating_tag_behavior,
    );
    map.insert(
        "consumer_full_mode_success".to_string(),
        scenario_consumer_full_mode_success,
    );
    map.insert(
        "full-semantic-release".to_string(),
        scenario_consumer_full_mode_success,
    );
    map.insert(
        "consumer_release_update_failure".to_string(),
        scenario_consumer_release_update_failure,
    );
    map.insert(
        "release-body-fallback".to_string(),
        scenario_consumer_release_update_failure,
    );
    map.insert(
        "consumer_synthesis_only_success".to_string(),
        scenario_consumer_synthesis_only_success,
    );
    map.insert(
        "manifest_defaults_and_overrides".to_string(),
        scenario_manifest_defaults_and_overrides,
    );
    map.insert(
        "action_manifest_defaults_precedence".to_string(),
        scenario_action_manifest_defaults_precedence,
    );
    map.insert(
        "local_provider_run".to_string(),
        scenario_local_provider_run,
    );
    map.insert(
        "first_run_local_preview".to_string(),
        scenario_first_run_local_preview,
    );
    map.insert(
        "github_provider_run".to_string(),
        scenario_github_provider_run,
    );
    map.insert(
        "provider_run_parity".to_string(),
        scenario_provider_run_parity,
    );
    map.insert(
        "fleet_adoption_planner".to_string(),
        scenario_fleet_adoption_planner,
    );
    map.insert(
        "self_release_pr_path".to_string(),
        scenario_self_release_pr_path,
    );
    map.insert(
        "synthesis_cost_policy".to_string(),
        scenario_synthesis_cost_policy,
    );
    map.insert(
        "backfill_release_history".to_string(),
        scenario_backfill_release_history,
    );
    map.insert(
        "agent_native_contracts".to_string(),
        scenario_agent_native_contracts,
    );
    map.insert(
        "http_resilience_policy".to_string(),
        scenario_http_resilience_policy,
    );
    map.insert(
        "action_side_effect_coverage".to_string(),
        scenario_action_side_effect_coverage,
    );
    map.insert(
        "synthesis-only-success".to_string(),
        scenario_consumer_synthesis_only_success,
    );
    map.insert(
        "publication_degraded_optional".to_string(),
        scenario_publication_degraded_optional,
    );
    map.insert(
        "publication_degraded_required".to_string(),
        scenario_publication_degraded_required,
    );
    map.insert(
        "summary_artifact_failed".to_string(),
        scenario_summary_artifact_failed,
    );
    map.insert(
        "summary_release_update_failed".to_string(),
        scenario_summary_release_update_failed,
    );
    map.insert(
        "summary_rss_failed".to_string(),
        scenario_summary_rss_failed,
    );
    map
}

fn canonical_scenarios() -> Vec<&'static str> {
    vec![
        "action_static_contract",
        "action_manifest_defaults_precedence",
        "consumer_degraded_required_fails",
        "consumer_floating_tag_behavior",
        "consumer_full_mode_success",
        "fleet_adoption_planner",
        "first_run_local_preview",
        "github_provider_run",
        "local_provider_run",
        "provider_run_parity",
        "manifest_defaults_and_overrides",
        "consumer_release_update_failure",
        "consumer_synthesis_only_success",
        "self_release_pr_path",
        "synthesis_cost_policy",
        "backfill_release_history",
        "publication_degraded_optional",
        "publication_degraded_required",
        "summary_artifact_failed",
        "summary_release_update_failed",
        "summary_rss_failed",
        "agent_native_contracts",
        "http_resilience_policy",
        "action_side_effect_coverage",
    ]
}

fn scenario_http_resilience_policy(_: &Path) -> Result<Value> {
    let token = "ghp_123456789abcdef";
    let webhook_url = "https://hooks.slack.invalid/services/T000/B000/secret";
    let invocation = build_curl_invocation(
        "POST",
        webhook_url,
        Some(token),
        Some(&json!({"ok": true})),
        HttpPolicy::default(),
    );
    let argv = invocation.args.join(" ");
    if argv.contains(token) || argv.contains(webhook_url) || argv.contains("Authorization") {
        return Err("curl argv contains secret-bearing request data".into());
    }
    if !invocation.config.contains(token) || !invocation.config.contains(webhook_url) {
        return Err("curl stdin config missing request secret data".into());
    }
    let redacted = redact_secret_values(
        &format!("token={token} slack={webhook_url} short=abc123"),
        [
            token.to_string(),
            webhook_url.to_string(),
            "abc123".to_string(),
        ],
    );
    if redacted.contains(token) || redacted.contains(webhook_url) {
        return Err("configured secret redaction left a secret in evidence text".into());
    }
    if !redacted.contains("short=abc123") {
        return Err("configured secret redaction removed a short non-secret value".into());
    }

    let mut retry_429 = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.into(),
        ..FakeState::default()
    };
    retry_429.llm_responses.push_back((429, String::new()));
    retry_429.llm_responses.push_back((200, VALID_NOTES.into()));
    let retry_429_server = start_fake_server(retry_429)?;
    let retry_429_response = curl_json_with_policy(
        "POST",
        &format!("{}/chat/completions", retry_429_server.url),
        Some(token),
        Some(&json!({"messages": []})),
        HttpPolicy {
            retry_delay_ms: 1,
            ..HttpPolicy::default()
        },
    )?;
    if retry_429_response.status != 200 {
        return Err("429 retry did not recover to HTTP 200".into());
    }
    let retry_429_requests = retry_429_server.state.lock().unwrap().requests.len();
    if retry_429_requests != 2 {
        return Err(format!("429 retry expected 2 requests, got {retry_429_requests}").into());
    }

    let mut retry_500 = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.into(),
        ..FakeState::default()
    };
    retry_500.llm_responses.push_back((500, String::new()));
    retry_500.llm_responses.push_back((200, VALID_NOTES.into()));
    let retry_500_server = start_fake_server(retry_500)?;
    let retry_500_response = curl_json_with_policy(
        "POST",
        &format!("{}/chat/completions", retry_500_server.url),
        Some(token),
        Some(&json!({"messages": []})),
        HttpPolicy {
            retry_delay_ms: 1,
            ..HttpPolicy::default()
        },
    )?;
    if retry_500_response.status != 200 {
        return Err("5xx retry did not recover to HTTP 200".into());
    }

    let slow_url = start_slow_http_server(Duration::from_millis(1500))?;
    let slow = curl_json_with_policy(
        "GET",
        &slow_url,
        None,
        None,
        HttpPolicy {
            connect_timeout_seconds: 1,
            max_time_seconds: 1,
            attempts: 1,
            retry_delay_ms: 1,
        },
    );
    if slow.is_ok() {
        return Err("slow provider request unexpectedly succeeded".into());
    }
    let slow_error = redact_known_secrets(&slow.unwrap_err().to_string());
    if slow_error.contains(token) {
        return Err("timeout error leaked token-like content".into());
    }

    Ok(json!({
        "argv_secret_free": true,
        "retry_429_requests": retry_429_requests,
        "retry_500_status": retry_500_response.status,
        "configured_secret_redaction": true,
        "slow_timeout": true
    }))
}

fn scenario_action_side_effect_coverage(_: &Path) -> Result<Value> {
    let action = fs::read_to_string("action.yml")?;
    let invoked = action_landmark_subcommands(&action);
    let coverage = action_subcommand_replay_coverage();
    let scenarios = scenario_map();
    let mut missing = Vec::new();
    for command in &invoked {
        match coverage.get(command.as_str()) {
            Some(names) if names.iter().all(|name| scenarios.contains_key(*name)) => {}
            Some(_) => missing.push(format!("{command}: coverage references unknown scenario")),
            None => missing.push(format!("{command}: no replay coverage mapping")),
        }
    }
    if !missing.is_empty() {
        return Err(missing.join("\n").into());
    }
    Ok(json!({
        "covered_commands": invoked,
        "coverage": coverage
    }))
}

fn action_landmark_subcommands(action: &str) -> BTreeSet<String> {
    let re = Regex::new(r#"dist/landmark"\s+([a-z][a-z-]*)(?:\s+([a-z][a-z-]*))?"#).unwrap();
    re.captures_iter(action)
        .map(|caps| {
            let command = caps.get(1).unwrap().as_str();
            let nested = caps.get(2).map(|value| value.as_str()).unwrap_or("");
            if command == "fleet" || command == "release-policy" {
                format!("{command} {nested}")
            } else {
                command.to_string()
            }
        })
        .collect()
}

fn action_subcommand_replay_coverage() -> BTreeMap<&'static str, Vec<&'static str>> {
    BTreeMap::from([
        (
            "manifest-defaults",
            vec![
                "manifest_defaults_and_overrides",
                "action_manifest_defaults_precedence",
            ],
        ),
        ("healthcheck", vec!["http_resilience_policy"]),
        ("preflight-tags", vec!["action_side_effect_coverage"]),
        ("fetch-release-body", vec!["consumer_full_mode_success"]),
        ("extract-prs", vec!["consumer_full_mode_success"]),
        (
            "synthesize",
            vec!["synthesis_cost_policy", "consumer_synthesis_only_success"],
        ),
        (
            "release-policy publication",
            vec![
                "publication_degraded_optional",
                "publication_degraded_required",
            ],
        ),
        (
            "release-policy summary",
            vec![
                "summary_artifact_failed",
                "summary_release_update_failed",
                "summary_rss_failed",
            ],
        ),
        (
            "run",
            vec![
                "local_provider_run",
                "github_provider_run",
                "provider_run_parity",
            ],
        ),
        ("notify-webhook", vec!["http_resilience_policy"]),
        ("notify-slack", vec!["http_resilience_policy"]),
        ("floating-tag", vec!["consumer_floating_tag_behavior"]),
        (
            "close-resolved-failures",
            vec!["consumer_release_update_failure"],
        ),
        (
            "report-synthesis-failure",
            vec!["consumer_degraded_required_fails"],
        ),
    ])
}

fn scenario_agent_native_contracts(tmp_root: &Path) -> Result<Value> {
    let repo_root = env::current_dir()?;
    let schema_errors = validate_agent_native_contracts(&repo_root)?;
    if !schema_errors.is_empty() {
        return Err(schema_errors.join("\n").into());
    }

    let describe = Command::new(current_exe())
        .args(["describe", "--json"])
        .output()?;
    if !describe.status.success() {
        return Err(format!(
            "describe --json failed: {}",
            String::from_utf8_lossy(&describe.stderr)
        )
        .into());
    }
    let describe_json: Value = serde_json::from_slice(&describe.stdout)?;
    assert_json_eq(
        &describe_json,
        "/schema_version",
        "landmark.describe.v1",
        "describe schema version",
    )?;
    let schema_paths: BTreeSet<String> = describe_json["schemas"]
        .as_array()
        .ok_or("describe schemas must be an array")?
        .iter()
        .filter_map(|schema| schema["path"].as_str().map(str::to_string))
        .collect();
    for descriptor in schema_descriptors() {
        if !schema_paths.contains(descriptor.path) {
            return Err(format!("describe missing schema {}", descriptor.path).into());
        }
    }
    let contracts = describe_json["contracts"]
        .as_array()
        .ok_or("describe contracts must be an array")?;
    let run_contract = contracts
        .iter()
        .find(|contract| contract["command"] == "run")
        .ok_or("describe missing run contract")?;
    assert_json_eq(
        run_contract,
        "/preview",
        "--dry-run computes evidence without writing artifacts or mutating releases",
        "run preview",
    )?;

    let invalid = Command::new(current_exe())
        .args([
            "--error-format",
            "json",
            "run",
            "--provider",
            "unsupported",
            "--dry-run",
        ])
        .output()?;
    if invalid.status.success() {
        return Err("invalid provider unexpectedly succeeded".into());
    }
    let failure: Value = serde_json::from_slice(&invalid.stderr)?;
    assert_json_eq(
        &failure,
        "/error/code",
        "invalid_input",
        "invalid provider error code",
    )?;
    assert_json_eq(
        &failure,
        "/error/stage",
        "configuration",
        "invalid provider error stage",
    )?;
    if failure["error"]["context"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("ghp_")
    {
        return Err("failure context leaked token-like content".into());
    }

    let repo = tmp_root.join("agent-native-run");
    init_fixture_repo(&repo, "v1.0.0")?;
    fs::write(repo.join("feature.txt"), "feature\n")?;
    run_ok("git", ["add", "feature.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat: agent native run"],
        &repo,
    )?;
    let dry_run = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "local",
            "--repo-root",
            repo.to_str().unwrap(),
            "--dry-run",
            "--output-file",
            "docs/releases/{version}.md",
            "--output-json",
            "docs/releases/releases.json",
            "--rss-feed-file",
            "docs/releases/feed.xml",
        ])
        .output()?;
    if !dry_run.status.success() {
        return Err(format!(
            "run --dry-run failed: {}",
            String::from_utf8_lossy(&dry_run.stderr)
        )
        .into());
    }
    let evidence: Value = serde_json::from_slice(&dry_run.stdout)?;
    assert_json_eq(&evidence, "/provider", "local", "dry-run provider")?;
    assert_json_eq(
        &evidence,
        "/publication/status",
        "dry-run; release-body publication previewed but not mutated",
        "dry-run publication status",
    )?;
    if repo.join("docs/releases/releases.json").exists() {
        return Err("run --dry-run wrote JSON artifact".into());
    }

    let backfill = Command::new(current_exe())
        .args([
            "backfill",
            "--repo-root",
            repo.to_str().unwrap(),
            "--dry-run",
            "--max-tags",
            "1",
            "--output-json",
            "docs/releases/releases.json",
            "--rss-feed-file",
            "docs/releases/feed.xml",
        ])
        .output()?;
    if !backfill.status.success() {
        return Err(format!(
            "backfill --dry-run failed: {}",
            String::from_utf8_lossy(&backfill.stderr)
        )
        .into());
    }
    let backfill_manifest: Value = serde_json::from_slice(&backfill.stdout)?;
    if backfill_manifest["dry_run"] != json!(true) {
        return Err("backfill --dry-run did not emit dry_run=true".into());
    }

    let fleet_fixture_path = tmp_root.join("agent-native-fleet.json");
    let fleet_scan_path = tmp_root.join("agent-native-fleet-scan.json");
    let fleet_plan_dir = tmp_root.join("agent-native-fleet-plan");
    let fleet_pr_dir = fleet_plan_dir.join("prs");
    let fleet_fixture = FleetScan {
        generated_at: "2026-06-15T00:00:00Z".into(),
        owners: vec!["phrazzld".into()],
        repositories: vec![fleet_fixture_repo(
            "phrazzld/semantic-app",
            "semantic-release",
            ("application", "github-release+semantic-release"),
            (false, false),
            "unprotected-or-unavailable",
            &[],
            &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
        )],
        warnings: Vec::new(),
    };
    fs::write(
        &fleet_fixture_path,
        serde_json::to_string_pretty(&fleet_fixture)? + "\n",
    )?;
    let fleet_scan = Command::new(current_exe())
        .args([
            "fleet",
            "scan",
            "--fixture",
            fleet_fixture_path.to_str().unwrap(),
            "--output",
            fleet_scan_path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()?;
    if !fleet_scan.status.success() {
        return Err(format!(
            "fleet scan --format json failed: {}",
            String::from_utf8_lossy(&fleet_scan.stderr)
        )
        .into());
    }
    let fleet_scan_json: Value = serde_json::from_slice(&fleet_scan.stdout)?;
    if fleet_scan_json["repositories"].as_array().map(Vec::len) != Some(1) {
        return Err("fleet scan JSON stdout did not include one repository".into());
    }
    let fleet_plan = Command::new(current_exe())
        .args([
            "fleet",
            "plan",
            "--input",
            fleet_scan_path.to_str().unwrap(),
            "--output-dir",
            fleet_plan_dir.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()?;
    if !fleet_plan.status.success() {
        return Err(format!(
            "fleet plan --format json failed: {}",
            String::from_utf8_lossy(&fleet_plan.stderr)
        )
        .into());
    }
    let fleet_plan_json: Value = serde_json::from_slice(&fleet_plan.stdout)?;
    if fleet_plan_json["repositories"].as_array().map(Vec::len) != Some(1) {
        return Err("fleet plan JSON stdout did not include one repository".into());
    }
    let fleet_prs = Command::new(current_exe())
        .args([
            "fleet",
            "open-prs",
            "--plan-dir",
            fleet_plan_dir.to_str().unwrap(),
            "--output-dir",
            fleet_pr_dir.to_str().unwrap(),
            "--dry-run",
            "--format",
            "json",
        ])
        .output()?;
    if !fleet_prs.status.success() {
        return Err(format!(
            "fleet open-prs --format json failed: {}",
            String::from_utf8_lossy(&fleet_prs.stderr)
        )
        .into());
    }
    let fleet_prs_json: Value = serde_json::from_slice(&fleet_prs.stdout)?;
    if fleet_prs_json["dry_run"] != json!(true) {
        return Err("fleet open-prs JSON stdout did not include dry_run=true".into());
    }

    Ok(json!({
        "describe_schemas": schema_paths.len(),
        "backfill_dry_run": backfill_manifest["dry_run"],
        "json_error_code": failure["error"]["code"],
        "dry_run_release_tag": evidence["release_tag"],
        "dry_run_artifacts": evidence["artifacts"],
        "fleet_json_paths": {
            "scan_repositories": fleet_scan_json["repositories"].as_array().map(Vec::len).unwrap_or(0),
            "plan_repositories": fleet_plan_json["repositories"].as_array().map(Vec::len).unwrap_or(0),
            "prs_dry_run": fleet_prs_json["dry_run"]
        }
    }))
}

fn assert_json_eq(value: &Value, pointer: &str, expected: &str, label: &str) -> Result<()> {
    let actual = value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{label} missing JSON string at {pointer}"))?;
    if actual != expected {
        return Err(format!("{label} expected `{expected}`, got `{actual}`").into());
    }
    Ok(())
}

fn scenario_backfill_release_history(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("backfill-release-history");
    init_fixture_repo(&repo, "v1.0.0")?;
    fs::write(
        repo.join("CHANGELOG.md"),
        "## [1.4.0]\n\n- feat(pkg-a): package artifact history\n\n## [1.3.0]\n\n- feat: first duplicate section\n\n## [1.3.0]\n\n- fix: second duplicate section\n\n## [1.1.0]\n\n- feat: historical changelog entry\n\n## [1.0.0]\n\n- feat: initial release\n",
    )?;
    run_ok("git", ["add", "CHANGELOG.md"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "docs: expand historical changelog"],
        &repo,
    )?;
    run_ok("git", ["tag", "v1.1.0"], &repo)?;
    fs::write(repo.join("beta.txt"), "beta\n")?;
    run_ok("git", ["add", "beta.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat: prerelease beta"],
        &repo,
    )?;
    run_ok("git", ["tag", "v1.2.0-beta.1"], &repo)?;
    fs::write(repo.join("managed.txt"), "managed\n")?;
    run_ok("git", ["add", "managed.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat: already managed release"],
        &repo,
    )?;
    run_ok("git", ["tag", "v1.2.0"], &repo)?;
    fs::write(repo.join("duplicate.txt"), "duplicate\n")?;
    run_ok("git", ["add", "duplicate.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat: duplicate changelog release"],
        &repo,
    )?;
    run_ok("git", ["tag", "v1.3.0"], &repo)?;
    fs::create_dir_all(repo.join("packages/pkg-a"))?;
    fs::write(repo.join("packages/pkg-a/README.md"), "# pkg-a\n")?;
    run_ok("git", ["add", "packages/pkg-a/README.md"], &repo)?;
    run_ok(
        "git",
        [
            "commit",
            "-q",
            "-m",
            "feat(pkg-a): monorepo package release",
        ],
        &repo,
    )?;
    run_ok("git", ["tag", "pkg-a@v1.4.0"], &repo)?;
    fs::write(repo.join("existing-body.txt"), "existing body\n")?;
    run_ok("git", ["add", "existing-body.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat: existing release body source"],
        &repo,
    )?;
    run_ok("git", ["tag", "v1.5.0"], &repo)?;

    let mut fake = FakeState {
        update_status: 200,
        ..Default::default()
    };
    fake.releases.insert("v1.2.0".to_string(), json!({"id": 2, "tag_name": "v1.2.0", "body": "## What's New\n\n- Managed already\n\n## Technical\n\n- Existing", "html_url": "https://example.invalid/releases/v1.2.0"}));
    fake.releases.insert("v1.3.0".to_string(), json!({"id": 3, "tag_name": "v1.3.0", "body": "", "html_url": "https://example.invalid/releases/v1.3.0"}));
    fake.releases.insert("pkg-a@v1.4.0".to_string(), json!({"id": 4, "tag_name": "pkg-a@v1.4.0", "body": "", "html_url": "https://example.invalid/releases/pkg-a@v1.4.0"}));
    fake.releases.insert("v1.5.0".to_string(), json!({"id": 5, "tag_name": "v1.5.0", "body": "## Technical\n\n- Existing release-body source", "html_url": "https://example.invalid/releases/v1.5.0"}));
    let server = start_fake_server(fake)?;

    let dry_run = Command::new(current_exe())
        .args([
            "backfill",
            "--repo-root",
            repo.to_str().unwrap(),
            "--since",
            "v1.0.0",
            "--dry-run",
            "--repository",
            "owner/repo",
            "--github-token",
            "token",
            "--api-base-url",
            &server.url,
        ])
        .output()?;
    if !dry_run.status.success() {
        return Err(String::from_utf8_lossy(&dry_run.stderr).to_string().into());
    }
    let dry_manifest: Value = serde_json::from_slice(&dry_run.stdout)?;
    let skipped = dry_manifest["skipped_tags"].as_array().unwrap();
    if !skipped.iter().any(|entry| entry["tag"] == "v1.2.0-beta.1")
        || !skipped.iter().any(|entry| entry["tag"] == "v1.2.0")
    {
        return Err("dry-run did not report prerelease and already-managed skips".into());
    }
    if !dry_manifest["processed_tags"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["tag"] == "pkg-a@v1.4.0")
    {
        return Err("dry-run did not include monorepo package tag".into());
    }
    if dry_manifest["estimated_cost"]["llm_calls"] != 0 {
        return Err("backfill dry-run should not schedule LLM calls".into());
    }
    if !dry_manifest["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| {
            entry["tag"] == "v1.1.0"
                && entry["markdown"]
                    .as_str()
                    .unwrap_or("")
                    .ends_with("docs/releases/v1.1.0.md")
        })
    {
        return Err("dry-run did not include artifact path plan".into());
    }
    if !dry_manifest["processed_tags"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["tag"] == "v1.1.0" && entry["release_status"] == "missing")
    {
        return Err("dry-run did not preserve missing release status".into());
    }

    let artifact_run = Command::new(current_exe())
        .args([
            "backfill",
            "--repo-root",
            repo.to_str().unwrap(),
            "--since",
            "v1.0.0",
            "--mode",
            "artifacts-only",
            "--repository",
            "owner/repo",
            "--github-token",
            "token",
            "--api-base-url",
            &server.url,
        ])
        .output()?;
    if !artifact_run.status.success() {
        return Err(String::from_utf8_lossy(&artifact_run.stderr)
            .to_string()
            .into());
    }
    if !repo.join("docs/releases/v1.1.0.md").is_file()
        || !repo.join("docs/releases/v1.1.0.txt").is_file()
        || !repo.join("docs/releases/v1.1.0.html").is_file()
        || repo.join("docs/releases/v1.3.0.md").is_file()
        || !repo.join("docs/releases/releases.json").is_file()
        || !repo.join("docs/releases/feed.xml").is_file()
        || !repo.join(".landmark/backfill-manifest.json").is_file()
    {
        return Err("artifact-only backfill did not write the expected artifact set or wrote an ambiguous duplicate".into());
    }

    let release_body_preview = Command::new(current_exe())
        .args([
            "backfill",
            "--repo-root",
            repo.to_str().unwrap(),
            "--since",
            "v1.2.0",
            "--mode",
            "release-body",
            "--dry-run",
            "--repository",
            "owner/repo",
            "--github-token",
            "token",
            "--api-base-url",
            &server.url,
        ])
        .output()?;
    if !release_body_preview.status.success() {
        return Err(String::from_utf8_lossy(&release_body_preview.stderr)
            .to_string()
            .into());
    }
    let release_manifest: Value = serde_json::from_slice(&release_body_preview.stdout)?;
    if !release_manifest["skipped_tags"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| {
            entry["tag"] == "v1.3.0"
                && entry["reason"]
                    .as_str()
                    .unwrap_or("")
                    .contains("duplicate changelog")
        })
    {
        return Err("release-body dry-run did not refuse duplicate changelog mapping".into());
    }
    if !release_manifest["skipped_tags"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| {
            entry["tag"] == "v1.5.0"
                && entry["reason"]
                    .as_str()
                    .unwrap_or("")
                    .contains("refusing to duplicate")
        })
    {
        return Err("release-body dry-run did not refuse existing-body source duplication".into());
    }
    if !release_manifest["release_body_updates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["tag"] == "pkg-a@v1.4.0" && entry["dry_run"] == true)
    {
        return Err("release-body dry-run did not preview package tag update".into());
    }

    let confirmed_update = Command::new(current_exe())
        .args([
            "backfill",
            "--repo-root",
            repo.to_str().unwrap(),
            "--since",
            "v1.3.0",
            "--mode",
            "release-body",
            "--confirm-release-body",
            "--repository",
            "owner/repo",
            "--github-token",
            "token",
            "--api-base-url",
            &server.url,
        ])
        .output()?;
    if !confirmed_update.status.success() {
        return Err(String::from_utf8_lossy(&confirmed_update.stderr)
            .to_string()
            .into());
    }
    let updated_body = server
        .state
        .lock()
        .unwrap()
        .releases
        .get("pkg-a@v1.4.0")
        .and_then(|release| release["body"].as_str())
        .unwrap_or("")
        .to_string();
    if !updated_body.contains("## What's New") || !updated_body.contains("package artifact history")
    {
        return Err("confirmed release-body update did not patch fake GitHub release".into());
    }

    let empty_template_preview = Command::new(current_exe())
        .args([
            "backfill",
            "--repo-root",
            repo.to_str().unwrap(),
            "--since",
            "v1.3.0",
            "--dry-run",
            "--output-text-file",
            "",
            "--output-html-file",
            "",
            "--output-json",
            "",
            "--rss-feed-file",
            "",
        ])
        .output()?;
    if !empty_template_preview.status.success() {
        return Err(String::from_utf8_lossy(&empty_template_preview.stderr)
            .to_string()
            .into());
    }
    let empty_template_manifest: Value = serde_json::from_slice(&empty_template_preview.stdout)?;
    if !empty_template_manifest["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| {
            entry["tag"] == "pkg-a@v1.4.0"
                && entry["plaintext"] == ""
                && entry["html"] == ""
                && entry["json"] == ""
                && entry["rss"] == ""
        })
    {
        return Err("empty artifact templates were not treated as disabled outputs".into());
    }

    let missing_since = Command::new(current_exe())
        .args([
            "backfill",
            "--repo-root",
            repo.to_str().unwrap(),
            "--since",
            "v9.9.9",
            "--dry-run",
        ])
        .output()?;
    if !missing_since.status.success() {
        return Err(String::from_utf8_lossy(&missing_since.stderr)
            .to_string()
            .into());
    }
    let missing_manifest: Value = serde_json::from_slice(&missing_since.stdout)?;
    if missing_manifest["skipped_tags"][0]["reason"] != "since tag not found" {
        return Err("missing since tag was not reported as a skip reason".into());
    }

    let private_preview = Command::new(current_exe())
        .args([
            "backfill",
            "--repo-root",
            repo.to_str().unwrap(),
            "--since",
            "v1.3.0",
            "--dry-run",
            "--repository",
            "owner/private",
        ])
        .output()?;
    if !private_preview.status.success() {
        return Err(String::from_utf8_lossy(&private_preview.stderr)
            .to_string()
            .into());
    }
    let private_manifest: Value = serde_json::from_slice(&private_preview.stdout)?;
    if private_manifest["processed_tags"][0]["release_status"]
        .as_str()
        .unwrap_or("")
        .contains("github token not configured")
    {
        Ok(json!({
            "dry_run": dry_manifest,
            "artifact_manifest": serde_json::from_slice::<Value>(&artifact_run.stdout)?,
            "release_body_preview": release_manifest,
            "confirmed_update": serde_json::from_slice::<Value>(&confirmed_update.stdout)?,
            "empty_template_preview": empty_template_manifest,
            "missing_since": missing_manifest,
            "private_preview": private_manifest,
        }))
    } else {
        Err("private/no-token release lookup was not reported".into())
    }
}

fn scenario_action_static_contract(_: &Path) -> Result<Value> {
    let action = fs::read_to_string("action.yml")?;
    if action.contains("python ") || action.contains("setup-python") {
        return Err("action.yml still invokes Python".into());
    }
    if !action.contains("dist/landmark") {
        return Err("action.yml does not invoke dist/landmark".into());
    }
    if !action.contains("dist/landmark\" run")
        || !action.contains("--provider github")
        || !action.contains("--release-tag \"${RELEASE_TAG}\"")
    {
        return Err("action.yml does not invoke the provider-neutral run command".into());
    }
    if action.contains("dist/landmark\" update-release") {
        return Err("action.yml still mutates GitHub releases through update-release".into());
    }
    if action.contains("dist/landmark\" write-artifacts")
        || action.contains("dist/landmark\" update-feed")
    {
        return Err("action.yml still writes release artifacts outside the run command".into());
    }
    Ok(json!({"checked": ["action.yml", "provider-neutral run"]}))
}

fn scenario_action_manifest_defaults_precedence(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("action-manifest-defaults");
    fs::create_dir_all(&repo)?;
    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Manifest Product
  description: Manifest description
audience: enterprise
voice: Manifest voice
changelog:
  source: prs
artifacts:
  markdown: docs/releases/{version}.md
  plaintext: docs/releases/{version}.txt
  html: docs/releases/{version}.html
  json: docs/releases/releases.json
  rss: docs/releases/feed.xml
model:
  policy: cheap
  fallbacks:
    - manifest/fallback
"#,
    )?;
    let output = temp_file("landmark-manifest-defaults")?;
    let result = Command::new(current_exe())
        .args([
            "manifest-defaults",
            "--repo-root",
            repo.to_str().unwrap(),
            "--github-output",
        ])
        .arg(&output)
        .output()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).to_string().into());
    }
    let defaults = parse_outputs(&output)?;
    let effective_audience = action_value("", defaults.get("audience"), "general");
    let explicit_audience = action_value("developer", defaults.get("audience"), "general");
    let effective_model = action_value("", defaults.get("llm_model"), "anthropic/claude-sonnet-4");
    let explicit_model = action_value(
        "explicit/model",
        defaults.get("llm_model"),
        "anthropic/claude-sonnet-4",
    );
    let effective_markdown = action_value("", defaults.get("notes_output_file"), "");
    let explicit_markdown = action_value("docs/explicit.md", defaults.get("notes_output_file"), "");

    if effective_audience != "enterprise"
        || explicit_audience != "developer"
        || effective_model != "openai/gpt-4o-mini"
        || explicit_model != "explicit/model"
        || effective_markdown != "docs/releases/{version}.md"
        || explicit_markdown != "docs/explicit.md"
    {
        return Err("manifest default precedence did not match action semantics".into());
    }

    Ok(json!({
        "checked": [
            "manifest-defaults github output",
            "empty action input uses manifest default",
            "explicit action input overrides manifest default",
            "model.policy cheap selects openai/gpt-4o-mini"
        ],
        "defaults": defaults,
        "effective": {
            "audience": effective_audience,
            "llm_model": effective_model,
            "notes_output_file": effective_markdown,
        },
        "explicit": {
            "audience": explicit_audience,
            "llm_model": explicit_model,
            "notes_output_file": explicit_markdown,
        }
    }))
}

fn action_value(input: &str, manifest: Option<&String>, fallback: &str) -> String {
    trimmed_option(input)
        .or_else(|| manifest.and_then(|value| trimmed_option(value)))
        .unwrap_or_else(|| fallback.to_string())
}

fn scenario_publication_degraded_required(_: &Path) -> Result<Value> {
    let output = temp_file("landmark-policy")?;
    let result = Command::new(current_exe())
        .args([
            "release-policy",
            "publication",
            "--synthesis-required",
            "true",
            "--synthesis-strict",
            "false",
            "--synth-succeeded",
            "true",
            "--synth-quality",
            "degraded",
            "--github-output",
        ])
        .arg(&output)
        .output()?;
    if result.status.success() {
        return Err("degraded required synthesis should fail".into());
    }
    Ok(json!({"outputs": parse_outputs(&output)?}))
}

fn scenario_publication_degraded_optional(_: &Path) -> Result<Value> {
    let output = temp_file("landmark-policy")?;
    let result = Command::new(current_exe())
        .args([
            "release-policy",
            "publication",
            "--synthesis-required",
            "false",
            "--synthesis-strict",
            "false",
            "--synth-succeeded",
            "true",
            "--synth-quality",
            "degraded",
            "--github-output",
        ])
        .arg(&output)
        .output()?;
    if !result.status.success() {
        return Err("degraded optional synthesis should pass".into());
    }
    Ok(json!({"outputs": parse_outputs(&output)?}))
}

fn scenario_summary_release_update_failed(_: &Path) -> Result<Value> {
    scenario_summary_failure("release_update", "update failed")
}

fn scenario_summary_artifact_failed(_: &Path) -> Result<Value> {
    scenario_summary_failure("artifact_write", "artifact failed")
}

fn scenario_summary_rss_failed(_: &Path) -> Result<Value> {
    scenario_summary_failure("rss_update", "rss failed")
}

fn scenario_summary_failure(stage: &str, message: &str) -> Result<Value> {
    let output = temp_file("landmark-summary")?;
    let result = Command::new(current_exe())
        .args([
            "release-policy",
            "summary",
            "--synthesis-enabled",
            "true",
            "--released",
            "true",
            "--synth-succeeded",
            "true",
            "--synth-quality",
            "valid",
            "--update-succeeded",
            if stage == "release_update" {
                "false"
            } else {
                "true"
            },
            "--update-failure-stage",
            if stage == "release_update" { stage } else { "" },
            "--update-failure-message",
            if stage == "release_update" {
                message
            } else {
                ""
            },
            "--artifact-succeeded",
            if stage == "artifact_write" {
                "false"
            } else {
                "true"
            },
            "--artifact-failure-stage",
            if stage == "artifact_write" { stage } else { "" },
            "--artifact-failure-message",
            if stage == "artifact_write" {
                message
            } else {
                ""
            },
            "--rss-enabled",
            if stage == "rss_update" {
                "true"
            } else {
                "false"
            },
            "--rss-succeeded",
            if stage == "rss_update" {
                "false"
            } else {
                "true"
            },
            "--rss-failure-stage",
            if stage == "rss_update" { stage } else { "" },
            "--rss-failure-message",
            if stage == "rss_update" { message } else { "" },
            "--github-output",
        ])
        .arg(&output)
        .output()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).to_string().into());
    }
    Ok(json!({"outputs": parse_outputs(&output)?}))
}

#[derive(Default)]
struct FakeState {
    llm_status: u16,
    llm_notes: String,
    llm_responses: VecDeque<(u16, String)>,
    update_status: u16,
    releases: BTreeMap<String, Value>,
    requests: Vec<Value>,
}

fn scenario_consumer_full_mode_success(tmp_root: &Path) -> Result<Value> {
    consumer_success(tmp_root, "consumer-full", true)
}

fn scenario_consumer_synthesis_only_success(tmp_root: &Path) -> Result<Value> {
    consumer_success(tmp_root, "consumer-synthesis-only", false)
}

fn scenario_first_run_local_preview(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("first-run-local-preview");
    init_fixture_repo(&repo, "v0.1.0")?;
    fs::write(repo.join("README.md"), "# First Run Demo\n")?;
    fs::write(repo.join("feature.txt"), "first run adoption\n")?;
    run_ok("git", ["add", "README.md", "feature.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "fix(cli): make first run obvious"],
        &repo,
    )?;

    let result = Command::new(current_exe())
        .args(["run", "--provider", "local", "--repo-root"])
        .arg(&repo)
        .output()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).to_string().into());
    }
    let stdout_evidence: Value = serde_json::from_slice(&result.stdout)?;
    let evidence_path = repo.join(".landmark/run/evidence.json");
    let evidence: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    if stdout_evidence != evidence {
        return Err("first-run preview stdout did not match written evidence packet".into());
    }
    if evidence["provider"] != "local"
        || evidence["publication"]["release_body_updated"] != false
        || evidence["version_decision"]["bump"] != "patch"
        || evidence["release_tag"] != "v0.1.1"
    {
        return Err("first-run preview evidence did not record local patch preview".into());
    }
    let expected = [
        repo.join(".landmark/run/technical-changelog.md"),
        repo.join(".landmark/run/evidence.json"),
        repo.join("docs/releases/v0.1.1.md"),
        repo.join("docs/releases/v0.1.1.txt"),
        repo.join("docs/releases/v0.1.1.html"),
        repo.join("docs/releases/releases.json"),
        repo.join("docs/releases/feed.xml"),
    ];
    for path in expected {
        if !path.is_file() {
            return Err(format!("first-run preview did not write {}", path.display()).into());
        }
    }
    let notes = fs::read_to_string(repo.join("docs/releases/v0.1.1.md"))?;
    let technical = fs::read_to_string(repo.join(".landmark/run/technical-changelog.md"))?;
    if !notes.contains("Make first run obvious")
        || !technical.contains("fix(cli): make first run obvious")
    {
        return Err("first-run preview artifacts did not include release context".into());
    }
    Ok(json!({
        "release_tag": evidence["release_tag"],
        "provider": evidence["provider"],
        "evidence": evidence_path,
        "markdown": repo.join("docs/releases/v0.1.1.md"),
        "technical_changelog": repo.join(".landmark/run/technical-changelog.md")
    }))
}

fn scenario_local_provider_run(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("local-provider-run");
    init_fixture_repo(&repo, "v1.0.0")?;
    fs::write(repo.join("feature.txt"), "portable release\n")?;
    run_ok("git", ["add", "feature.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat(cli): add portable release run"],
        &repo,
    )?;
    let result = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "local",
            "--repo-root",
            repo.to_str().unwrap(),
            "--repository",
            "local-provider-run",
            "--output-dir",
            ".landmark/run",
            "--technical-changelog-file",
            ".landmark/run/technical.md",
            "--evidence-file",
            ".landmark/run/evidence.json",
            "--output-file",
            "docs/releases/{version}.md",
            "--output-text-file",
            "docs/releases/{version}.txt",
            "--output-html-file",
            "docs/releases/{version}.html",
            "--output-json",
            "docs/releases/releases.json",
            "--rss-feed-file",
            "docs/releases/feed.xml",
        ])
        .output()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).to_string().into());
    }
    let evidence_path = repo.join(".landmark/run/evidence.json");
    let evidence: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    if evidence["provider"] != "local" {
        return Err("local provider evidence did not record provider=local".into());
    }
    if evidence["release_tag"] != "v1.1.0" {
        return Err(format!(
            "expected local run to compute v1.1.0, got {}",
            evidence["release_tag"]
        )
        .into());
    }
    if evidence["version_decision"]["bump"] != "minor" {
        return Err("local run did not classify feat commit as a minor bump".into());
    }
    if evidence["artifacts"]["technical_changelog_schema"]
        != "landmark.internal-technical-changelog.v1"
        || evidence["artifacts"]["public_notes_schema"] != "landmark.public-release-notes.v1"
        || evidence["artifacts"]["technical_changelog_audience"] != "internal-developer-operator"
    {
        return Err(
            "local run evidence did not separate internal and public artifact schemas".into(),
        );
    }
    let markdown = repo.join("docs/releases/v1.1.0.md");
    let plaintext = repo.join("docs/releases/v1.1.0.txt");
    let html = repo.join("docs/releases/v1.1.0.html");
    let json_path = repo.join("docs/releases/releases.json");
    let feed = repo.join("docs/releases/feed.xml");
    for path in [&markdown, &plaintext, &html, &json_path, &feed] {
        if !path.is_file() {
            return Err(format!("local run did not write {}", path.display()).into());
        }
    }
    let notes = fs::read_to_string(&markdown)?;
    if !notes.contains("Add portable release run") {
        return Err("local run release notes did not include the feature commit".into());
    }
    let technical = fs::read_to_string(repo.join(".landmark/run/technical.md"))?;
    if !technical.contains("feat(cli): add portable release run") {
        return Err("local run technical changelog did not include the raw commit".into());
    }
    run_ok("git", ["tag", "v1.1.0"], &repo)?;
    fs::write(repo.join("after-release.txt"), "post release work\n")?;
    run_ok("git", ["add", "after-release.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "fix(cli): post-release patch"],
        &repo,
    )?;
    let tagged_result = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "local",
            "--repo-root",
            repo.to_str().unwrap(),
            "--repository",
            "local-provider-run",
            "--release-tag",
            "v1.1.0",
            "--output-dir",
            ".landmark/tagged-run",
            "--technical-changelog-file",
            ".landmark/tagged-run/technical.md",
            "--evidence-file",
            ".landmark/tagged-run/evidence.json",
            "--output-file",
            "",
            "--output-text-file",
            "",
            "--output-html-file",
            "",
            "--output-json",
            "",
            "--rss-feed-file",
            "",
        ])
        .output()?;
    if !tagged_result.status.success() {
        return Err(String::from_utf8_lossy(&tagged_result.stderr)
            .to_string()
            .into());
    }
    let tagged_evidence_path = repo.join(".landmark/tagged-run/evidence.json");
    let tagged_evidence: Value = serde_json::from_str(&fs::read_to_string(&tagged_evidence_path)?)?;
    if tagged_evidence["version_decision"]["range"] != "v1.0.0..v1.1.0" {
        return Err(format!(
            "expected existing-tag run to end at v1.1.0, got {}",
            tagged_evidence["version_decision"]["range"]
        )
        .into());
    }
    if tagged_evidence["version_decision"]["commit_count"] != 1 {
        return Err("existing-tag run included commits outside the tagged range".into());
    }
    let tagged_technical = fs::read_to_string(repo.join(".landmark/tagged-run/technical.md"))?;
    if tagged_technical.contains("post-release patch") {
        return Err("existing-tag run included a post-release commit".into());
    }
    let breaking_repo = tmp_root.join("local-provider-breaking-footer");
    init_fixture_repo(&breaking_repo, "v1.2.3")?;
    fs::write(breaking_repo.join("api.txt"), "breaking api\n")?;
    run_ok("git", ["add", "api.txt"], &breaking_repo)?;
    run_ok(
        "git",
        [
            "commit",
            "-q",
            "-m",
            "feat(api): rename field",
            "-m",
            "BREAKING CHANGE: clients must migrate field names",
        ],
        &breaking_repo,
    )?;
    let breaking_result = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "local",
            "--repo-root",
            breaking_repo.to_str().unwrap(),
            "--repository",
            "local-provider-breaking-footer",
            "--output-dir",
            ".landmark/run",
            "--technical-changelog-file",
            ".landmark/run/technical.md",
            "--evidence-file",
            ".landmark/run/evidence.json",
            "--output-file",
            "",
            "--output-text-file",
            "",
            "--output-html-file",
            "",
            "--output-json",
            "",
            "--rss-feed-file",
            "",
        ])
        .output()?;
    if !breaking_result.status.success() {
        return Err(String::from_utf8_lossy(&breaking_result.stderr)
            .to_string()
            .into());
    }
    let breaking_evidence_path = breaking_repo.join(".landmark/run/evidence.json");
    let breaking_evidence: Value =
        serde_json::from_str(&fs::read_to_string(&breaking_evidence_path)?)?;
    if breaking_evidence["version_decision"]["bump"] != "major"
        || breaking_evidence["release_tag"] != "v2.0.0"
    {
        return Err("local run did not treat BREAKING CHANGE footer as a major bump".into());
    }
    Ok(json!({
        "evidence": evidence,
        "tagged_evidence": tagged_evidence,
        "breaking_footer_evidence": breaking_evidence,
        "stdout": String::from_utf8_lossy(&result.stdout).trim(),
        "artifacts": {
            "markdown": markdown,
            "plaintext": plaintext,
            "html": html,
            "json": json_path,
            "rss": feed,
            "technical_changelog": repo.join(".landmark/run/technical.md"),
            "evidence": evidence_path,
        }
    }))
}

fn scenario_github_provider_run(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("github-provider-run");
    init_fixture_repo(&repo, "v1.0.0")?;
    fs::write(repo.join("feature.txt"), "github provider release\n")?;
    run_ok("git", ["add", "feature.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat(action): add provider run"],
        &repo,
    )?;
    let mut fake = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.to_string(),
        update_status: 200,
        ..Default::default()
    };
    fake.releases.insert(
        "v1.1.0".to_string(),
        json!({"id": 11, "tag_name": "v1.1.0", "body": "## Technical\n\n- Existing release body", "html_url": "https://example.invalid/releases/v1.1.0"}),
    );
    let server = start_fake_server(fake)?;
    let notes_file = repo.join("notes.md");
    fs::write(
        &notes_file,
        "## Improvements in v1.1.0\n\n- Add provider run\n",
    )?;
    let result = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "github",
            "--repo-root",
            repo.to_str().unwrap(),
            "--repository",
            "owner/repo",
            "--release-tag",
            "v1.1.0",
            "--notes-file",
            notes_file.to_str().unwrap(),
            "--github-token",
            "token",
            "--api-base-url",
            &server.url,
            "--publish-release-body",
            "--output-dir",
            ".landmark/run",
            "--technical-changelog-file",
            ".landmark/run/technical.md",
            "--evidence-file",
            ".landmark/run/evidence.json",
            "--output-file",
            "docs/releases/{version}.md",
            "--output-json",
            "docs/releases/releases.json",
            "--rss-feed-file",
            "",
        ])
        .output()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).to_string().into());
    }
    let evidence_path = repo.join(".landmark/run/evidence.json");
    let evidence: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    if evidence["provider"] != "github" {
        return Err("github provider evidence did not record provider=github".into());
    }
    if evidence["publication"]["release_body_updated"] != true {
        return Err("github provider did not report release-body update".into());
    }
    let state = server.state.lock().unwrap();
    let body = state.releases["v1.1.0"]["body"].as_str().unwrap_or("");
    if !body.contains("## What's New") || !body.contains("Add provider run") {
        return Err("github provider did not update the fake release body with run notes".into());
    }
    Ok(json!({
        "evidence": evidence,
        "release_body": body,
        "requests": state.requests,
        "artifacts": {
            "markdown": repo.join("docs/releases/v1.1.0.md"),
            "json": repo.join("docs/releases/releases.json"),
            "technical_changelog": repo.join(".landmark/run/technical.md"),
            "evidence": evidence_path,
        }
    }))
}

fn scenario_provider_run_parity(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("provider-run-parity");
    init_fixture_repo(&repo, "v1.0.0")?;
    fs::write(repo.join("feature.txt"), "provider parity\n")?;
    run_ok("git", ["add", "feature.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat(release): add provider parity"],
        &repo,
    )?;

    let local = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "local",
            "--repo-root",
            repo.to_str().unwrap(),
            "--repository",
            "provider-run-parity",
            "--output-dir",
            ".landmark/local",
            "--technical-changelog-file",
            ".landmark/local/technical.md",
            "--evidence-file",
            ".landmark/local/evidence.json",
            "--output-file",
            "docs/local/{version}.md",
            "--output-text-file",
            "docs/local/{version}.txt",
            "--output-html-file",
            "docs/local/{version}.html",
            "--output-json",
            "docs/local/releases.json",
            "--rss-feed-file",
            "docs/local/feed.xml",
        ])
        .output()?;
    if !local.status.success() {
        return Err(String::from_utf8_lossy(&local.stderr).to_string().into());
    }
    let local_evidence_path = repo.join(".landmark/local/evidence.json");
    let local_evidence: Value = serde_json::from_str(&fs::read_to_string(&local_evidence_path)?)?;
    if local_evidence["provider"] != "local" || local_evidence["release_tag"] != "v1.1.0" {
        return Err("provider parity local run did not produce v1.1.0 local evidence".into());
    }
    for path in [
        repo.join("docs/local/v1.1.0.md"),
        repo.join("docs/local/v1.1.0.txt"),
        repo.join("docs/local/v1.1.0.html"),
        repo.join("docs/local/releases.json"),
        repo.join("docs/local/feed.xml"),
    ] {
        if !path.is_file() {
            return Err(
                format!("provider parity local run did not write {}", path.display()).into(),
            );
        }
    }

    let mut fake = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.to_string(),
        update_status: 200,
        ..Default::default()
    };
    fake.releases.insert(
        "v1.1.0".to_string(),
        json!({"id": 22, "tag_name": "v1.1.0", "body": "## Technical\n\n- Existing", "html_url": "https://example.invalid/releases/v1.1.0"}),
    );
    let server = start_fake_server(fake)?;
    let local_notes = repo.join("docs/local/v1.1.0.md");
    let github = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "github",
            "--repo-root",
            repo.to_str().unwrap(),
            "--repository",
            "owner/repo",
            "--release-tag",
            "v1.1.0",
            "--server-url",
            "https://github.enterprise.invalid",
            "--notes-file",
            local_notes.to_str().unwrap(),
            "--github-token",
            "token",
            "--api-base-url",
            &server.url,
            "--publish-release-body",
            "--output-dir",
            ".landmark/github",
            "--technical-changelog-file",
            ".landmark/github/technical.md",
            "--evidence-file",
            ".landmark/github/evidence.json",
            "--output-file",
            "docs/github/{version}.md",
            "--output-text-file",
            "docs/github/{version}.txt",
            "--output-html-file",
            "docs/github/{version}.html",
            "--output-json",
            "docs/github/releases.json",
            "--rss-feed-file",
            "docs/github/feed.xml",
        ])
        .output()?;
    if !github.status.success() {
        return Err(String::from_utf8_lossy(&github.stderr).to_string().into());
    }
    let github_evidence_path = repo.join(".landmark/github/evidence.json");
    let github_evidence: Value = serde_json::from_str(&fs::read_to_string(&github_evidence_path)?)?;
    if github_evidence["provider"] != "github"
        || github_evidence["release_tag"] != local_evidence["release_tag"]
        || github_evidence["publication"]["release_body_updated"] != true
    {
        return Err("provider parity github run did not publish the same fixture release".into());
    }
    for path in [
        repo.join("docs/github/v1.1.0.md"),
        repo.join("docs/github/v1.1.0.txt"),
        repo.join("docs/github/v1.1.0.html"),
        repo.join("docs/github/releases.json"),
        repo.join("docs/github/feed.xml"),
    ] {
        if !path.is_file() {
            return Err(format!(
                "provider parity github run did not write {}",
                path.display()
            )
            .into());
        }
    }
    let github_feed = fs::read_to_string(repo.join("docs/github/feed.xml"))?;
    if !github_feed.contains("<link>https://github.enterprise.invalid/owner/repo</link>") {
        return Err(
            "provider parity github feed channel did not use the configured GitHub server URL"
                .into(),
        );
    }
    if !github_feed.contains("https://github.enterprise.invalid/owner/repo/releases/tag/v1.1.0") {
        return Err(
            "provider parity github feed did not use the configured GitHub server URL".into(),
        );
    }
    let state = server.state.lock().unwrap();
    let body = state.releases["v1.1.0"]["body"].as_str().unwrap_or("");
    if !body.contains("Add provider parity") {
        return Err("provider parity github release body did not use local notes".into());
    }
    Ok(json!({
        "local_evidence": local_evidence,
        "github_evidence": github_evidence,
        "release_body": body,
        "requests": state.requests,
    }))
}

fn consumer_success(tmp_root: &Path, name: &str, write_artifact: bool) -> Result<Value> {
    let repo = tmp_root.join(name);
    init_fixture_repo(&repo, "v1.2.3")?;
    let templates_dir = env::current_dir()?.join("templates/prompts");
    let mut fake = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.to_string(),
        update_status: 200,
        ..Default::default()
    };
    fake.releases.insert("v1.2.3".to_string(), json!({"id": 1, "tag_name": "v1.2.3", "body": "## Technical\n\n- Old", "html_url": "https://example.invalid/releases/v1.2.3"}));
    let server = start_fake_server(fake)?;
    let notes_file = repo.join("notes.md");
    let quality_file = repo.join("quality.txt");
    let synth = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "test-key",
            "--model",
            "test/model",
            "--api-url",
            &format!("{}/chat/completions", server.url),
            "--product-name",
            "fixture",
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file"])
        .arg(&quality_file)
        .current_dir(&repo)
        .output()?;
    if !synth.status.success() {
        return Err(String::from_utf8_lossy(&synth.stderr).to_string().into());
    }
    fs::write(&notes_file, &synth.stdout)?;
    let update = Command::new(current_exe())
        .args([
            "update-release",
            "--github-token",
            "token",
            "--repository",
            "owner/repo",
            "--tag",
            "v1.2.3",
            "--notes-file",
        ])
        .arg(&notes_file)
        .args(["--api-base-url", &server.url])
        .current_dir(&repo)
        .output()?;
    if !update.status.success() {
        return Err(String::from_utf8_lossy(&update.stderr).to_string().into());
    }
    let artifact = if write_artifact {
        let result = Command::new(current_exe())
            .args([
                "write-artifacts",
                "--notes-file",
                notes_file.to_str().unwrap(),
                "--version",
                "v1.2.3",
                "--output-file",
                "docs/releases/{version}.md",
            ])
            .current_dir(&repo)
            .output()?;
        Some(
            json!({"returncode": result.status.code(), "stdout": String::from_utf8_lossy(&result.stdout).trim()}),
        )
    } else {
        None
    };
    let state = server.state.lock().unwrap();
    Ok(json!({
        "quality": fs::read_to_string(quality_file)?.trim(),
        "generated_notes": String::from_utf8(synth.stdout)?,
        "release_body": state.releases["v1.2.3"]["body"],
        "requests": state.requests,
        "artifact": artifact,
        "tags": git_tags(&repo)?,
    }))
}

fn scenario_manifest_defaults_and_overrides(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("manifest-defaults");
    init_fixture_repo(&repo, "v1.2.3")?;
    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Manifest Product
  description: Manifest description
audience: enterprise
voice: Manifest voice
changelog:
  source: release-body
model:
  policy: cheap
  primary: manifest/model
  fallbacks:
    - manifest/fallback
"#,
    )?;
    let release_body = repo.join("release-body.md");
    fs::write(
        &release_body,
        "## Manifest Technical\n\n- Manifest source\n",
    )?;
    let explicit_changelog = repo.join("CHANGELOG.md");
    fs::write(&explicit_changelog, "## [1.2.3]\n\n- Explicit source\n")?;
    let templates_dir = env::current_dir()?.join("templates/prompts");
    let fake = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.to_string(),
        update_status: 200,
        ..Default::default()
    };
    let server = start_fake_server(fake)?;
    let defaults_quality = repo.join("defaults-quality.txt");
    let defaults = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "test-key",
            "--api-url",
            &format!("{}/chat/completions", server.url),
            "--version",
            "v1.2.3",
            "--changelog-file",
        ])
        .arg(repo.join("missing-changelog.md"))
        .args(["--release-body-file"])
        .arg(&release_body)
        .args(["--templates-dir"])
        .arg(&templates_dir)
        .args(["--quality-file"])
        .arg(&defaults_quality)
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !defaults.status.success() {
        return Err(String::from_utf8_lossy(&defaults.stderr).to_string().into());
    }

    let override_quality = repo.join("override-quality.txt");
    let overrides = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "test-key",
            "--model",
            "explicit/model",
            "--api-url",
            &format!("{}/chat/completions", server.url),
            "--product-name",
            "Explicit Product",
            "--product-description",
            "Explicit description",
            "--voice-guide",
            "Explicit voice",
            "--audience",
            "developer",
            "--changelog-source",
            "changelog",
            "--version",
            "v1.2.3",
            "--changelog-file",
        ])
        .arg(&explicit_changelog)
        .args(["--templates-dir"])
        .arg(&templates_dir)
        .args(["--quality-file"])
        .arg(&override_quality)
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !overrides.status.success() {
        return Err(String::from_utf8_lossy(&overrides.stderr)
            .to_string()
            .into());
    }

    let requests = server.state.lock().unwrap().requests.clone();
    let default_request = request_payload(&requests, 0)?;
    let override_request = request_payload(&requests, 1)?;
    let default_prompt = default_request["messages"][1]["content"]
        .as_str()
        .unwrap_or_default();
    let override_prompt = override_request["messages"][1]["content"]
        .as_str()
        .unwrap_or_default();
    if default_request["model"] != "manifest/model"
        || !default_prompt.contains("Manifest Product")
        || !default_prompt.contains("Manifest description")
        || !default_prompt.contains("Manifest voice")
        || !default_prompt.contains("Manifest source")
    {
        return Err("manifest defaults did not reach synthesis prompt".into());
    }
    if override_request["model"] != "explicit/model"
        || !override_prompt.contains("Explicit Product")
        || !override_prompt.contains("Explicit description")
        || !override_prompt.contains("Explicit voice")
        || !override_prompt.contains("Explicit source")
        || override_prompt.contains("Manifest source")
    {
        return Err("explicit synthesis inputs did not override manifest defaults".into());
    }

    Ok(json!({
        "default_model": default_request["model"],
        "override_model": override_request["model"],
        "default_quality": fs::read_to_string(defaults_quality)?.trim(),
        "override_quality": fs::read_to_string(override_quality)?.trim(),
        "checked": [
            ".landmark.yml",
            "manifest model/product/audience/voice/changelog defaults",
            "explicit CLI override precedence"
        ],
    }))
}

fn scenario_synthesis_cost_policy(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("synthesis-cost-policy");
    init_fixture_repo(&repo, "v1.2.3")?;
    let templates_dir = env::current_dir()?.join("templates/prompts");

    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Cost Policy Demo
  description: Demo release automation.
model:
  policy: balanced
"#,
    )?;
    fs::write(
        repo.join("CHANGELOG.md"),
        "## [1.2.3]\n\n- docs: update README.md\n",
    )?;
    let dry_run = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "",
            "--api-url",
            "http://127.0.0.1:1/chat/completions",
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file", "quality-dry.txt", "--dry-run-cost"])
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !dry_run.status.success() {
        return Err(String::from_utf8_lossy(&dry_run.stderr).to_string().into());
    }
    let dry_context: Value = serde_json::from_slice(&dry_run.stdout)?;
    if dry_context["cost"]["skip"] != true
        || dry_context["cost"]["model_tier"] != "off"
        || dry_context["decision"]["action"] != "skipped"
        || dry_context["deterministic"]["docs"]
            .as_array()
            .unwrap()
            .is_empty()
        || dry_context["deterministic"]["artifacts"]["internal_technical_changelog"]
            != "landmark.internal-technical-changelog.v1"
        || dry_context["classification"]["categories"]
            .as_array()
            .unwrap()
            .iter()
            .all(|category| category != "docs-only")
    {
        return Err("dry-run cost policy did not skip docs-only release".into());
    }

    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Cost Policy Demo
  description: Demo release automation.
model:
  policy: cheap
"#,
    )?;
    fs::write(
        repo.join("CHANGELOG.md"),
        "## [1.2.3]\n\n- feat(cli): add a fleet command\n",
    )?;
    let cheap_context_file = repo.join("cheap-context.json");
    let cheap_attempts = repo.join("cheap-attempts.json");
    let cheap_quality = repo.join("cheap-quality.txt");
    let fake = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.to_string(),
        update_status: 200,
        ..Default::default()
    };
    let server = start_fake_server(fake)?;
    let cheap = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "test-key",
            "--api-url",
            &format!("{}/chat/completions", server.url),
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file"])
        .arg(&cheap_quality)
        .args(["--attempts-file"])
        .arg(&cheap_attempts)
        .args(["--context-metadata-file"])
        .arg(&cheap_context_file)
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !cheap.status.success() {
        return Err(String::from_utf8_lossy(&cheap.stderr).to_string().into());
    }
    let cheap_requests = server.state.lock().unwrap().requests.clone();
    let cheap_request = request_payload(&cheap_requests, 0)?;
    let cheap_context: Value = serde_json::from_str(&fs::read_to_string(&cheap_context_file)?)?;
    if cheap_request["model"] != "openai/gpt-4o-mini"
        || cheap_context["cost"]["model_tier"] != "cheap"
        || cheap_context["decision"]["action"] != "used"
        || cheap_context["deterministic"]["manifest"]["present"] != true
        || cheap_context["sources"]
            .as_array()
            .unwrap()
            .iter()
            .all(|source| source["name"] != "technical_changelog")
    {
        return Err("cheap policy did not use cheap model with context metadata".into());
    }

    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Cost Policy Demo
  description: Demo release automation.
model:
  policy: balanced
  primary: primary/model
  fallbacks:
    - fallback/model
"#,
    )?;
    let mut fallback_fake = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.to_string(),
        update_status: 200,
        ..Default::default()
    };
    for _ in 0..HttpPolicy::default().attempts {
        fallback_fake.llm_responses.push_back((500, String::new()));
    }
    fallback_fake
        .llm_responses
        .push_back((200, VALID_NOTES.to_string()));
    let fallback_server = start_fake_server(fallback_fake)?;
    let fallback_attempts = repo.join("fallback-attempts.json");
    let fallback = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "test-key",
            "--api-url",
            &format!("{}/chat/completions", fallback_server.url),
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file", "fallback-quality.txt"])
        .args(["--attempts-file"])
        .arg(&fallback_attempts)
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !fallback.status.success() {
        return Err(String::from_utf8_lossy(&fallback.stderr).to_string().into());
    }
    let attempts: Value = serde_json::from_str(&fs::read_to_string(&fallback_attempts)?)?;
    if attempts.as_array().unwrap().len() != 2
        || attempts[0]["succeeded"] != false
        || attempts[1]["model"] != "fallback/model"
        || attempts[1]["succeeded"] != true
    {
        return Err("fallback attempt sequence was not recorded".into());
    }
    let fallback_requests = fallback_server.state.lock().unwrap().requests.len();
    if fallback_requests != HttpPolicy::default().attempts + 1 {
        return Err(format!(
            "fallback replay expected primary HTTP retries plus fallback request, got {fallback_requests}"
        )
        .into());
    }

    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Cost Policy Demo
  description: Demo release automation.
model:
  policy: balanced
"#,
    )?;
    fs::write(
        repo.join("CHANGELOG.md"),
        "## [1.2.3]\n\n- feat(api)!: rotate security-sensitive release token configuration\n\nBREAKING CHANGE: tokens moved to a new manifest field.\n",
    )?;
    let rich = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "",
            "--api-url",
            "http://127.0.0.1:1/chat/completions",
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file", "rich-quality.txt", "--dry-run-cost"])
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !rich.status.success() {
        return Err(String::from_utf8_lossy(&rich.stderr).to_string().into());
    }
    let rich_context: Value = serde_json::from_slice(&rich.stdout)?;
    if rich_context["cost"]["model_tier"] != "rich"
        || rich_context["decision"]["action"] != "escalated"
        || rich_context["classification"]["security"] != true
        || rich_context["classification"]["breaking"] != true
    {
        return Err("balanced policy did not escalate high-significance release".into());
    }

    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Cost Policy Demo
  description: Demo release automation.
model:
  policy: rich
"#,
    )?;
    let direct_rich = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "",
            "--api-url",
            "http://127.0.0.1:1/chat/completions",
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args([
            "--quality-file",
            "direct-rich-quality.txt",
            "--dry-run-cost",
        ])
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !direct_rich.status.success() {
        return Err(String::from_utf8_lossy(&direct_rich.stderr)
            .to_string()
            .into());
    }
    let direct_rich_context: Value = serde_json::from_slice(&direct_rich.stdout)?;
    if direct_rich_context["cost"]["model_tier"] != "rich"
        || direct_rich_context["decision"]["action"] != "used"
    {
        return Err("direct rich policy should use, not escalate, rich synthesis".into());
    }

    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Cost Policy Demo
  description: Demo release automation.
model:
  policy: off
"#,
    )?;
    let off_attempts = repo.join("off-attempts.json");
    let off = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "",
            "--api-url",
            "http://127.0.0.1:1/chat/completions",
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file", "off-quality.txt"])
        .args(["--attempts-file"])
        .arg(&off_attempts)
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !off.status.success() {
        return Err(String::from_utf8_lossy(&off.stderr).to_string().into());
    }
    let off_attempts_json: Value = serde_json::from_str(&fs::read_to_string(&off_attempts)?)?;
    if off_attempts_json[0]["quality"] != "skipped"
        || off_attempts_json[0]["decision"]["action"] != "skipped"
    {
        return Err("off policy did not explain skipped synthesis".into());
    }

    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Cost Policy Demo
  description: Demo release automation.
model:
  policy: cheap
  primary: primary/model
"#,
    )?;
    let provider_failure_server = start_fake_server(FakeState {
        llm_status: 500,
        llm_notes: String::new(),
        update_status: 200,
        ..Default::default()
    })?;
    let provider_failure_attempts = repo.join("provider-failure-attempts.json");
    let provider_failure = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "test-key",
            "--api-url",
            &format!("{}/chat/completions", provider_failure_server.url),
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file", "provider-failure-quality.txt"])
        .args(["--attempts-file"])
        .arg(&provider_failure_attempts)
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if provider_failure.status.success() {
        return Err("provider failure synthesis should return a failed exit".into());
    }
    let provider_failure_json: Value =
        serde_json::from_str(&fs::read_to_string(&provider_failure_attempts)?)?;
    if provider_failure_json[0]["quality"] != "failed"
        || provider_failure_json[0]["decision"]["action"] != "used"
        || !provider_failure_json[0]["message"]
            .as_str()
            .unwrap_or("")
            .contains("failed")
    {
        return Err("provider failure path did not record failed attempt metadata".into());
    }

    Ok(json!({
        "dry_run_skip": dry_context["cost"],
        "cheap_model": cheap_request["model"],
        "fallback_attempts": attempts,
        "rich_cost": rich_context["cost"],
        "direct_rich_decision": direct_rich_context["decision"],
        "off_policy": off_attempts_json,
        "provider_failure": provider_failure_json,
    }))
}

fn request_payload(requests: &[Value], index: usize) -> Result<Value> {
    let body = requests
        .get(index)
        .and_then(|request| request["body"].as_str())
        .ok_or_else(|| format!("missing fake LLM request {index}"))?;
    Ok(serde_json::from_str(body)?)
}

fn scenario_consumer_degraded_required_fails(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("consumer-degraded");
    init_fixture_repo(&repo, "v1.2.3")?;
    let templates_dir = env::current_dir()?.join("templates/prompts");
    let mut fake = FakeState {
        llm_status: 200,
        llm_notes: INVALID_NOTES.to_string(),
        update_status: 200,
        ..Default::default()
    };
    fake.releases.insert(
        "v1.2.3".to_string(),
        json!({"id": 1, "tag_name": "v1.2.3", "body": "body"}),
    );
    let server = start_fake_server(fake)?;
    let quality_file = repo.join("quality.txt");
    let synth = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "test-key",
            "--model",
            "test/model",
            "--api-url",
            &format!("{}/chat/completions", server.url),
            "--product-name",
            "fixture",
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file"])
        .arg(&quality_file)
        .current_dir(&repo)
        .output()?;
    if !synth.status.success() {
        return Err("degraded synthesis should still emit notes".into());
    }
    let output = temp_file("landmark-policy")?;
    let policy = Command::new(current_exe())
        .args([
            "release-policy",
            "publication",
            "--synthesis-required",
            "true",
            "--synthesis-strict",
            "false",
            "--synth-succeeded",
            "true",
            "--synth-quality",
            "degraded",
            "--github-output",
        ])
        .arg(&output)
        .output()?;
    if policy.status.success() {
        return Err("required degraded policy should fail".into());
    }
    Ok(
        json!({"quality": fs::read_to_string(quality_file)?.trim(), "outputs": parse_outputs(&output)?}),
    )
}

fn scenario_consumer_release_update_failure(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("consumer-update-fail");
    init_fixture_repo(&repo, "v1.2.3")?;
    let mut fake = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.to_string(),
        update_status: 500,
        ..Default::default()
    };
    fake.releases.insert(
        "v1.2.3".to_string(),
        json!({"id": 1, "tag_name": "v1.2.3", "body": "body"}),
    );
    let server = start_fake_server(fake)?;
    let notes_file = repo.join("notes.md");
    fs::write(&notes_file, VALID_NOTES)?;
    let update = Command::new(current_exe())
        .args([
            "update-release",
            "--github-token",
            "token",
            "--repository",
            "owner/repo",
            "--tag",
            "v1.2.3",
            "--notes-file",
        ])
        .arg(&notes_file)
        .args(["--api-base-url", &server.url])
        .current_dir(&repo)
        .output()?;
    if update.status.success() {
        return Err("release update should fail".into());
    }
    Ok(
        json!({"returncode": update.status.code(), "stderr": String::from_utf8_lossy(&update.stderr).trim()}),
    )
}

fn scenario_consumer_floating_tag_behavior(tmp_root: &Path) -> Result<Value> {
    let stable = Command::new(current_exe())
        .args(["floating-tag", "--release-tag", "v2.3.4"])
        .output()?;
    let prerelease = Command::new(current_exe())
        .args(["floating-tag", "--release-tag", "v2.3.4-beta.1"])
        .output()?;
    let stable_tag = String::from_utf8(stable.stdout)?.trim().to_string();
    let pre_tag = String::from_utf8(prerelease.stdout)?.trim().to_string();
    if stable_tag != "v2" || !pre_tag.is_empty() {
        return Err("floating tag parsing mismatch".into());
    }
    let repo = tmp_root.join("floating");
    init_fixture_repo(&repo, "v2.3.4")?;
    Ok(json!({"stable": stable_tag, "prerelease": pre_tag, "tags": git_tags(&repo)?}))
}

fn scenario_self_release_pr_path(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("self-release-pr");
    init_self_release_fixture(&repo)?;
    let prepare_output = temp_file("landmark-self-release-prepare")?;
    let dist_target = rustc_host_target()?;
    let prepare = Command::new(current_exe())
        .args([
            "prepare-self-release",
            "--repo-root",
            repo.to_str().unwrap(),
            "--repository",
            "owner/repo",
            "--release-branch",
            "landmark/self-release",
            "--dist-target",
            &dist_target,
            "--github-output",
        ])
        .arg(&prepare_output)
        .output()?;
    if !prepare.status.success() {
        return Err(String::from_utf8_lossy(&prepare.stderr).to_string().into());
    }
    let prepare_plan: Value = serde_json::from_slice(&prepare.stdout)?;
    let prepare_outputs = parse_outputs(&prepare_output)?;
    if prepare_outputs.get("released").map(String::as_str) != Some("true") {
        return Err("prepare-self-release did not mark a release PR ready".into());
    }
    assert_file_contains(&repo.join("package.json"), r#""version": "1.1.0""#)?;
    assert_file_contains(
        &repo.join("crates/landmark/Cargo.toml"),
        r#"version = "1.1.0""#,
    )?;
    assert_file_contains(&repo.join("Cargo.lock"), r#"version = "1.1.0""#)?;
    assert_file_contains(&repo.join("CHANGELOG.md"), "# [1.1.0]")?;
    let prepared_dist = fs::read(repo.join("dist/landmark"))?;
    if prepared_dist == b"stale fixture binary\n" {
        return Err("prepare-self-release did not refresh dist/landmark".into());
    }
    assert_file_contains(&repo.join("dist/landmark.sha256"), "  dist/landmark")?;
    let changed_files = prepare_plan["changed_files"]
        .as_array()
        .ok_or("prepare plan missing changed_files")?;
    for expected in ["dist/landmark", "dist/landmark.sha256"] {
        if !changed_files
            .iter()
            .any(|file| file.as_str() == Some(expected))
        {
            return Err(format!("prepare plan missing {expected}").into());
        }
    }
    let dist_sha256 = fs::read_to_string(repo.join("dist/landmark.sha256"))?
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string();

    run_ok("git", ["add", "."], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "chore(release): 1.1.0"],
        &repo,
    )?;
    let target_sha = run_ok("git", ["rev-parse", "HEAD"], &repo)?
        .trim()
        .to_string();
    let mut fake = FakeState::default();
    fake.releases.insert(
        "v1.0.0".to_string(),
        json!({"id": 1, "tag_name": "v1.0.0", "body": "old", "html_url": "https://example.invalid/releases/v1.0.0"}),
    );
    let server = start_fake_server(fake)?;
    let publish_output = temp_file("landmark-self-release-publish")?;
    let publish = Command::new(current_exe())
        .args([
            "publish-self-release",
            "--repo-root",
            repo.to_str().unwrap(),
            "--github-token",
            "token",
            "--repository",
            "owner/repo",
            "--target-sha",
            &target_sha,
            "--api-base-url",
            &server.url,
            "--github-output",
        ])
        .arg(&publish_output)
        .output()?;
    if !publish.status.success() {
        return Err(String::from_utf8_lossy(&publish.stderr).to_string().into());
    }
    let publish_outputs = parse_outputs(&publish_output)?;
    if publish_outputs.get("published").map(String::as_str) != Some("true") {
        return Err("publish-self-release did not publish the landed release".into());
    }
    let state = server.state.lock().unwrap();
    let created = state
        .releases
        .get("v1.1.0")
        .ok_or("fake GitHub release was not created")?;
    Ok(json!({
        "prepare": prepare_outputs,
        "publish": publish_outputs,
        "release": created,
        "requests": state.requests,
        "target_sha": target_sha,
        "dist": {
            "size": prepared_dist.len(),
            "sha256": dist_sha256,
            "changed_files": changed_files,
        },
    }))
}

fn assert_file_contains(path: &Path, needle: &str) -> Result<()> {
    let text = fs::read_to_string(path)?;
    if text.contains(needle) {
        Ok(())
    } else {
        Err(format!("{} did not contain {needle}", path.display()).into())
    }
}

fn init_self_release_fixture(path: &Path) -> Result<()> {
    fs::create_dir_all(path.join("crates/landmark/src"))?;
    fs::create_dir_all(path.join("dist"))?;
    run_ok("git", ["init", "-q"], path)?;
    run_ok("git", ["config", "user.name", "Landmark Replay"], path)?;
    run_ok(
        "git",
        ["config", "user.email", "replay@example.invalid"],
        path,
    )?;
    fs::write(path.join("README.md"), "# Fixture\n")?;
    fs::write(
        path.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/landmark\"]\nresolver = \"3\"\n",
    )?;
    fs::write(
        path.join("package.json"),
        serde_json::to_string_pretty(&json!({"name": "landmark", "version": "1.0.0"}))? + "\n",
    )?;
    fs::write(
        path.join("crates/landmark/Cargo.toml"),
        "[package]\nname = \"landmark\"\nversion = \"1.0.0\"\nedition = \"2024\"\n",
    )?;
    fs::write(
        path.join("crates/landmark/src/main.rs"),
        "fn main() { println!(\"landmark fixture {}\", env!(\"CARGO_PKG_VERSION\")); }\n",
    )?;
    fs::write(
        path.join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\nversion = 4\n\n[[package]]\nname = \"landmark\"\nversion = \"1.0.0\"\n",
    )?;
    fs::write(path.join("dist/landmark"), "stale fixture binary\n")?;
    fs::write(
        path.join("dist/landmark.sha256"),
        "1c8d630e34f92c015d86aacd405409334e6bf29b853d7af0d1952517cf8bc6cb  dist/landmark\n",
    )?;
    fs::write(
        path.join("CHANGELOG.md"),
        "## [1.0.0](https://github.com/owner/repo/releases/tag/v1.0.0) (2026-01-01)\n\n### Features\n\n* seed\n",
    )?;
    run_ok("git", ["add", "."], path)?;
    run_ok("git", ["commit", "-q", "-m", "chore: seed release"], path)?;
    run_ok("git", ["tag", "v1.0.0"], path)?;
    fs::write(
        path.join("README.md"),
        "# Fixture\n\nRelease PR protected branch flow.\n",
    )?;
    run_ok("git", ["add", "README.md"], path)?;
    run_ok(
        "git",
        [
            "commit",
            "-q",
            "-m",
            "feat(release): add protected branch self release",
        ],
        path,
    )?;
    Ok(())
}

struct FakeServer {
    url: String,
    state: Arc<Mutex<FakeState>>,
}

fn start_fake_server(mut state: FakeState) -> Result<FakeServer> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let server = Server::from_listener(listener, None).map_err(|error| error.to_string())?;
    let shared = Arc::new(Mutex::new({
        state.llm_notes = state.llm_notes.trim().to_string();
        state
    }));
    let thread_state = Arc::clone(&shared);
    thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            let path = request.url().to_string();
            let method = request.method().clone();
            let mut state = thread_state.lock().unwrap();
            state
                .requests
                .push(json!({"method": method.as_str(), "path": path, "body": body}));
            let response = match (method, request.url()) {
                (Method::Post, "/chat/completions") => {
                    let (status, notes) = state
                        .llm_responses
                        .pop_front()
                        .unwrap_or_else(|| (state.llm_status, state.llm_notes.clone()));
                    if status >= 400 {
                        json_response(status, json!({"error": {"message": "fake LLM failure"}}))
                    } else {
                        json_response(200, json!({"choices": [{"message": {"content": notes}}]}))
                    }
                }
                (Method::Get, url) if url.contains("/releases/tags/") => {
                    let tag = url.rsplit("/releases/tags/").next().unwrap();
                    let tag = urlencoding::decode(tag).unwrap_or_default().to_string();
                    if let Some(release) = state.releases.get(&tag) {
                        json_response(200, release.clone())
                    } else {
                        json_response(404, json!({"message": "Not Found"}))
                    }
                }
                (Method::Patch, url) if url.contains("/releases/") => {
                    if state.update_status >= 400 {
                        json_response(state.update_status, json!({"message": "update failed"}))
                    } else {
                        let id: i64 = url.rsplit('/').next().unwrap_or("0").parse().unwrap_or(0);
                        let payload: Value =
                            serde_json::from_str(&body).unwrap_or_else(|_| json!({}));
                        let mut found = None;
                        for release in state.releases.values_mut() {
                            if release["id"].as_i64() == Some(id) {
                                if let Some(new_body) = payload["body"].as_str() {
                                    release["body"] = Value::String(new_body.to_string());
                                }
                                found = Some(release.clone());
                                break;
                            }
                        }
                        found
                            .map(|release| json_response(200, release))
                            .unwrap_or_else(|| json_response(404, json!({"message": "Not Found"})))
                    }
                }
                (Method::Post, url) if url.contains("/repos/") && url.ends_with("/releases") => {
                    let payload: Value = serde_json::from_str(&body).unwrap_or_else(|_| json!({}));
                    let tag = payload["tag_name"].as_str().unwrap_or("").to_string();
                    if tag.is_empty() {
                        json_response(422, json!({"message": "tag_name is required"}))
                    } else if state.releases.contains_key(&tag) {
                        json_response(422, json!({"message": "already_exists"}))
                    } else {
                        let id = state.releases.len() as i64 + 1;
                        let release = json!({
                            "id": id,
                            "tag_name": tag,
                            "target_commitish": payload["target_commitish"],
                            "name": payload["name"],
                            "body": payload["body"],
                            "html_url": format!("https://example.invalid/releases/{}", payload["tag_name"].as_str().unwrap_or(""))
                        });
                        state.releases.insert(tag, release.clone());
                        json_response(201, release)
                    }
                }
                _ => json_response(404, json!({"message": "not found"})),
            };
            let _ = request.respond(response);
        }
    });
    thread::sleep(Duration::from_millis(50));
    Ok(FakeServer {
        url: format!("http://{addr}"),
        state: shared,
    })
}

fn start_slow_http_server(delay: Duration) -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let server = Server::from_listener(listener, None).map_err(|error| error.to_string())?;
    thread::spawn(move || {
        if let Some(request) = server.incoming_requests().next() {
            thread::sleep(delay);
            let _ = request.respond(json_response(200, json!({"ok": true})));
        }
    });
    Ok(format!("http://{addr}/slow"))
}

fn json_response(status: u16, payload: Value) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::to_vec(&payload).unwrap();
    Response::from_data(body)
        .with_status_code(status)
        .with_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
}

fn scenario_fleet_adoption_planner(tmp_root: &Path) -> Result<Value> {
    let fixture = tmp_root.join("fleet-fixture.json");
    let scan_output = tmp_root.join("fleet.json");
    let plan_dir = tmp_root.join("fleet-plan");
    let pr_dir = plan_dir.join("prs");
    let confirmed_pr_dir = plan_dir.join("prs-confirmed");
    let scan = FleetScan {
        generated_at: "2026-06-13T00:00:00Z".into(),
        owners: vec!["phrazzld".into(), "misty-step".into()],
        warnings: Vec::new(),
        repositories: vec![
            fleet_fixture_repo(
                "phrazzld/semantic-app",
                "semantic-release",
                ("application", "github-release+semantic-release"),
                (false, false),
                "unprotected-or-unavailable",
                &[],
                &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
            ),
            fleet_existing_semantic_release_workflow_fixture(),
            fleet_fixture_repo(
                "misty-step/release-please-app",
                "release-please",
                ("application", "github-release"),
                (false, false),
                "protected",
                &[],
                &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
            ),
            fleet_fixture_repo(
                "misty-step/changesets-app",
                "changesets",
                ("library", "github-release"),
                (false, false),
                "unprotected-or-unavailable",
                &["package.json", "Cargo.toml"],
                &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
            ),
            fleet_fixture_repo(
                "phrazzld/manual-app",
                "manual-tag",
                ("application", "github-release"),
                (false, false),
                "unprotected-or-unavailable",
                &[],
                &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
            ),
            fleet_fixture_repo_with_packages(
                "phrazzld/no-release-ts-app",
                "no-release-tool",
                ("application", "none"),
                (false, false),
                "unprotected-or-unavailable",
                &["package.json"],
                &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
            ),
            fleet_fixture_repo_with_packages(
                "misty-step/no-release-rust-crate",
                "no-release-tool",
                ("library", "none"),
                (false, false),
                "unprotected-or-unavailable",
                &["Cargo.toml"],
                &[],
            ),
            fleet_fixture_repo_with_packages(
                "phrazzld/no-release-go-app",
                "no-release-tool",
                ("application", "none"),
                (false, false),
                "unprotected-or-unavailable",
                &["go.mod"],
                &[],
            ),
            fleet_fixture_repo_with_packages(
                "misty-step/no-release-python-lib",
                "no-release-tool",
                ("library", "none"),
                (false, false),
                "unprotected-or-unavailable",
                &["pyproject.toml"],
                &[],
            ),
            fleet_fixture_repo_with_packages(
                "phrazzld/no-release-multipackage",
                "no-release-tool",
                ("library", "none"),
                (false, false),
                "unprotected-or-unavailable",
                &["Cargo.toml", "package.json", "packages/api/package.json"],
                &[],
            ),
            fleet_fixture_repo(
                "misty-step/archived-app",
                "semantic-release",
                ("archived", "github-release+semantic-release"),
                (true, false),
                "unprotected-or-unavailable",
                &[],
                &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
            ),
            fleet_fixture_repo(
                "misty-step/private-app",
                "release-please",
                ("application", "github-release"),
                (false, true),
                "unavailable: no GitHub token supplied",
                &[],
                &[],
            ),
            fleet_fixture_repo(
                "phrazzld/protected-app",
                "manual-tag",
                ("application", "github-release"),
                (false, false),
                "protected",
                &[],
                &["GH_RELEASE_TOKEN"],
            ),
            fleet_fixture_repo(
                "misty-step/terraform-infra",
                "no-release-tool",
                ("infrastructure", "local-artifacts"),
                (false, false),
                "unprotected-or-unavailable",
                &["go.mod"],
                &[],
            ),
            fleet_fixture_repo(
                "phrazzld/search-experiment",
                "no-release-tool",
                ("experiment", "local-artifacts"),
                (false, false),
                "unprotected-or-unavailable",
                &["Cargo.toml"],
                &[],
            ),
            fleet_fixture_repo(
                "misty-step/docs-site",
                "no-release-tool",
                ("non-release", "none"),
                (false, false),
                "unprotected-or-unavailable",
                &[],
                &[],
            ),
            fleet_incomplete_secret_fixture(),
            fleet_existing_landmark_fixture(),
            fleet_existing_landmark_workflow_fixture(),
        ],
    };
    fs::write(&fixture, serde_json::to_string_pretty(&scan)? + "\n")?;
    let scan_result = Command::new(current_exe())
        .args([
            "fleet",
            "scan",
            "--owner",
            "phrazzld",
            "--owner",
            "misty-step",
            "--fixture",
            fixture.to_str().unwrap(),
            "--output",
            scan_output.to_str().unwrap(),
        ])
        .output()?;
    if !scan_result.status.success() {
        return Err(String::from_utf8_lossy(&scan_result.stderr)
            .to_string()
            .into());
    }
    let plan_result = Command::new(current_exe())
        .args([
            "fleet",
            "plan",
            "--input",
            scan_output.to_str().unwrap(),
            "--output-dir",
            plan_dir.to_str().unwrap(),
        ])
        .output()?;
    if !plan_result.status.success() {
        return Err(String::from_utf8_lossy(&plan_result.stderr)
            .to_string()
            .into());
    }
    let dry_run = Command::new(current_exe())
        .args([
            "fleet",
            "open-prs",
            "--dry-run",
            "--plan-dir",
            plan_dir.to_str().unwrap(),
            "--output-dir",
            pr_dir.to_str().unwrap(),
        ])
        .output()?;
    if !dry_run.status.success() {
        return Err(String::from_utf8_lossy(&dry_run.stderr).to_string().into());
    }
    let unconfirmed = Command::new(current_exe())
        .args([
            "fleet",
            "open-prs",
            "--plan-dir",
            plan_dir.to_str().unwrap(),
            "--output-dir",
            confirmed_pr_dir.to_str().unwrap(),
        ])
        .output()?;
    if unconfirmed.status.success()
        || !String::from_utf8_lossy(&unconfirmed.stderr).contains("--confirm-remote")
    {
        return Err("fleet open-prs non-dry-run should require --confirm-remote".into());
    }
    let confirmed = Command::new(current_exe())
        .args([
            "fleet",
            "open-prs",
            "--confirm-remote",
            "--max-prs",
            "1",
            "--plan-dir",
            plan_dir.to_str().unwrap(),
            "--output-dir",
            confirmed_pr_dir.to_str().unwrap(),
        ])
        .output()?;
    if !confirmed.status.success() {
        return Err(String::from_utf8_lossy(&confirmed.stderr)
            .to_string()
            .into());
    }
    let plan: FleetPlan = serde_json::from_str(&fs::read_to_string(plan_dir.join("plan.json"))?)?;
    let pr_plan: FleetPrPlan =
        serde_json::from_str(&fs::read_to_string(pr_dir.join("open-prs.json"))?)?;
    let confirmed_plan: FleetPrPlan =
        serde_json::from_str(&fs::read_to_string(confirmed_pr_dir.join("open-prs.json"))?)?;
    if confirmed_plan.dry_run
        || !confirmed_plan
            .repositories
            .iter()
            .any(|repo| repo.disposition.contains("operator-apply"))
    {
        return Err("confirmed fleet open-prs receipt did not record guarded rollout".into());
    }
    if !confirmed_pr_dir
        .join("phrazzld__semantic-app")
        .join("APPLY.md")
        .is_file()
    {
        return Err("confirmed fleet open-prs did not write an apply packet".into());
    }
    let mut modes = BTreeMap::new();
    let mut statuses = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    for repo in &plan.repositories {
        modes.insert(repo.repository.clone(), repo.integration_mode.clone());
        statuses.insert(repo.repository.clone(), repo.status.clone());
        kinds.insert(repo.repository.clone(), repo.repository_kind.clone());
    }
    for (repo, expected) in [
        ("phrazzld/semantic-app", "github-full"),
        ("phrazzld/semantic-workflow-app", "blocked"),
        ("misty-step/release-please-app", "github-synthesis-only"),
        ("misty-step/changesets-app", "github-synthesis-only"),
        ("phrazzld/manual-app", "github-synthesis-only"),
        ("phrazzld/no-release-ts-app", "backfill-first"),
        ("misty-step/no-release-rust-crate", "backfill-first"),
        ("phrazzld/no-release-go-app", "backfill-first"),
        ("misty-step/no-release-python-lib", "backfill-first"),
        ("phrazzld/no-release-multipackage", "backfill-first"),
        ("misty-step/terraform-infra", "generic-ci"),
        ("phrazzld/search-experiment", "local"),
        ("misty-step/docs-site", "skipped"),
        ("phrazzld/incomplete-secret-app", "github-full"),
        ("misty-step/archived-app", "skipped"),
        ("misty-step/existing-landmark-app", "manifest-only"),
        ("phrazzld/existing-landmark-workflow", "manifest-only"),
    ] {
        if modes.get(repo).map(String::as_str) != Some(expected) {
            return Err(format!("{repo} expected mode {expected}").into());
        }
    }
    for repo in [
        "phrazzld/no-release-ts-app",
        "misty-step/no-release-rust-crate",
        "phrazzld/no-release-go-app",
        "misty-step/no-release-python-lib",
        "phrazzld/no-release-multipackage",
    ] {
        if statuses.get(repo).map(String::as_str) != Some("ready") {
            return Err(format!("{repo} should be ready for backfill-first adoption").into());
        }
    }
    for (repo, expected) in [
        ("phrazzld/semantic-app", "application"),
        ("misty-step/changesets-app", "library"),
        ("misty-step/no-release-rust-crate", "library"),
        ("misty-step/no-release-python-lib", "library"),
        ("misty-step/terraform-infra", "infrastructure"),
        ("phrazzld/search-experiment", "experiment"),
        ("misty-step/docs-site", "non-release"),
        ("misty-step/archived-app", "archived"),
    ] {
        if kinds.get(repo).map(String::as_str) != Some(expected) {
            return Err(format!("{repo} expected kind {expected}").into());
        }
    }
    let infra = plan
        .repositories
        .iter()
        .find(|repo| repo.repository == "misty-step/terraform-infra")
        .ok_or("infrastructure fixture missing")?;
    if !infra.required_secrets.is_empty() || !infra.missing_secrets.is_empty() {
        return Err(
            "generic-ci infrastructure fixture should not require GitHub Action secrets".into(),
        );
    }
    if !infra
        .integration_rationale
        .iter()
        .any(|reason| reason.contains("infrastructure"))
    {
        return Err("infrastructure fixture missing integration rationale".into());
    }
    let incomplete = plan
        .repositories
        .iter()
        .find(|repo| repo.repository == "phrazzld/incomplete-secret-app")
        .ok_or("incomplete-secret fixture missing")?;
    if incomplete.status != "blocked" || incomplete.unavailable_secret_metadata.len() != 2 {
        return Err("incomplete secret metadata should block GitHub integration".into());
    }
    let protected = plan
        .repositories
        .iter()
        .find(|repo| repo.repository == "phrazzld/protected-app")
        .ok_or("protected fixture missing")?;
    if !protected
        .risk_flags
        .iter()
        .any(|flag| flag.contains("protected"))
    {
        return Err("branch-protected repository missing risk flag".into());
    }
    let private = plan
        .repositories
        .iter()
        .find(|repo| repo.repository == "misty-step/private-app")
        .ok_or("private fixture missing")?;
    if private.unavailable_secret_metadata.len() != 2 {
        return Err("private fixture should report unavailable secret metadata".into());
    }
    let dry_diff = pr_dir.join("phrazzld__semantic-app").join("diff.md");
    if !dry_diff.is_file() {
        return Err("dry-run PR diff missing for semantic fixture".into());
    }
    let manifest_only = pr_plan
        .repositories
        .iter()
        .find(|repo| repo.repository == "phrazzld/existing-landmark-workflow")
        .ok_or("existing Landmark workflow dry-run missing")?;
    if manifest_only
        .files
        .iter()
        .any(|file| file.contains("landmark-release.yml"))
    {
        return Err("manifest-only dry-run should not add a duplicate workflow".into());
    }
    if pr_dir
        .join("phrazzld__existing-landmark-workflow")
        .join(".github/workflows/landmark-release.yml")
        .exists()
    {
        return Err("manifest-only dry-run wrote a duplicate workflow file".into());
    }
    for repo in ["misty-step__terraform-infra", "phrazzld__search-experiment"] {
        if pr_dir
            .join(repo)
            .join(".github/workflows/landmark-release.yml")
            .exists()
        {
            return Err(format!("{repo} dry-run wrote a GitHub workflow").into());
        }
    }
    let plan_readme = fs::read_to_string(plan_dir.join("README.md"))?;
    if !plan_readme.contains("## Blocked And Skipped Repositories")
        || !plan_readme.contains("existing semantic-release workflow")
        || !plan_readme.contains("secret metadata unavailable")
    {
        return Err("fleet rollout report should explain blocked and skipped repositories".into());
    }
    for (slug, repo_name) in [
        ("phrazzld__no-release-ts-app", "phrazzld/no-release-ts-app"),
        (
            "misty-step__no-release-rust-crate",
            "misty-step/no-release-rust-crate",
        ),
        ("phrazzld__no-release-go-app", "phrazzld/no-release-go-app"),
        (
            "misty-step__no-release-python-lib",
            "misty-step/no-release-python-lib",
        ),
        (
            "phrazzld__no-release-multipackage",
            "phrazzld/no-release-multipackage",
        ),
    ] {
        let receipt = pr_plan
            .repositories
            .iter()
            .find(|repo| repo.repository == repo_name)
            .ok_or_else(|| format!("{repo_name} backfill-first receipt missing"))?;
        if receipt.skipped || !receipt.commit_message.contains("backfill-first") {
            return Err(format!("{repo_name} should render a backfill-first PR receipt").into());
        }
        if receipt
            .files
            .iter()
            .any(|file| file.contains("landmark-release.yml"))
        {
            return Err(format!("{repo_name} should not include a release workflow").into());
        }
        if pr_dir
            .join(slug)
            .join(".github/workflows/landmark-release.yml")
            .exists()
        {
            return Err(format!("{repo_name} dry-run wrote a GitHub workflow").into());
        }
        let diff = fs::read_to_string(pr_dir.join(slug).join("diff.md"))?;
        if !diff.contains("Initial version recommendation: `0.1.0`")
            || !diff.contains("landmark backfill --repo-root . --since")
            || !diff.contains("--mode artifacts-only --dry-run")
            || !diff.contains("Rollback:")
        {
            return Err(
                format!("{repo_name} diff missing backfill-first operator guidance").into(),
            );
        }
    }
    let release_please_path = pr_dir
        .join("misty-step__release-please-app")
        .join(".github/workflows/release.yml");
    if !release_please_path.is_file() {
        return Err("release-please dry-run did not update existing workflow path".into());
    }
    if pr_dir
        .join("misty-step__release-please-app")
        .join(".github/workflows/landmark-release.yml")
        .exists()
    {
        return Err("release-please dry-run wrote a duplicate Landmark workflow".into());
    }
    let release_please_workflow = fs::read_to_string(release_please_path)?;
    serde_yaml::from_str::<serde_yaml::Value>(&release_please_workflow)?;
    if release_please_workflow
        .matches("googleapis/release-please-action")
        .count()
        != 1
        || !release_please_workflow.contains("needs: release-please")
        || !release_please_workflow.contains("mode: synthesis-only")
    {
        return Err("release-please workflow patch duplicated or missed synthesis job".into());
    }
    let changesets_path = pr_dir
        .join("misty-step__changesets-app")
        .join(".github/workflows/release.yml");
    if !changesets_path.is_file() {
        return Err("changesets dry-run did not update existing workflow path".into());
    }
    if pr_dir
        .join("misty-step__changesets-app")
        .join(".github/workflows/landmark-release.yml")
        .exists()
    {
        return Err("changesets dry-run wrote a duplicate Landmark workflow".into());
    }
    let changesets_workflow = fs::read_to_string(changesets_path)?;
    serde_yaml::from_str::<serde_yaml::Value>(&changesets_workflow)?;
    if changesets_workflow.matches("changesets/action").count() != 1
        || !changesets_workflow.contains("needs: release")
        || !changesets_workflow.contains("mode: synthesis-only")
    {
        return Err("changesets workflow patch duplicated or missed synthesis job".into());
    }
    let semantic_blocked = pr_plan
        .repositories
        .iter()
        .find(|repo| repo.repository == "phrazzld/semantic-workflow-app")
        .ok_or("semantic workflow fixture dry-run missing")?;
    if !semantic_blocked.skipped
        || !semantic_blocked
            .reason
            .contains("existing semantic-release workflow")
    {
        return Err(
            "existing semantic-release workflow should be blocked for operator choice".into(),
        );
    }
    let semantic_receipt = pr_plan
        .repositories
        .iter()
        .find(|repo| repo.repository == "phrazzld/semantic-app")
        .ok_or("semantic receipt missing")?;
    if semantic_receipt.branch != "landmark/adopt-phrazzld-semantic-app"
        || !semantic_receipt.commit_message.contains("github-full")
        || !semantic_receipt.rollback.contains("delete branch")
        || !semantic_receipt
            .monitor_status
            .contains("monitor downstream release")
    {
        return Err("fleet PR receipt missing branch/commit/rollback/monitoring data".into());
    }
    let scan_text = fs::read_to_string(&scan_output)?;
    if scan_text.contains("super-secret") || scan_text.contains("ghp_") {
        return Err("fleet scan leaked a secret-looking value".into());
    }
    Ok(json!({
        "repositories": plan.repositories.len(),
        "dry_run_prs": pr_plan.repositories.iter().filter(|repo| !repo.skipped).count(),
        "modes": modes,
        "statuses": statuses,
        "kinds": kinds,
        "evidence": {
            "scan": scan_output,
            "plan": plan_dir.join("plan.json"),
            "dry_run": pr_dir.join("open-prs.json"),
        }
    }))
}

fn existing_release_please_workflow() -> String {
    r#"name: Existing Release

on:
  push:
    branches: [master]

jobs:
  release-please:
    runs-on: ubuntu-latest
    outputs:
      release_created: ${{ steps.release.outputs.release_created }}
      tag_name: ${{ steps.release.outputs.tag_name }}
    steps:
      - uses: googleapis/release-please-action@v4
        id: release
"#
    .into()
}

fn existing_changesets_workflow() -> String {
    r#"name: Existing Release

on:
  push:
    branches: [master]

jobs:
  release:
    runs-on: ubuntu-latest
    outputs:
      published: ${{ steps.changesets.outputs.published }}
      published_packages: ${{ steps.changesets.outputs.publishedPackages }}
    steps:
      - uses: actions/checkout@v4
      - uses: changesets/action@v1
        id: changesets
        env:
          GITHUB_TOKEN: ${{ secrets.GH_RELEASE_TOKEN }}
"#
    .into()
}

fn existing_semantic_release_workflow() -> String {
    r#"name: Existing Release

on:
  push:
    branches: [master]

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npx semantic-release
        env:
          GITHUB_TOKEN: ${{ secrets.GH_RELEASE_TOKEN }}
"#
    .into()
}

fn fleet_fixture_repo(
    name_with_owner: &str,
    release_tool: &str,
    classification: (&str, &str),
    visibility: (bool, bool),
    branch_protected: &str,
    extra_packages: &[&str],
    present_secrets: &[&str],
) -> FleetRepository {
    let (owner, name) = name_with_owner.split_once('/').unwrap();
    let (repository_kind, release_surface) = classification;
    let (archived, private) = visibility;
    let mut package_topology = vec!["package.json".to_string()];
    package_topology.extend(extra_packages.iter().map(|value| (*value).to_string()));
    package_topology.sort();
    package_topology.dedup();
    let release_files = match release_tool {
        "semantic-release" => vec![".releaserc.json".into()],
        "release-please" => vec!["release-please-config.json".into()],
        "changesets" => vec![".changeset/".into()],
        _ => Vec::new(),
    };
    let workflows = match release_tool {
        "release-please" | "changesets" => vec!["release.yml".into()],
        "manual-tag" => vec!["release.yml".into()],
        _ => Vec::new(),
    };
    let workflow_files = match release_tool {
        "release-please" => vec![
            fleet_workflow_file(
                ".github/workflows/release.yml",
                &existing_release_please_workflow(),
            )
            .expect("release-please fixture workflow"),
        ],
        "changesets" => vec![
            fleet_workflow_file(
                ".github/workflows/release.yml",
                &existing_changesets_workflow(),
            )
            .expect("changesets fixture workflow"),
        ],
        _ => Vec::new(),
    };
    let required_secrets = ["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"]
        .iter()
        .map(|name| {
            if present_secrets.contains(name) {
                FleetSecretStatus {
                    name: (*name).into(),
                    status: "present".into(),
                    detail: "metadata only; value not read".into(),
                }
            } else if private && present_secrets.is_empty() {
                FleetSecretStatus {
                    name: (*name).into(),
                    status: "unavailable".into(),
                    detail: "secret metadata unavailable in fixture".into(),
                }
            } else {
                FleetSecretStatus {
                    name: (*name).into(),
                    status: "missing".into(),
                    detail: "required secret name is absent from Actions secret metadata".into(),
                }
            }
        })
        .collect();
    FleetRepository {
        owner: owner.into(),
        name: name.into(),
        name_with_owner: name_with_owner.into(),
        repository_kind: repository_kind.into(),
        release_surface: release_surface.into(),
        private,
        archived,
        pushed_at: "2026-06-13T00:00:00Z".into(),
        default_branch: "master".into(),
        branch_protected: branch_protected.into(),
        release_tool: release_tool.into(),
        tag_format: "v{version}".into(),
        package_topology,
        release_files,
        workflows,
        workflow_files,
        existing_landmark: false,
        required_secrets,
        signals: vec![format!("{release_tool} fixture")],
    }
}

fn fleet_fixture_repo_with_packages(
    name_with_owner: &str,
    release_tool: &str,
    classification: (&str, &str),
    visibility: (bool, bool),
    branch_protected: &str,
    package_topology: &[&str],
    present_secrets: &[&str],
) -> FleetRepository {
    let mut repo = fleet_fixture_repo(
        name_with_owner,
        release_tool,
        classification,
        visibility,
        branch_protected,
        &[],
        present_secrets,
    );
    repo.package_topology = package_topology
        .iter()
        .map(|value| (*value).to_string())
        .collect();
    repo.package_topology.sort();
    repo.package_topology.dedup();
    repo.tag_format = fleet_tag_format(&[], &repo.package_topology);
    repo
}

fn fleet_existing_landmark_fixture() -> FleetRepository {
    let mut repo = fleet_fixture_repo(
        "misty-step/existing-landmark-app",
        "manual-tag",
        ("application", "github-release"),
        (false, false),
        "unprotected-or-unavailable",
        &[],
        &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
    );
    repo.existing_landmark = true;
    repo.release_files.push(".landmark.yml".into());
    repo.workflows.push("landmark-release.yml".into());
    repo.signals.push(".landmark.yml present".into());
    repo
}

fn fleet_existing_landmark_workflow_fixture() -> FleetRepository {
    let mut repo = fleet_fixture_repo(
        "phrazzld/existing-landmark-workflow",
        "manual-tag",
        ("application", "github-release"),
        (false, false),
        "unprotected-or-unavailable",
        &[],
        &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
    );
    repo.existing_landmark = true;
    repo.workflows = vec!["release.yml".into()];
    repo.signals
        .push("release.yml invokes Landmark action".into());
    repo
}

fn fleet_existing_semantic_release_workflow_fixture() -> FleetRepository {
    let mut repo = fleet_fixture_repo(
        "phrazzld/semantic-workflow-app",
        "semantic-release",
        ("application", "github-release+semantic-release"),
        (false, false),
        "unprotected-or-unavailable",
        &[],
        &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
    );
    repo.workflows = vec!["release.yml".into()];
    repo.workflow_files = vec![
        fleet_workflow_file(
            ".github/workflows/release.yml",
            &existing_semantic_release_workflow(),
        )
        .expect("semantic-release workflow fixture"),
    ];
    repo
}

fn fleet_incomplete_secret_fixture() -> FleetRepository {
    let mut repo = fleet_fixture_repo(
        "phrazzld/incomplete-secret-app",
        "semantic-release",
        ("application", "github-release+semantic-release"),
        (false, false),
        "unprotected-or-unavailable",
        &[],
        &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
    );
    repo.required_secrets.clear();
    repo
}

fn init_fixture_repo(path: &Path, release_tag: &str) -> Result<()> {
    fs::create_dir_all(path)?;
    run_ok("git", ["init", "-q"], path)?;
    run_ok("git", ["config", "user.name", "Landmark Replay"], path)?;
    run_ok(
        "git",
        ["config", "user.email", "replay@example.invalid"],
        path,
    )?;
    fs::write(path.join("README.md"), "# Fixture\n")?;
    fs::write(
        path.join("CHANGELOG.md"),
        format!("## {release_tag}\n\n- feat: replay fixture\n"),
    )?;
    run_ok("git", ["add", "."], path)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat: seed replay fixture"],
        path,
    )?;
    run_ok("git", ["tag", release_tag], path)?;
    Ok(())
}

fn git_tags(path: &Path) -> Result<Vec<String>> {
    Ok(run_ok("git", ["tag", "--list", "--sort=refname"], path)?
        .lines()
        .map(str::to_string)
        .collect())
}

fn current_exe() -> PathBuf {
    env::current_exe().expect("current executable")
}

fn rustc_host_target() -> Result<String> {
    let version = run_ok("rustc", ["-vV"], Path::new("."))?;
    version
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::to_string))
        .ok_or_else(|| "rustc -vV did not report a host target".into())
}

fn temp_file(prefix: &str) -> Result<PathBuf> {
    let path = env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    fs::write(&path, "")?;
    Ok(path)
}

fn parse_outputs(path: &Path) -> Result<BTreeMap<String, String>> {
    let mut outputs = BTreeMap::new();
    for line in fs::read_to_string(path)?.lines() {
        if let Some((key, value)) = line.split_once('=') {
            outputs.insert(key.to_string(), value.to_string());
        }
    }
    Ok(outputs)
}

fn read_nonempty(path: &Path) -> Result<String> {
    let text = fs::read_to_string(path)?;
    if text.trim().is_empty() {
        Err(format!("{} is empty", path.display()).into())
    } else {
        Ok(text)
    }
}

fn write_json_if_requested<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if !is_requested_path(path) {
        return Ok(());
    }
    ensure_parent(path)?;
    fs::write(path, serde_json::to_string_pretty(value)? + "\n")?;
    Ok(())
}

fn read_json_array_if_requested(path: &Path) -> Result<Vec<Value>> {
    if !is_requested_path(path) || !path.is_file() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn read_json_value_if_requested(path: &Path) -> Result<Value> {
    if !is_requested_path(path) || !path.is_file() {
        return Ok(json!({}));
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn is_requested_path(path: &Path) -> bool {
    !path.as_os_str().is_empty() && path != Path::new(".")
}

fn validate_nonblank(value: &str, name: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(format!("{name} must not be blank").into())
    } else {
        Ok(())
    }
}

fn validate_repo(repository: &str) -> Result<()> {
    Regex::new(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")?
        .is_match(repository)
        .then_some(())
        .ok_or_else(|| format!("invalid repository {repository}").into())
}

fn validate_url(url: &str) -> Result<()> {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        Ok(())
    } else {
        Err(format!("invalid URL {url}").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_policy_blocks_degraded_required() {
        let path = temp_file("policy-test").unwrap();
        let args = PublicationArgs {
            synthesis_required: "true".into(),
            synthesis_strict: "false".into(),
            synth_succeeded: "true".into(),
            synth_quality: "degraded".into(),
            synth_failure_stage: "".into(),
            synth_failure_message: "".into(),
            github_output: path.clone(),
        };
        assert!(publication_policy(args).is_err());
        let outputs = parse_outputs(&path).unwrap();
        assert_eq!(outputs["can_update_release"], "false");
    }

    #[test]
    fn release_body_replaces_existing_whats_new() {
        let body = compose_release_body(
            "## Better\n\n- New",
            "## What's New\n\nold\n\n## Technical\n\nraw",
        );
        assert!(body.contains("## Better"));
        assert!(!body.contains("old"));
        assert!(body.contains("## Technical"));
    }

    #[test]
    fn markdown_filters_unsafe_links() {
        let html =
            markdown_to_html_fragment("[bad](javascript:alert(1)) [ok](https://example.com)");
        assert!(html.contains("href=\"#\""));
        assert!(html.contains("href=\"https://example.com\""));
    }

    #[test]
    fn typed_artifact_renders_shared_outputs() {
        let artifact = ReleaseNoteArtifact::from_markdown(
            "v1.2.3",
            "## Added\n\n- See [docs](https://example.com) and [bad](javascript:alert(1))",
        );
        assert_eq!(artifact.version, "1.2.3");
        assert!(artifact.html.contains("href=\"https://example.com\""));
        assert!(artifact.html.contains("href=\"#\""));
        assert!(artifact.plaintext.contains("See docs and bad"));
        assert!(artifact.slack.contains("<https://example.com|docs>"));
        assert!(!artifact.slack.contains("javascript:"));
        assert_eq!(artifact.sections[0].title, "Added");
        assert_eq!(
            artifact.sections[0].bullets[0].links[0].href,
            "https://example.com"
        );
        assert!(artifact.json_entry()["sections"].is_array());
    }

    #[test]
    fn local_provider_version_decision_uses_conventional_commits() {
        let latest = BackfillTag {
            tag: "v1.2.3".into(),
            version: "1.2.3".into(),
            key: (1, 2, 3),
            package: String::new(),
            prerelease: false,
        };
        assert_eq!(
            decide_version_bump(&[RunCommit {
                subject: "feat(cli): add local run".into(),
                short_hash: String::new(),
                body: String::new(),
            }]),
            "minor"
        );
        assert_eq!(
            decide_version_bump(&[RunCommit {
                subject: "fix(action): patch output".into(),
                short_hash: String::new(),
                body: String::new(),
            }]),
            "patch"
        );
        assert_eq!(
            decide_version_bump(&[RunCommit {
                subject: "feat(api)!: change provider contract".into(),
                short_hash: String::new(),
                body: String::new(),
            }]),
            "major"
        );
        assert_eq!(
            decide_version_bump(&[RunCommit {
                subject: "feat(api): rename field".into(),
                short_hash: String::new(),
                body: "BREAKING CHANGE: clients must migrate field names".into(),
            }]),
            "major"
        );
        assert_eq!(next_release_tag(Some(&latest), "minor"), "v1.3.0");
        assert_eq!(next_release_tag(Some(&latest), "patch"), "v1.2.4");
        assert_eq!(next_release_tag(Some(&latest), "major"), "v2.0.0");
    }

    #[test]
    fn summary_status_includes_attempts_and_destinations() {
        let output = temp_file("summary-test").unwrap();
        let attempts = temp_file("attempts-test").unwrap();
        fs::write(
            &attempts,
            r#"[{"model":"primary","succeeded":false},{"model":"fallback","succeeded":true}]"#,
        )
        .unwrap();
        let args = SummaryArgs {
            synthesis_enabled: "true".into(),
            released: "true".into(),
            synth_succeeded: "true".into(),
            synth_quality: "valid".into(),
            update_succeeded: "true".into(),
            synth_failure_stage: "".into(),
            synth_failure_message: "".into(),
            update_failure_stage: "".into(),
            update_failure_message: "".into(),
            artifact_succeeded: "true".into(),
            artifact_failure_stage: "".into(),
            artifact_failure_message: "".into(),
            rss_enabled: "true".into(),
            rss_succeeded: "false".into(),
            rss_failure_stage: "rss_update".into(),
            rss_failure_message: "push failed".into(),
            webhook_enabled: "true".into(),
            webhook_sent: "true".into(),
            slack_enabled: "true".into(),
            slack_sent: "false".into(),
            github_output: output.clone(),
            attempts_file: attempts.to_string_lossy().into_owned(),
            context_metadata_file: ".".into(),
        };
        summary_policy(args).unwrap();
        let outputs = parse_outputs(&output).unwrap();
        assert_eq!(outputs["succeeded"], "false");
        let status: Value = serde_json::from_str(&outputs["status_json"]).unwrap();
        assert_eq!(status["model_attempts"].as_array().unwrap().len(), 2);
        assert_eq!(status["destinations"]["rss"]["enabled"], true);
        assert_eq!(status["destinations"]["rss"]["failure_stage"], "rss_update");
        assert_eq!(status["destinations"]["webhook"]["succeeded"], true);
        assert_eq!(status["destinations"]["slack"]["succeeded"], false);
    }

    #[test]
    fn summary_no_release_accepts_empty_artifact_paths() {
        let output = temp_file("summary-no-release").unwrap();
        let cli = Cli::try_parse_from([
            "landmark",
            "release-policy",
            "summary",
            "--synthesis-enabled",
            "true",
            "--released",
            "false",
            "--synth-succeeded",
            "",
            "--update-succeeded",
            "",
            "--github-output",
            output.to_str().unwrap(),
            "--attempts-file",
            "",
            "--context-metadata-file",
            "",
        ])
        .unwrap();
        let Commands::ReleasePolicy(ReleasePolicyArgs {
            command: ReleasePolicyCommand::Summary(args),
        }) = cli.command
        else {
            panic!("expected release-policy summary command");
        };

        summary_policy(*args).unwrap();

        let outputs = parse_outputs(&output).unwrap();
        assert_eq!(outputs["succeeded"], "true");
        assert_eq!(outputs["failure_stage"], "");
        let status: Value = serde_json::from_str(&outputs["status_json"]).unwrap();
        assert_eq!(status["released"], false);
        assert_eq!(status["model_attempts"].as_array().unwrap().len(), 0);
        assert_eq!(status["context"], json!({}));
    }

    #[test]
    fn setup_detects_changesets_monorepo_and_generates_matrix_workflow() {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir(repo.path().join(".changeset")).unwrap();
        fs::write(
            repo.path().join("package.json"),
            r#"{"name":"demo","workspaces":["packages/*"]}"#,
        )
        .unwrap();
        let diagnosis = diagnose_setup(repo.path());
        assert_eq!(diagnosis.release_tool, "changesets");
        assert!(diagnosis.monorepo);
        let recommendation = recommend_setup(&diagnosis, None);
        assert_eq!(recommendation.workflow, "changesets-monorepo");
        let workflows = setup_workflows(&diagnosis, None);
        let changesets = &workflows["changesets"].content;
        assert!(
            changesets.contains("fromJson(needs.release.outputs.published_packages)[0].version")
        );
        assert!(!changesets.contains("${{tag}}"));
        assert!(!changesets.contains("python3"));
        let workflow = &workflows["changesets-monorepo"].content;
        assert!(workflow.contains("strategy:"));
        assert!(workflow.contains("healthcheck: 'true'"));
        assert!(workflow.contains("pull-requests: write"));
        assert!(workflow.contains("NPM_TOKEN"));
    }

    #[test]
    fn setup_detects_semantic_release_and_reports_backfill_available() {
        let repo = tempfile::tempdir().unwrap();
        fs::write(
            repo.path().join("package.json"),
            r#"{"name":"demo","devDependencies":{"semantic-release":"^24.0.0"}}"#,
        )
        .unwrap();
        let diagnosis = diagnose_setup(repo.path());
        assert_eq!(diagnosis.release_tool, "semantic-release");
        assert_eq!(recommend_setup(&diagnosis, None).mode, "full");
        let workflow = &setup_workflows(&diagnosis, None)["semantic-release"].content;
        assert!(workflow.contains("mode: full"));
        assert!(workflow.contains("healthcheck: 'true'"));
        assert!(workflow.contains("GH_RELEASE_TOKEN"));
    }

    #[test]
    fn fleet_plan_patches_existing_release_please_workflow() {
        let mut repo = fleet_fixture_repo(
            "misty-step/release-please-app",
            "release-please",
            ("application", "github-release"),
            (false, false),
            "unprotected-or-unavailable",
            &[],
            &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
        );
        repo.workflow_files = vec![
            fleet_workflow_file(
                ".github/workflows/release-please.yml",
                &existing_release_please_workflow(),
            )
            .expect("release-please workflow fixture"),
        ];

        let plan = plan_fleet_repository(&repo);

        assert_eq!(plan.status, "ready");
        assert_eq!(plan.workflow_patches.len(), 1);
        let patch = &plan.workflow_patches[0];
        assert_eq!(patch.path, ".github/workflows/release-please.yml");
        serde_yaml::from_str::<serde_yaml::Value>(&patch.content).unwrap();
        assert!(patch.content.contains("Existing Release"));
        assert_eq!(
            patch
                .content
                .matches("googleapis/release-please-action")
                .count(),
            1
        );
        assert!(patch.content.contains("needs: release-please"));
        assert!(patch.content.contains("mode: synthesis-only"));
        assert!(patch.content.contains("healthcheck: 'true'"));
    }

    #[test]
    fn fleet_plan_patches_existing_changesets_workflow() {
        let mut repo = fleet_fixture_repo(
            "misty-step/changesets-app",
            "changesets",
            ("library", "github-release"),
            (false, false),
            "unprotected-or-unavailable",
            &[],
            &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
        );
        repo.workflow_files = vec![
            fleet_workflow_file(
                ".github/workflows/changesets.yml",
                &existing_changesets_workflow(),
            )
            .expect("changesets workflow fixture"),
        ];

        let plan = plan_fleet_repository(&repo);

        assert_eq!(plan.status, "ready");
        assert_eq!(plan.workflow_patches.len(), 1);
        let patch = &plan.workflow_patches[0];
        assert_eq!(patch.path, ".github/workflows/changesets.yml");
        serde_yaml::from_str::<serde_yaml::Value>(&patch.content).unwrap();
        assert!(patch.content.contains("Existing Release"));
        assert_eq!(patch.content.matches("changesets/action").count(), 1);
        assert!(patch.content.contains("needs: release"));
        assert!(patch.content.contains("mode: synthesis-only"));
        assert!(patch.content.contains("healthcheck: 'true'"));
    }

    #[test]
    fn fleet_plan_blocks_existing_semantic_release_workflow() {
        let plan = plan_fleet_repository(&fleet_existing_semantic_release_workflow_fixture());

        assert_eq!(plan.status, "blocked");
        assert_eq!(plan.integration_mode, "blocked");
        assert!(plan.workflow_patches.is_empty());
        assert!(
            plan.skip_reason
                .contains("existing semantic-release workflow")
        );
    }

    #[test]
    fn fleet_plan_blocks_secret_like_workflow_bodies() {
        let mut repo = fleet_fixture_repo(
            "misty-step/release-please-app",
            "release-please",
            ("application", "github-release"),
            (false, false),
            "unprotected-or-unavailable",
            &[],
            &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
        );
        let mut workflow = existing_release_please_workflow();
        workflow.push_str("\n# ghp_1234567890abcdef\n");
        repo.workflow_files = vec![
            fleet_workflow_file(".github/workflows/release.yml", &workflow)
                .expect("release-please workflow fixture"),
        ];

        let plan = plan_fleet_repository(&repo);

        assert_eq!(plan.status, "blocked");
        assert!(plan.workflow_patches.is_empty());
        assert!(plan.skip_reason.contains("secret-like literals"));
        assert!(repo.workflow_files[0].content.is_empty());
        assert!(repo.workflow_files[0].content_redacted);
    }

    #[test]
    fn fleet_plan_readies_backfill_first_no_release_package_repositories() {
        for (name, kind, packages) in [
            (
                "phrazzld/no-release-ts-app",
                "application",
                vec!["package.json"],
            ),
            (
                "misty-step/no-release-rust-crate",
                "library",
                vec!["Cargo.toml"],
            ),
            ("phrazzld/no-release-go-app", "application", vec!["go.mod"]),
            (
                "misty-step/no-release-python-lib",
                "library",
                vec!["pyproject.toml"],
            ),
            (
                "phrazzld/no-release-multipackage",
                "library",
                vec!["Cargo.toml", "package.json", "packages/api/package.json"],
            ),
        ] {
            let repo = fleet_fixture_repo_with_packages(
                name,
                "no-release-tool",
                (kind, "none"),
                (false, false),
                "unprotected-or-unavailable",
                &packages,
                &[],
            );

            let plan = plan_fleet_repository(&repo);

            assert_eq!(plan.status, "ready");
            assert_eq!(plan.recommended_mode, "backfill-first");
            assert_eq!(plan.integration_mode, "backfill-first");
            assert!(plan.required_secrets.is_empty());
            assert!(plan.missing_secrets.is_empty());
            assert_eq!(plan.initial_version_recommendation, "0.1.0");
            assert!(!plan.initial_tag_recommendation.is_empty());
            if name == "phrazzld/no-release-multipackage" {
                assert_eq!(
                    plan.initial_tag_recommendation,
                    "no-release-multipackage@0.1.0"
                );
            } else {
                assert_eq!(plan.initial_tag_recommendation, "v0.1.0");
            }
            assert!(
                plan.artifact_paths
                    .iter()
                    .any(|path| path == "docs/releases/{version}.md")
            );
            assert!(
                plan.historical_preview_command
                    .contains("--mode artifacts-only --dry-run")
            );
            assert!(
                plan.historical_preview_command
                    .contains(&plan.initial_tag_recommendation)
            );
            assert!(plan.rollback_guidance.contains("close the PR"));
            assert!(
                plan.rollback_guidance
                    .contains("previewed local artifact files")
            );
            assert!(
                plan.migration_notes
                    .iter()
                    .any(|note| note.contains("operator-approved initial tag"))
            );
            assert_eq!(
                plan.manifest.release.profile.as_deref(),
                Some("synthesis-only")
            );
        }
    }

    #[test]
    fn fleet_plan_keeps_no_package_no_release_repositories_skipped() {
        let repo = fleet_fixture_repo_with_packages(
            "misty-step/docs-site",
            "no-release-tool",
            ("non-release", "none"),
            (false, false),
            "unprotected-or-unavailable",
            &[],
            &[],
        );

        let plan = plan_fleet_repository(&repo);

        assert_eq!(plan.status, "skipped");
        assert_eq!(plan.recommended_mode, "skipped");
        assert!(plan.initial_version_recommendation.is_empty());
        assert!(plan.initial_tag_recommendation.is_empty());
        assert!(plan.artifact_paths.is_empty());
        assert!(plan.historical_preview_command.is_empty());
    }

    #[test]
    fn fleet_backfill_guidance_helpers_handle_empty_and_tag_variants() {
        let mut repo = fleet_fixture_repo_with_packages(
            "misty-step/library",
            "no-release-tool",
            ("library", "none"),
            (false, false),
            "unprotected-or-unavailable",
            &["Cargo.toml"],
            &[],
        );

        assert!(fleet_initial_version("skipped", "skipped").is_empty());
        assert!(fleet_initial_tag(&repo, "").is_empty());
        assert!(fleet_historical_preview_command("").is_empty());

        repo.tag_format = "{version}".into();
        assert_eq!(fleet_initial_tag(&repo, "0.1.0"), "0.1.0");
        repo.tag_format = "package@{version}".into();
        assert_eq!(fleet_initial_tag(&repo, "0.1.0"), "library@0.1.0");
        repo.tag_format = "custom".into();
        assert_eq!(
            fleet_initial_tag(&repo, "0.1.0"),
            "0.1.0 (custom tag format requires operator approval)"
        );
    }

    #[test]
    fn org_secret_visibility_all_counts_for_fleet_secret_metadata() {
        let metadata = json!({
            "secrets": [
                {"name": "GH_RELEASE_TOKEN", "visibility": "all"},
                {"name": "OPENROUTER_API_KEY", "visibility": "selected", "selected_repositories": [
                    {"full_name": "misty-step/landmark", "name": "landmark"}
                ]},
                {"name": "UNRELATED", "visibility": "private"}
            ]
        });

        let names = org_secret_names_for_repo(&metadata, "misty-step/landmark", "landmark");
        assert!(names.contains("GH_RELEASE_TOKEN"));
        assert!(names.contains("OPENROUTER_API_KEY"));
        assert!(!names.contains("UNRELATED"));

        let other = org_secret_names_for_repo(&metadata, "misty-step/other", "other");
        assert!(other.contains("GH_RELEASE_TOKEN"));
        assert!(!other.contains("OPENROUTER_API_KEY"));
    }

    #[test]
    fn fleet_detects_landmark_and_legacy_landmark_release_workflow_content() {
        for action_ref in ["misty-step/landmark@v1", "misty-step/landmark@v1"] {
            let workflow_texts = vec![(
                "release.yml".to_string(),
                format!("steps:\n  - uses: {action_ref}\n"),
            )];

            assert!(workflow_invokes_landmark(&workflow_texts[0].1));
            assert_eq!(
                fleet_release_tool(
                    &[],
                    &["release.yml".into()],
                    &workflow_texts,
                    &["v1.2.3".into()]
                ),
                "manual-tag"
            );
        }
    }

    #[test]
    fn fleet_classifiers_distinguish_rollout_kinds_and_surfaces() {
        assert_eq!(
            classify_fleet_repository_kind("release-docs", &[], &[]),
            "non-release"
        );
        assert_eq!(
            classify_fleet_repository_kind("terraform-infra", &["go.mod".into()], &[]),
            "infrastructure"
        );
        assert_eq!(
            classify_fleet_repository_kind("search-experiment", &["Cargo.toml".into()], &[]),
            "experiment"
        );
        assert_eq!(
            classify_fleet_repository_kind("widget-crate", &["Cargo.toml".into()], &[]),
            "library"
        );
        assert_eq!(
            classify_fleet_repository_kind("billing-app", &["package.json".into()], &[]),
            "application"
        );
        assert_eq!(
            classify_fleet_release_surface(
                "changesets",
                &[],
                &[("release.yml".into(), "npm publish".into())],
            ),
            "package-registry"
        );
        assert_eq!(
            classify_fleet_release_surface("semantic-release", &[], &[]),
            "github-release+semantic-release"
        );
        assert_eq!(
            classify_fleet_release_surface("no-release-tool", &[], &[]),
            "none"
        );
    }

    #[test]
    fn init_manifest_infers_product_context_from_repo_metadata() {
        let repo = tempfile::tempdir().unwrap();
        fs::write(
            repo.path().join("package.json"),
            r#"{"name":"@mistystep/atlas","description":"Release operations for app fleets."}"#,
        )
        .unwrap();
        fs::write(
            repo.path().join("README.md"),
            "# Atlas\n\nLandmark-managed release automation.\n",
        )
        .unwrap();

        let manifest = infer_manifest(repo.path());
        assert_eq!(manifest.product.name.as_deref(), Some("Atlas"));
        assert_eq!(
            manifest.product.description.as_deref(),
            Some("Release operations for app fleets.")
        );
        assert_eq!(manifest.audience.as_deref(), Some("developer"));
        assert_eq!(manifest.changelog.source.as_deref(), Some("auto"));

        let rendered = render_manifest_yaml(&manifest).unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(&rendered).unwrap();
        assert_eq!(parsed["product"]["name"], "Atlas");
        assert_eq!(parsed["model"]["policy"], "balanced");
    }

    #[test]
    fn setup_projects_manifest_defaults_into_generated_workflows() {
        let diagnosis = SetupDiagnosis {
            release_tool: "semantic-release".into(),
            default_branch: "master".into(),
            tag_format: "v{version}".into(),
            conventional_commits: "ready".into(),
            monorepo: false,
            packages: vec!["landmark".into()],
            signals: Vec::new(),
        };
        let manifest = LandmarkManifest {
            product: ProductManifest {
                name: Some("Landmark".into()),
                description: Some("Release notes and changelog automation.".into()),
            },
            audience: Some("enterprise".into()),
            voice: Some("plainspoken, specific, operator-facing".into()),
            changelog: ChangelogManifest {
                source: Some("release-body".into()),
            },
            artifacts: ArtifactManifest {
                markdown: Some("docs/releases/{version}.md".into()),
                plaintext: None,
                html: Some("docs/releases/{version}.html".into()),
                json: None,
                rss: Some("docs/releases/feed.xml".into()),
            },
            release: ReleaseManifest {
                profile: Some("full".into()),
            },
            model: ModelManifest {
                policy: Some("cheap".into()),
                primary: Some("openai/gpt-4o-mini".into()),
                fallbacks: vec!["google/gemini-2.5-flash".into()],
            },
            budget: BudgetManifest {
                max_input_tokens: Some(8000),
                max_output_tokens: Some(900),
                max_usd: Some(0.05),
            },
        };

        let workflows = setup_workflows(&diagnosis, Some(&manifest));
        let workflow = &workflows["semantic-release"].content;
        assert!(workflow.contains("product-description: Release notes and changelog automation."));
        assert!(workflow.contains("audience: enterprise"));
        assert!(workflow.contains("voice-guide: plainspoken, specific, operator-facing"));
        assert!(workflow.contains("changelog-source: release-body"));
        assert!(workflow.contains("notes-output-file: docs/releases/{version}.md"));
        assert!(workflow.contains("notes-output-html-file: docs/releases/{version}.html"));
        assert!(workflow.contains("rss-feed-file: docs/releases/feed.xml"));
        assert!(workflow.contains("llm-model: openai/gpt-4o-mini"));
        assert!(workflow.contains("llm-fallback-models: google/gemini-2.5-flash"));
        let release_please = &workflows["release-please"].content;
        assert_eq!(release_please.matches("changelog-source:").count(), 1);
        assert!(release_please.contains("changelog-source: release-body"));

        let mut synthesis_only_manifest = manifest.clone();
        synthesis_only_manifest.release.profile = Some("synthesis-only".into());
        let recommendation = recommend_setup(&diagnosis, Some(&synthesis_only_manifest));
        assert_eq!(recommendation.mode, "synthesis-only");
        assert!(
            recommendation
                .rationale
                .contains(&"manifest release profile: synthesis-only".into())
        );
    }

    #[test]
    fn synthesis_manifest_defaults_keep_explicit_cli_precedence() {
        let repo = tempfile::tempdir().unwrap();
        fs::write(
            repo.path().join(".landmark.yml"),
            r#"product:
  name: Manifest Product
  description: Manifest description
audience: enterprise
voice: Manifest voice
changelog:
  source: release-body
model:
  policy: cheap
  primary: manifest/model
  fallbacks:
    - manifest/fallback
"#,
        )
        .unwrap();
        let mut args = SynthesizeArgs {
            api_key: "test".into(),
            model: String::new(),
            model_policy: String::new(),
            api_url: "http://example.invalid".into(),
            fallback_models: String::new(),
            product_name: String::new(),
            product_description: String::new(),
            voice_guide: String::new(),
            audience: None,
            changelog_source: None,
            version: "v1.2.3".into(),
            changelog_file: repo.path().join("CHANGELOG.md"),
            release_body_file: repo.path().join("release.md"),
            pr_changelog_file: PathBuf::from("."),
            prompt_template: PathBuf::from("."),
            quality_file: repo.path().join("quality.txt"),
            attempts_file: PathBuf::from("."),
            templates_dir: PathBuf::from("templates/prompts"),
            repo_root: repo.path().to_path_buf(),
            dry_run_cost: false,
            context_metadata_file: PathBuf::from("."),
        };
        let defaults = resolve_synthesis_config(&args).unwrap();
        assert_eq!(defaults.product_name, "Manifest Product");
        assert_eq!(defaults.product_description, "Manifest description");
        assert_eq!(defaults.voice_guide, "Manifest voice");
        assert_eq!(defaults.audience, "enterprise");
        assert_eq!(defaults.changelog_source, "release-body");
        assert_eq!(defaults.model, "manifest/model");
        assert_eq!(defaults.fallback_models, "manifest/fallback");

        args.audience = Some("developer".into());
        args.changelog_source = Some("prs".into());
        args.product_description = "Explicit description".into();
        args.model = "explicit/model".into();
        let explicit = resolve_synthesis_config(&args).unwrap();
        assert_eq!(explicit.product_description, "Explicit description");
        assert_eq!(explicit.audience, "developer");
        assert_eq!(explicit.changelog_source, "prs");
        assert_eq!(explicit.model, "explicit/model");

        args.model = String::new();
        args.audience = None;
        args.changelog_source = None;
        let mut manifest: LandmarkManifest =
            serde_yaml::from_str(&fs::read_to_string(repo.path().join(".landmark.yml")).unwrap())
                .unwrap();
        manifest.model.primary = None;
        manifest.model.policy = Some("rich".into());
        fs::write(
            repo.path().join(".landmark.yml"),
            render_manifest_yaml(&manifest).unwrap(),
        )
        .unwrap();
        let policy_default = resolve_synthesis_config(&args).unwrap();
        assert_eq!(policy_default.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn release_classifier_defaults_unknown_context_to_user_visible_medium() {
        let classification = classify_release_context("## [1.2.3]\n\n- improve output\n", &[]);

        assert!(classification.user_visible);
        assert_eq!(classification.significance, "medium");
        assert!(
            classification
                .categories
                .iter()
                .any(|category| category == "user-visible")
        );
    }

    #[test]
    fn synthesis_budget_preserves_existing_policy_skip_reason() {
        let config = test_synthesis_config("off", Some(1), Some(0.0));
        let classification = test_release_classification("medium");
        let cost = estimate_synthesis_cost(&config, "a long enough prompt", &classification, &[]);

        assert!(cost.skip);
        assert_eq!(cost.skip_reason, "model.policy=off disables LLM synthesis");
    }

    #[test]
    fn synthesis_budget_counts_rendered_prompt_once() {
        let prompt = "abcd".repeat(100);
        let max_input_tokens = estimate_tokens(&prompt);
        let config = test_synthesis_config("balanced", Some(max_input_tokens), None);
        let classification = test_release_classification("medium");
        let prompt_source = ContextSource {
            name: "prompt_template".into(),
            kind: "prompt".into(),
            bytes: prompt.len(),
            estimated_tokens: max_input_tokens,
            included: true,
        };

        let cost = estimate_synthesis_cost(&config, &prompt, &classification, &[prompt_source]);

        assert!(!cost.skip, "{:?}", cost);
        assert_eq!(cost.input_tokens, max_input_tokens);
    }

    #[test]
    fn manifest_validation_rejects_multiline_action_scalars() {
        let manifest = LandmarkManifest {
            product: ProductManifest {
                name: Some("Demo".into()),
                description: Some("first line\nsecond line".into()),
            },
            audience: Some("developer".into()),
            voice: Some("clear".into()),
            changelog: ChangelogManifest {
                source: Some("auto".into()),
            },
            artifacts: ArtifactManifest::default(),
            release: ReleaseManifest::default(),
            model: ModelManifest::default(),
            budget: BudgetManifest::default(),
        };
        let errors = validate_manifest(&manifest);
        assert!(errors.iter().any(|error| error.contains("single-line")));
    }

    #[test]
    fn manifest_validation_rejects_unsupported_policy_and_profile() {
        let manifest = LandmarkManifest {
            product: ProductManifest {
                name: Some("Demo".into()),
                description: Some("Demo app".into()),
            },
            audience: Some("developer".into()),
            voice: Some("clear".into()),
            changelog: ChangelogManifest {
                source: Some("auto".into()),
            },
            artifacts: ArtifactManifest::default(),
            release: ReleaseManifest {
                profile: Some("banana".into()),
            },
            model: ModelManifest {
                policy: Some("banana".into()),
                primary: None,
                fallbacks: Vec::new(),
            },
            budget: BudgetManifest::default(),
        };
        let errors = validate_manifest(&manifest);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("release.profile must be full or synthesis-only"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("model.policy must be cheap, balanced, rich, or off"))
        );
    }

    #[test]
    fn manifest_shape_rejects_unknown_keys() {
        let raw: serde_yaml::Value = serde_yaml::from_str(
            "product:\n  name: Demo\n  description: Demo app\n  tagline: nope\nrelease:\n  profile: synthesis-only\nsurprise: true\n",
        )
        .unwrap();
        let errors = validate_manifest_yaml_shape(&raw);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("manifest contains unknown key `surprise`"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("manifest.product contains unknown key `tagline`"))
        );
    }

    #[test]
    fn failure_classifier_emits_stable_codes_and_redacts_tokens() {
        let auth = classify_failure("--publish-release-body requires --github-token");
        assert_eq!(auth.code, "provider_auth");
        assert_eq!(auth.stage, "provider");
        assert!(!auth.retryable);

        let changelog = classify_failure("manifest changelog.source must be auto");
        assert_eq!(changelog.code, "invalid_changelog_source");
        assert_eq!(changelog.stage, "configuration");

        let redacted =
            redact_context("request failed with ghp_123456789abcdef and sk-123456789abcdef");
        assert!(!redacted.contains("ghp_123456789abcdef"));
        assert!(!redacted.contains("sk-123456789abcdef"));
        assert!(redacted.contains("[REDACTED]"));
    }

    fn test_synthesis_config(
        model_policy: &str,
        max_input_tokens: Option<u64>,
        max_usd: Option<f64>,
    ) -> EffectiveSynthesisConfig {
        EffectiveSynthesisConfig {
            product_name: "Demo".into(),
            product_description: "Demo release automation.".into(),
            voice_guide: String::new(),
            audience: "developer".into(),
            changelog_source: "auto".into(),
            model_policy: model_policy.into(),
            model: "primary/model".into(),
            fallback_models: String::new(),
            max_input_tokens,
            max_output_tokens: None,
            max_usd,
        }
    }

    fn test_release_classification(significance: &str) -> ReleaseClassification {
        ReleaseClassification {
            categories: vec!["user-visible".into()],
            significance: significance.into(),
            user_visible: true,
            breaking: false,
            security: false,
            migration_heavy: false,
            reasons: Vec::new(),
        }
    }

    #[test]
    fn setup_generated_workflows_are_yaml() {
        let diagnosis = SetupDiagnosis {
            release_tool: "manual-tag".into(),
            default_branch: "main".into(),
            tag_format: "v{version}".into(),
            conventional_commits: "ready".into(),
            monorepo: true,
            packages: vec!["pkg-a".into(), "pkg-b".into()],
            signals: Vec::new(),
        };
        for candidate in setup_workflows(&diagnosis, None).values() {
            let parsed: serde_yaml::Value = serde_yaml::from_str(&candidate.content).unwrap();
            assert!(parsed["jobs"].is_mapping(), "{}", candidate.path);
        }
        let manual = &setup_workflows(&diagnosis, None)["manual-tag"].content;
        assert!(manual.contains("release:\n    types: [published]"));
        assert!(!manual.contains("push:\n    tags:"));
        assert!(manual.contains("${{ github.event.release.tag_name }}"));
        assert!(manual.contains("${{ secrets.GH_RELEASE_TOKEN }}"));
    }

    #[test]
    fn self_release_bump_ignores_non_release_commits() {
        assert!(classify_release_commit("abcdef0", "docs: update readme", "").is_none());
    }

    #[test]
    fn self_release_bump_detects_breaking_major() {
        let fix = classify_release_commit("abcdef0", "fix(runtime): close leak", "").unwrap();
        let feat = classify_release_commit("abcdef1", "feat(setup): add analyzer", "").unwrap();
        let breaking = classify_release_commit("abcdef2", "feat(api)!: rename output", "").unwrap();
        let bump = release_bump(&[fix, feat, breaking]).unwrap();
        assert!(matches!(bump, ReleaseBump::Major));
        assert_eq!(bump_version("1.2.3", bump).unwrap(), "2.0.0");
    }

    #[test]
    fn changelog_section_extracts_only_requested_version() {
        let repo = tempfile::tempdir().unwrap();
        let path = repo.path().join("CHANGELOG.md");
        fs::write(
            &path,
            "# [1.2.0](compare) (2026-06-12)\n\n### Features\n\n* new\n\n## [1.1.0](compare) (2026-06-11)\n\n### Bug Fixes\n\n* old\n",
        )
        .unwrap();
        let section = changelog_section(&path, "1.2.0").unwrap();
        assert!(section.contains("* new"));
        assert!(!section.contains("* old"));
    }

    #[test]
    fn cargo_lock_version_update_targets_landmark_package_only() {
        let repo = tempfile::tempdir().unwrap();
        let path = repo.path().join("Cargo.lock");
        fs::write(
            &path,
            "[[package]]\nname = \"dep\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"landmark\"\nversion = \"1.2.3\"\n",
        )
        .unwrap();
        update_lock_package_version(&path, "landmark", "1.3.0").unwrap();
        let text = fs::read_to_string(path).unwrap();
        assert!(text.contains("name = \"dep\"\nversion = \"0.1.0\""));
        assert!(text.contains("name = \"landmark\"\nversion = \"1.3.0\""));
    }

    #[test]
    fn version_sync_allows_explicit_release_candidate() {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join("crates/landmark")).unwrap();
        run_ok("git", ["init", "-q"], repo.path()).unwrap();
        run_ok("git", ["config", "user.name", "Landmark Test"], repo.path()).unwrap();
        run_ok(
            "git",
            ["config", "user.email", "landmark@example.invalid"],
            repo.path(),
        )
        .unwrap();
        fs::write(
            repo.path().join("package.json"),
            r#"{"name":"landmark","version":"1.18.0"}"#,
        )
        .unwrap();
        fs::write(
            repo.path().join("crates/landmark/Cargo.toml"),
            "[package]\nname = \"landmark\"\nversion = \"1.18.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(
            repo.path().join("CHANGELOG.md"),
            "# [1.18.0](compare) (2026-06-12)\n\n### Features\n\n* release candidate\n",
        )
        .unwrap();
        run_ok("git", ["add", "."], repo.path()).unwrap();
        run_ok(
            "git",
            ["commit", "-q", "-m", "chore: candidate"],
            repo.path(),
        )
        .unwrap();
        run_ok("git", ["tag", "v1.17.2"], repo.path()).unwrap();
        let strict = CheckVersionArgs {
            reference: "HEAD".into(),
            repo_root: repo.path().to_path_buf(),
            allow_release_candidate: false,
        };
        assert!(check_version_sync(strict).is_err());
        let candidate = CheckVersionArgs {
            reference: "HEAD".into(),
            repo_root: repo.path().to_path_buf(),
            allow_release_candidate: true,
        };
        assert!(check_version_sync(candidate).is_ok());

        fs::write(
            repo.path().join("CHANGELOG.md"),
            "# [1.18.0](compare) (2026-06-12)\n\n### Features\n\n",
        )
        .unwrap();
        let missing_entry = CheckVersionArgs {
            reference: "HEAD".into(),
            repo_root: repo.path().to_path_buf(),
            allow_release_candidate: true,
        };
        assert!(check_version_sync(missing_entry).is_err());

        fs::write(
            repo.path().join("CHANGELOG.md"),
            "# [1.18.0](compare) (2026-06-12)\n\n### Features\n\n* release candidate\n",
        )
        .unwrap();
        fs::write(
            repo.path().join("crates/landmark/Cargo.toml"),
            "[package]\nname = \"landmark\"\nversion = \"1.17.9\"\nedition = \"2024\"\n",
        )
        .unwrap();
        let mismatched_metadata = CheckVersionArgs {
            reference: "HEAD".into(),
            repo_root: repo.path().to_path_buf(),
            allow_release_candidate: true,
        };
        assert!(check_version_sync(mismatched_metadata).is_err());
    }

    #[test]
    fn floating_tag_skips_prerelease() {
        assert_eq!(parse_major_tag("v1.2.3").as_deref(), Some("v1"));
        assert_eq!(parse_major_tag("v1.2.3-beta.1"), None);
    }
}
