use chrono::Utc;
use clap::{Args, Parser, Subcommand};
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
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tiny_http::{Header, Method, Response, Server};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const VALID_NOTES: &str = "## Improvements\n\n- Added a replay harness that checks release behavior in a disposable repo.\n- Captured release body updates, artifacts, tags, and structured logs.\n- Kept the run local so no production secrets or GitHub releases are touched.\n";
const INVALID_NOTES: &str = "hello, here are the release notes";
const LINUX_ACTION_TARGET: &str = "x86_64-unknown-linux-musl";

#[derive(Parser)]
#[command(name = "landfall")]
#[command(about = "Rust runtime for the Landfall release action")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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
struct InitArgs {
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
    #[arg(long, default_value = ".landfall.yml")]
    output: PathBuf,
    #[arg(long = "dry-run")]
    dry_run: bool,
}

#[derive(Args)]
struct DoctorArgs {
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
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
    attempts_file: PathBuf,
    #[arg(long = "context-metadata-file", default_value = ".")]
    context_metadata_file: PathBuf,
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
}

#[derive(Args)]
struct BackfillArgs {
    #[arg(long)]
    dry_run: bool,
}

#[derive(Args)]
struct SetupArgs {
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
    #[arg(long = "output-dir", default_value = "")]
    output_dir: String,
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
struct FleetScanArgs {
    #[arg(long)]
    owner: Vec<String>,
    #[arg(long, default_value = ".landfall/fleet.json")]
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
}

#[derive(Args)]
struct FleetPlanArgs {
    #[arg(long, default_value = ".landfall/fleet.json")]
    input: PathBuf,
    #[arg(long = "output-dir", default_value = ".landfall/fleet-plan")]
    output_dir: PathBuf,
}

#[derive(Args)]
struct FleetOpenPrsArgs {
    #[arg(long = "plan-dir", default_value = ".landfall/fleet-plan")]
    plan_dir: PathBuf,
    #[arg(long = "output-dir", default_value = ".landfall/fleet-plan/prs")]
    output_dir: PathBuf,
    #[arg(long = "dry-run")]
    dry_run: bool,
    #[arg(long = "max-prs", default_value_t = 0)]
    max_prs: usize,
}

#[derive(Args)]
struct PrepareSelfReleaseArgs {
    #[arg(long = "repo-root", default_value = ".")]
    repo_root: PathBuf,
    #[arg(long, default_value = "misty-step/landfall")]
    repository: String,
    #[arg(long = "release-branch", default_value = "landfall/self-release")]
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
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    match Cli::parse().command {
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
        Commands::Backfill(_) => {
            eprintln!(
                "backfill is retired from the core action surface; use release re-run or synthesis-only mode"
            );
            Ok(())
        }
        Commands::Setup(args) => setup(args),
        Commands::Fleet(args) => fleet(args),
        Commands::PrepareSelfRelease(args) => prepare_self_release(args),
        Commands::PublishSelfRelease(args) => publish_self_release(args),
    }
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
struct LandfallManifest {
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
    sources: Vec<ContextSource>,
    classification: ReleaseClassification,
    cost: CostEstimate,
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
    let manifest = load_manifest(&args.repo_root)?.ok_or(".landfall.yml is missing")?;
    let mut errors = validate_manifest(&manifest);
    errors.extend(validate_manifest_completeness(&manifest));
    if errors.is_empty() {
        println!("manifest ok");
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

fn infer_manifest(root: &Path) -> LandfallManifest {
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
    LandfallManifest {
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

fn render_manifest_yaml(manifest: &LandfallManifest) -> Result<String> {
    Ok(serde_yaml::to_string(manifest)?)
}

fn load_manifest(root: &Path) -> Result<Option<LandfallManifest>> {
    let path = root.join(".landfall.yml");
    if !path.is_file() {
        return Ok(None);
    }
    let manifest: LandfallManifest = serde_yaml::from_str(&fs::read_to_string(path)?)?;
    let errors = validate_manifest(&manifest);
    if errors.is_empty() {
        Ok(Some(manifest))
    } else {
        Err(errors.join("\n").into())
    }
}

fn validate_manifest(manifest: &LandfallManifest) -> Vec<String> {
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

fn validate_manifest_completeness(manifest: &LandfallManifest) -> Vec<String> {
    let mut errors = Vec::new();
    if manifest
        .product
        .name
        .as_deref()
        .and_then(trimmed_option)
        .is_none()
    {
        errors.push("manifest product.name is required for a complete Landfall manifest".into());
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

fn manifest_scalar_fields(manifest: &LandfallManifest) -> Vec<(&'static str, &str)> {
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
    manifest: Option<LandfallManifest>,
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
    existing_landfall: bool,
    required_secrets: Vec<FleetSecretStatus>,
    signals: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FleetSecretStatus {
    name: String,
    status: String,
    detail: String,
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
    rank: u64,
    default_branch: String,
    recommended_mode: String,
    workflow: String,
    status: String,
    skip_reason: String,
    risk_flags: Vec<String>,
    missing_secrets: Vec<String>,
    unavailable_secret_metadata: Vec<String>,
    migration_notes: Vec<String>,
    manifest: LandfallManifest,
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
    files: Vec<String>,
    skipped: bool,
    reason: String,
}

fn setup(args: SetupArgs) -> Result<()> {
    let diagnosis = diagnose_setup(&args.repo_root);
    let manifest = load_manifest(&args.repo_root)?;
    let recommendation = recommend_setup(&diagnosis, manifest.as_ref());
    let workflows = setup_workflows(&diagnosis, manifest.as_ref());
    if !args.output_dir.trim().is_empty() {
        let output_dir = args.repo_root.join(args.output_dir.trim());
        fs::create_dir_all(&output_dir)?;
        for candidate in workflows.values() {
            let filename = Path::new(&candidate.path)
                .file_name()
                .unwrap_or_else(|| OsStr::new("landfall-release.yml"));
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
        backfill: "retired: use release re-run or synthesis-only mode; no Python backfill script is part of the maintenance surface".into(),
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
    if !args.fixture.trim().is_empty() {
        let scan: FleetScan = serde_json::from_str(&fs::read_to_string(&args.fixture)?)?;
        write_json_if_requested(&args.output, &scan)?;
        print_fleet_scan_result(&args.output, &scan)?;
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
    print_fleet_scan_result(&args.output, &scan)?;
    Ok(())
}

fn fleet_plan(args: FleetPlanArgs) -> Result<()> {
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
    println!(
        "fleet plan wrote {} and {} ({} repositories)",
        plan_path.display(),
        args.output_dir.join("README.md").display(),
        plan.repositories.len()
    );
    Ok(())
}

fn fleet_open_prs(args: FleetOpenPrsArgs) -> Result<()> {
    if !args.dry_run {
        return Err(
            "fleet open-prs currently requires --dry-run; refusing to mutate remote repositories"
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
                files: vec!["SKIPPED.md".into()],
                skipped: true,
                reason,
            });
            continue;
        }
        opened += 1;
        let manifest = render_manifest_yaml(&repo.manifest)?;
        let workflow = fleet_workflow_for_plan(repo);
        fs::create_dir_all(repo_dir.join(".github/workflows"))?;
        fs::write(repo_dir.join(".landfall.yml"), &manifest)?;
        fs::write(
            repo_dir.join(".github/workflows/landfall-release.yml"),
            &workflow,
        )?;
        let diff = render_fleet_pr_diff(repo, &manifest, &workflow);
        fs::write(repo_dir.join("diff.md"), diff)?;
        rendered.push(FleetRepositoryPrPlan {
            repository: repo.repository.clone(),
            branch: format!("landfall/adopt-{}", repo.repository.replace('/', "-")),
            title: "chore(release): adopt Landfall".into(),
            files: vec![
                ".landfall.yml".into(),
                ".github/workflows/landfall-release.yml".into(),
                "diff.md".into(),
            ],
            skipped: false,
            reason: String::new(),
        });
    }
    let pr_plan = FleetPrPlan {
        generated_at: Utc::now().to_rfc3339(),
        dry_run: true,
        repositories: rendered,
    };
    fs::write(
        args.output_dir.join("open-prs.json"),
        serde_json::to_string_pretty(&pr_plan)? + "\n",
    )?;
    println!(
        "fleet dry-run wrote {} ({} repositories)",
        args.output_dir.join("open-prs.json").display(),
        pr_plan.repositories.len()
    );
    Ok(())
}

fn print_fleet_scan_result(path: &Path, scan: &FleetScan) -> Result<()> {
    if is_requested_path(path) {
        println!(
            "fleet scan wrote {} ({} repositories, {} warnings)",
            path.display(),
            scan.repositories.len(),
            scan.warnings.len()
        );
    } else {
        println!("{}", serde_json::to_string_pretty(scan)?);
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
    let paths = github_tree_paths(&name_with_owner, &default_branch, token)?;
    let path_set: BTreeSet<_> = paths.iter().map(String::as_str).collect();
    let workflows = paths
        .iter()
        .filter_map(|path| path.strip_prefix(".github/workflows/"))
        .filter(|name| name.ends_with(".yml") || name.ends_with(".yaml"))
        .map(str::to_string)
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
        ".landfall.yml",
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
    let tags = github_tags(&name_with_owner, token)?;
    let tag_format = fleet_tag_format(&tags, &package_topology);
    let release_tool = fleet_release_tool(&release_files, &workflows, &tags);
    let existing_landfall = release_files.iter().any(|file| file == ".landfall.yml")
        || workflows
            .iter()
            .any(|workflow| workflow.to_ascii_lowercase().contains("landfall"));
    let branch_protected = if deep_checks {
        github_branch_protection(&name_with_owner, &default_branch, api_base_url, token)
    } else {
        "unavailable: pass --deep-checks to query branch protection metadata".into()
    };
    let required_secrets = if deep_checks {
        github_secret_statuses(
            &name_with_owner,
            api_base_url,
            token,
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
        existing_landfall,
        required_secrets,
        signals,
    })
}

fn github_tree_paths(repository: &str, branch: &str, token: Option<&str>) -> Result<Vec<String>> {
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
        token,
    )?;
    Ok(serde_json::from_str(&output)?)
}

fn github_tags(repository: &str, token: Option<&str>) -> Result<Vec<String>> {
    let output = run_gh_ok(
        vec![
            "api".into(),
            format!("repos/{repository}/tags?per_page=30"),
            "--jq".into(),
            "[.[].name]".into(),
        ],
        token,
    )?;
    Ok(serde_json::from_str(&output)?)
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

fn github_branch_protection(
    repository: &str,
    branch: &str,
    api_base_url: &str,
    token: Option<&str>,
) -> String {
    let Some(token) = token else {
        return "unavailable: no GitHub token supplied".into();
    };
    let url = format!(
        "{}/repos/{}/branches/{}/protection",
        api_base_url.trim_end_matches('/'),
        repository,
        urlencoding::encode(branch)
    );
    match curl_json("GET", &url, Some(token), None) {
        Ok(response) if response.status == 200 => "protected".into(),
        Ok(response) if response.status == 404 => "unprotected-or-unavailable".into(),
        Ok(response) => format!("unavailable: HTTP {}", response.status),
        Err(error) => format!("unavailable: {error}"),
    }
}

fn github_secret_statuses(
    repository: &str,
    api_base_url: &str,
    token: Option<&str>,
    required: &[&str],
) -> Vec<FleetSecretStatus> {
    let Some(token) = token else {
        return unavailable_secret_statuses(
            required,
            "secret metadata requires a GitHub token with repository access",
        );
    };
    let url = format!(
        "{}/repos/{}/actions/secrets?per_page=100",
        api_base_url.trim_end_matches('/'),
        repository
    );
    let response = match curl_json("GET", &url, Some(token), None) {
        Ok(response) => response,
        Err(error) => {
            return unavailable_secret_statuses(
                required,
                &format!("secret metadata unavailable: {error}"),
            );
        }
    };
    if !(200..300).contains(&response.status) {
        return unavailable_secret_statuses(
            required,
            &format!(
                "GitHub returned HTTP {} for secret metadata",
                response.status
            ),
        );
    }
    let payload: Value = serde_json::from_str(&response.body).unwrap_or_else(|_| json!({}));
    let names: BTreeSet<_> = payload["secrets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|secret| secret["name"].as_str())
        .collect();
    required
        .iter()
        .map(|name| {
            if names.contains(name) {
                FleetSecretStatus {
                    name: (*name).into(),
                    status: "present".into(),
                    detail: "metadata only; value not read".into(),
                }
            } else {
                FleetSecretStatus {
                    name: (*name).into(),
                    status: "missing".into(),
                    detail: "required secret name is absent from Actions secret metadata".into(),
                }
            }
        })
        .collect()
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

fn fleet_release_tool(files: &[String], workflows: &[String], tags: &[String]) -> String {
    let haystack = files
        .iter()
        .chain(workflows)
        .map(|value| value.to_ascii_lowercase())
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

fn plan_fleet_repository(repo: &FleetRepository) -> FleetRepositoryPlan {
    let mut risk_flags = Vec::new();
    let mut migration_notes = Vec::new();
    let missing_secrets = repo
        .required_secrets
        .iter()
        .filter(|secret| secret.status == "missing")
        .map(|secret| secret.name.clone())
        .collect::<Vec<_>>();
    let unavailable_secret_metadata = repo
        .required_secrets
        .iter()
        .filter(|secret| secret.status == "unavailable")
        .map(|secret| secret.name.clone())
        .collect::<Vec<_>>();
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

    let (status, recommended_mode, workflow, skip_reason, rank) = if repo.archived {
        ("skipped", "skipped", "none", "repository is archived", 0u64)
    } else if let Some(reason) = secret_blocker.as_deref() {
        ("blocked", "blocked", "manual-tag", reason, 15u64)
    } else if repo.existing_landfall {
        migration_notes.push(
            "Landfall-like workflow or manifest already exists; inspect before replacing".into(),
        );
        ("ready", "manifest-only", "manual-tag", "", 65u64)
    } else {
        match repo.release_tool.as_str() {
            "semantic-release" => ("ready", "full", "semantic-release", "", 100u64),
            "release-please" => ("ready", "synthesis-only", "release-please", "", 85u64),
            "changesets" => ("ready", "synthesis-only", "changesets", "", 80u64),
            "manual-tag" => ("ready", "synthesis-only", "manual-tag", "", 60u64),
            "no-release-tool" => (
                "blocked",
                "backfill-first",
                "manual-tag",
                "no release tool or release tags detected",
                20u64,
            ),
            _ => (
                "blocked",
                "blocked",
                "manual-tag",
                "unknown release tooling",
                10u64,
            ),
        }
    };
    if repo.release_tool == "no-release-tool" {
        migration_notes
            .push("Choose release semantics before installing Landfall automation".into());
    }
    if repo.package_topology.len() > 1 {
        risk_flags.push("multi-package repository; validate tag format and artifact paths".into());
    }
    migration_notes.push(format!(
        "Detected release tool: {}; tag format: {}; default branch: {}",
        repo.release_tool, repo.tag_format, repo.default_branch
    ));
    let manifest = fleet_manifest(repo, recommended_mode);
    FleetRepositoryPlan {
        repository: repo.name_with_owner.clone(),
        rank,
        default_branch: repo.default_branch.clone(),
        recommended_mode: recommended_mode.into(),
        workflow: workflow.into(),
        status: status.into(),
        skip_reason: skip_reason.into(),
        risk_flags,
        missing_secrets,
        unavailable_secret_metadata,
        migration_notes,
        manifest,
    }
}

fn fleet_manifest(repo: &FleetRepository, mode: &str) -> LandfallManifest {
    LandfallManifest {
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
                if mode == "full" {
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
    let mut out = String::from("# Landfall Fleet Adoption Plan\n\n");
    out.push_str("## Summary\n\n");
    for (status, count) in &plan.summary {
        out.push_str(&format!("- {status}: {count}\n"));
    }
    out.push_str("\n## Repositories\n\n");
    for repo in &plan.repositories {
        out.push_str(&format!(
            "### {}\n\n- Rank: {}\n- Status: {}\n- Recommended mode: {}\n- Workflow: {}\n",
            repo.repository, repo.rank, repo.status, repo.recommended_mode, repo.workflow
        ));
        if !repo.skip_reason.is_empty() {
            out.push_str(&format!("- Skip reason: {}\n", repo.skip_reason));
        }
        if !repo.risk_flags.is_empty() {
            out.push_str(&format!("- Risk flags: {}\n", repo.risk_flags.join("; ")));
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

fn render_fleet_pr_diff(repo: &FleetRepositoryPlan, manifest: &str, workflow: &str) -> String {
    format!(
        "# {}\n\nDry-run branch: `landfall/adopt-{}`\n\n## Files\n\n### .landfall.yml\n\n```yaml\n{}\n```\n\n### .github/workflows/landfall-release.yml\n\n```yaml\n{}\n```\n\n## Notes\n\n{}\n",
        repo.repository,
        repo.repository.replace('/', "-"),
        manifest,
        workflow,
        repo.migration_notes
            .iter()
            .map(|note| format!("- {note}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
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
    manifest: Option<&LandfallManifest>,
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
    manifest: Option<&LandfallManifest>,
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
                path: format!(".github/workflows/landfall-{name}.yml"),
                release_tool: tool.into(),
                mode: mode.into(),
                rationale: vec![
                    "includes Landfall healthcheck before the release attempt".into(),
                    "declares contents/issues/pull-requests write permissions".into(),
                ],
                content,
            },
        );
    }
    workflows
}

fn workflow_semantic_release(branch: &str, manifest: Option<&LandfallManifest>) -> String {
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
      - uses: misty-step/landfall@v1
        with:
          mode: full
          healthcheck: "true"
          github-token: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
          llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
    )
}

fn workflow_release_please(branch: &str, manifest: Option<&LandfallManifest>) -> String {
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
  release-please:
    runs-on: ubuntu-latest
    outputs:
      release_created: ${{{{ steps.release.outputs.release_created }}}}
      tag_name: ${{{{ steps.release.outputs.tag_name }}}}
    steps:
      - uses: googleapis/release-please-action@v4
        id: release

  synthesize:
    needs: release-please
    if: needs.release-please.outputs.release_created == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: misty-step/landfall@v1
        with:
          mode: synthesis-only
          healthcheck: "true"
          release-tag: ${{{{ needs.release-please.outputs.tag_name }}}}
          github-token: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
          llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
    )
}

fn workflow_changesets(
    branch: &str,
    monorepo: bool,
    manifest: Option<&LandfallManifest>,
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
  release:
    runs-on: ubuntu-latest
    outputs:
      published: ${{{{ steps.changesets.outputs.published }}}}
      published_packages: ${{{{ steps.changesets.outputs.publishedPackages }}}}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - run: npm ci
      - uses: changesets/action@v1
        id: changesets
        with:
          publish: npm run release
        env:
          GITHUB_TOKEN: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
          NPM_TOKEN: ${{{{ secrets.NPM_TOKEN }}}}

  synthesize:
    needs: release
    if: needs.release.outputs.published == 'true'
    strategy:
      matrix:
        package: ${{{{ fromJson(needs.release.outputs.published_packages) }}}}
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: misty-step/landfall@v1
        with:
          mode: synthesis-only
          healthcheck: "true"
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
  release:
    runs-on: ubuntu-latest
    outputs:
      published: ${{{{ steps.changesets.outputs.published }}}}
      published_packages: ${{{{ steps.changesets.outputs.publishedPackages }}}}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - run: npm ci
      - uses: changesets/action@v1
        id: changesets
        with:
          publish: npm run release
        env:
          GITHUB_TOKEN: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
          NPM_TOKEN: ${{{{ secrets.NPM_TOKEN }}}}

  synthesize:
    needs: release
    if: needs.release.outputs.published == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: misty-step/landfall@v1
        with:
          mode: synthesis-only
          healthcheck: "true"
          release-tag: v${{{{ fromJson(needs.release.outputs.published_packages)[0].version }}}}
          github-token: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
          llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
        )
    }
}

fn workflow_manual_tag(manifest: Option<&LandfallManifest>) -> String {
    let manifest_inputs = render_manifest_action_inputs(manifest, 10, Some("auto"));
    format!(
        r#"name: Synthesize Release Notes

on:
  push:
    tags:
      - "v[0-9]*"
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
      - uses: misty-step/landfall@v1
        with:
          mode: synthesis-only
          healthcheck: "true"
          release-tag: ${{{{ github.event.release.tag_name || github.ref_name }}}}
          github-token: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
          llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
    )
}

fn render_manifest_action_inputs(
    manifest: Option<&LandfallManifest>,
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
        "landfall",
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
            "crates/landfall/Cargo.toml".into(),
            "Cargo.lock".into(),
            "dist/landfall".into(),
            "dist/landfall.sha256".into(),
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
    let dest = dist_dir.join("landfall");
    let temp = dist_dir.join(format!(
        ".landfall-{}-{}.tmp",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    fs::copy(&binary, &temp)?;
    fs::set_permissions(&temp, fs::metadata(&binary)?.permissions())?;
    fs::rename(&temp, &dest)?;

    let digest = hex::encode(Sha256::digest(fs::read(&dest)?));
    fs::write(
        dist_dir.join("landfall.sha256"),
        format!("{digest}  dist/landfall\n"),
    )?;
    Ok(())
}

fn build_action_binary(repo_root: &Path, target: &str) -> Result<PathBuf> {
    if target == LINUX_ACTION_TARGET && !rustc_host_target()?.contains("linux") {
        return Err(
            "refusing to build checked-in Linux action binary from a non-Linux host; run the release workflow or `bin/build-linux-action --write` so dist/landfall is produced in Linux, or pass --dist-target only for replay fixtures"
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
            "failed to build Landfall self-release action binary for {target}; install the Rust target and linker for {target}, then retry: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let binary = repo_root
        .join("target")
        .join(target)
        .join("release")
        .join("landfall");
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
    let cargo = cargo_version(&args.repo_root.join("crates/landfall/Cargo.toml"))
        .ok_or("crates/landfall/Cargo.toml missing package version")?;
    if cargo != package_version {
        return Err(format!(
            "package.json has {package_version}, crates/landfall/Cargo.toml has {cargo}"
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
    let existing_url = github_release_url(&args.api_base_url, &args.repository, &release_tag);
    let existing = curl_json("GET", &existing_url, Some(&args.github_token), None)?;
    if (200..300).contains(&existing.status) {
        let value: Value = serde_json::from_str(&existing.body)?;
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
    if existing.status != 404 {
        return Err(format!("GitHub release lookup failed with HTTP {}", existing.status).into());
    }

    let body = changelog_section(&args.repo_root.join("CHANGELOG.md"), &package_version)?;
    let create_url = format!(
        "{}/repos/{}/releases",
        args.api_base_url.trim_end_matches('/'),
        args.repository
    );
    let response = curl_json(
        "POST",
        &create_url,
        Some(&args.github_token),
        Some(&json!({
            "tag_name": release_tag,
            "target_commitish": args.target_sha,
            "name": release_tag,
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
    let publish = SelfReleasePublish {
        published: true,
        reason: "published release from landed release pull request".into(),
        latest_version,
        version: package_version,
        release_tag,
        release_url: value["html_url"].as_str().unwrap_or("").to_string(),
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

fn curl_json(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<&Value>,
) -> Result<HttpResponse> {
    let mut args = vec![
        "-sS".to_string(),
        "-L".to_string(),
        "-X".to_string(),
        method.to_string(),
        "-H".to_string(),
        "Accept: application/vnd.github+json".to_string(),
        "-H".to_string(),
        "User-Agent: landfall".to_string(),
        "-w".to_string(),
        "\n%{http_code}".to_string(),
    ];
    if let Some(token) = token {
        args.push("-H".to_string());
        args.push(format!("Authorization: Bearer {token}"));
    }
    if let Some(body) = body {
        args.push("-H".to_string());
        args.push("Content-Type: application/json".to_string());
        args.push("--data".to_string());
        args.push(body.to_string());
    }
    args.push(url.to_string());
    let output = Command::new("curl").args(args).output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string().into());
    }
    let raw = String::from_utf8(output.stdout)?;
    let (body, status) = raw.rsplit_once('\n').ok_or("curl status marker missing")?;
    Ok(HttpResponse {
        status: status.parse()?,
        body: body.to_string(),
    })
}

fn github_release_url(base: &str, repository: &str, tag: &str) -> String {
    format!(
        "{}/repos/{}/releases/tags/{}",
        base.trim_end_matches('/'),
        repository,
        urlencoding::encode(tag)
    )
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
    validate_repo(&args.repository)?;
    let url = github_release_url(&args.api_base_url, &args.repository, &args.release_tag);
    let response = curl_json("GET", &url, Some(&args.github_token), None)?;
    if response.status == 404 {
        ensure_parent(&args.output_file)?;
        fs::write(args.output_file, "")?;
        return Ok(());
    }
    if !(200..300).contains(&response.status) {
        return Err(format!("GitHub release fetch failed with HTTP {}", response.status).into());
    }
    let value: Value = serde_json::from_str(&response.body)?;
    ensure_parent(&args.output_file)?;
    fs::write(args.output_file, value["body"].as_str().unwrap_or(""))?;
    Ok(())
}

fn extract_prs(args: ExtractPrsArgs) -> Result<()> {
    validate_repo(&args.repository)?;
    let url = format!(
        "{}/repos/{}/pulls?state=closed&per_page=100",
        args.api_base_url.trim_end_matches('/'),
        args.repository
    );
    let response = curl_json("GET", &url, Some(&args.github_token), None)?;
    if !(200..300).contains(&response.status) {
        return Err(format!("GitHub PR fetch failed with HTTP {}", response.status).into());
    }
    let prs: Vec<Value> = serde_json::from_str(&response.body)?;
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
        sources,
        classification,
        cost,
    }
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
        model_attempts: read_json_array_if_requested(&args.attempts_file)?,
        context: read_json_value_if_requested(&args.context_metadata_file)?,
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
    validate_repo(&args.repository)?;
    let notes = read_nonempty(&args.notes_file)?;
    let url = github_release_url(&args.api_base_url, &args.repository, &args.tag);
    let response = curl_json("GET", &url, Some(&args.github_token), None)?;
    if response.status == 404 {
        return Err(format!("release {} not found", args.tag).into());
    }
    if !(200..300).contains(&response.status) {
        return Err(format!("GitHub release fetch failed with HTTP {}", response.status).into());
    }
    let release: Value = serde_json::from_str(&response.body)?;
    let id = release["id"]
        .as_i64()
        .ok_or("release response missing id")?;
    let existing = release["body"].as_str().unwrap_or("");
    let body = compose_release_body(&notes, existing);
    let update_url = format!(
        "{}/repos/{}/releases/{}",
        args.api_base_url.trim_end_matches('/'),
        args.repository,
        id
    );
    let update = curl_json(
        "PATCH",
        &update_url,
        Some(&args.github_token),
        Some(&json!({ "body": body })),
    )?;
    if (200..300).contains(&update.status) {
        Ok(())
    } else {
        Err(format!("GitHub release update failed with HTTP {}", update.status).into())
    }
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
    let xml = render_feed(&args.repository, &items);
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

fn render_feed(repository: &str, items: &[FeedItem]) -> String {
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\">\n<channel>\n<title>{}</title>\n<link>https://github.com/{}</link>\n<description>Release notes for {}</description>\n<lastBuildDate>{}</lastBuildDate>\n",
        xml_escape(repository),
        xml_escape(repository),
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
        .arg("User-Agent: landfall")
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
    let issues = find_failure_issues(
        &args.github_token,
        &args.api_base_url,
        &args.repository,
        &args.release_tag,
    )?;
    for issue in issues {
        let number = issue["number"].as_i64().unwrap_or_default();
        let comment_url = format!(
            "{}/repos/{}/issues/{}/comments",
            args.api_base_url.trim_end_matches('/'),
            args.repository,
            number
        );
        let _ = curl_json(
            "POST",
            &comment_url,
            Some(&args.github_token),
            Some(
                &json!({"body": format!("Landfall synthesis recovered for {}.", args.release_tag)}),
            ),
        )?;
        let issue_url = format!(
            "{}/repos/{}/issues/{}",
            args.api_base_url.trim_end_matches('/'),
            args.repository,
            number
        );
        let _ = curl_json(
            "PATCH",
            &issue_url,
            Some(&args.github_token),
            Some(&json!({"state": "closed"})),
        )?;
    }
    Ok(())
}

fn report_synthesis_failure(args: ReportFailureArgs) -> Result<()> {
    validate_url(&args.workflow_run_url)?;
    if !find_failure_issues(
        &args.github_token,
        &args.api_base_url,
        &args.repository,
        &args.release_tag,
    )?
    .is_empty()
    {
        return Ok(());
    }
    let title = failure_issue_title(&args.release_tag);
    let body = format!(
        "Landfall could not synthesize user-facing release notes for `{}`.\n\n- Workflow: {}\n- Run: {}\n- Stage: {}\n- Message: {}\n",
        args.release_tag,
        args.workflow_name,
        args.workflow_run_url,
        args.failure_stage,
        args.failure_message
    );
    let url = format!(
        "{}/repos/{}/issues",
        args.api_base_url.trim_end_matches('/'),
        args.repository
    );
    let response = curl_json(
        "POST",
        &url,
        Some(&args.github_token),
        Some(&json!({"title": title, "body": body, "labels": ["landfall", "release-notes"]})),
    )?;
    if (200..300).contains(&response.status) {
        Ok(())
    } else {
        Err(format!("issue creation failed with HTTP {}", response.status).into())
    }
}

fn find_failure_issues(
    token: &str,
    base: &str,
    repository: &str,
    release_tag: &str,
) -> Result<Vec<Value>> {
    validate_repo(repository)?;
    let url = format!(
        "{}/repos/{}/issues?state=open&labels=landfall,release-notes&per_page=100",
        base.trim_end_matches('/'),
        repository
    );
    let response = curl_json("GET", &url, Some(token), None)?;
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

fn failure_issue_title(release_tag: &str) -> String {
    format!("Landfall release-note synthesis failed for {release_tag}")
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
    let cargo_path = args.repo_root.join("crates/landfall/Cargo.toml");
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
        cargo_version(&args.repo_root.join("crates/landfall/Cargo.toml")).unwrap_or_default();
    let mut drift = Vec::new();
    if package_version != latest {
        drift.push(format!(
            "package.json has {package_version}, expected {latest}"
        ));
    }
    if !cargo_version.is_empty() && cargo_version != latest {
        drift.push(format!(
            "crates/landfall/Cargo.toml has {cargo_version}, expected {latest}"
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
        errors.extend(validate_landfall_usage_inputs(&path, &text, &known));
    }
    errors.extend(validate_manifest_schema_contract(&readme));
    errors.extend(validate_manifest_action_precedence_contract(
        &fs::read_to_string(&action_path)?,
    ));
    errors.extend(validate_self_release_workflow_contract(&args.repo_root)?);
    if errors.is_empty() {
        println!("action contract ok");
        Ok(())
    } else {
        Err(errors.join("\n").into())
    }
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
        ".landfall.yml",
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
        "dist/landfall doctor --repo-root .",
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
        "Landfall LLM healthcheck skipped because model.policy=off disables synthesis.",
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

fn validate_landfall_usage_inputs(path: &Path, text: &str, known: &BTreeSet<&str>) -> Vec<String> {
    let mut errors = Vec::new();
    let key_re = Regex::new(r"^\s*([A-Za-z0-9_-]+):").unwrap();
    let mut in_landfall_step = false;
    let mut in_with = false;
    let mut with_indent = 0usize;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("uses:") || trimmed.starts_with("- uses:") {
            in_landfall_step = trimmed.contains("misty-step/landfall") || trimmed == "uses: ./";
            in_with = false;
            continue;
        }
        if !in_landfall_step {
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
            in_landfall_step = false;
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
        env::temp_dir().join(format!("landfall-replay-{}", std::process::id()))
    } else {
        PathBuf::from(&args.evidence_dir)
    };
    fs::create_dir_all(&evidence_dir)?;
    let tmp_root = env::temp_dir().join(format!("landfall-replay-fixtures-{}", std::process::id()));
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
        println!(
            "replay evidence: {}",
            evidence_dir.join("replay-result.json").display()
        );
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
        "manifest_defaults_and_overrides",
        "consumer_release_update_failure",
        "consumer_synthesis_only_success",
        "self_release_pr_path",
        "synthesis_cost_policy",
        "publication_degraded_optional",
        "publication_degraded_required",
        "summary_artifact_failed",
        "summary_release_update_failed",
        "summary_rss_failed",
    ]
}

fn scenario_action_static_contract(_: &Path) -> Result<Value> {
    let action = fs::read_to_string("action.yml")?;
    if action.contains("python ") || action.contains("setup-python") {
        return Err("action.yml still invokes Python".into());
    }
    if !action.contains("dist/landfall") {
        return Err("action.yml does not invoke dist/landfall".into());
    }
    Ok(json!({"checked": ["action.yml"]}))
}

fn scenario_action_manifest_defaults_precedence(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("action-manifest-defaults");
    fs::create_dir_all(&repo)?;
    fs::write(
        repo.join(".landfall.yml"),
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
    let output = temp_file("landfall-manifest-defaults")?;
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
    let output = temp_file("landfall-policy")?;
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
    let output = temp_file("landfall-policy")?;
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
    let output = temp_file("landfall-summary")?;
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
        repo.join(".landfall.yml"),
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
            ".landfall.yml",
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
        repo.join(".landfall.yml"),
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
        || dry_context["classification"]["categories"]
            .as_array()
            .unwrap()
            .iter()
            .all(|category| category != "docs-only")
    {
        return Err("dry-run cost policy did not skip docs-only release".into());
    }

    fs::write(
        repo.join(".landfall.yml"),
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
        || cheap_context["sources"]
            .as_array()
            .unwrap()
            .iter()
            .all(|source| source["name"] != "technical_changelog")
    {
        return Err("cheap policy did not use cheap model with context metadata".into());
    }

    fs::write(
        repo.join(".landfall.yml"),
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
    fallback_fake.llm_responses.push_back((500, String::new()));
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

    fs::write(
        repo.join(".landfall.yml"),
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
        || rich_context["classification"]["security"] != true
        || rich_context["classification"]["breaking"] != true
    {
        return Err("balanced policy did not escalate high-significance release".into());
    }

    Ok(json!({
        "dry_run_skip": dry_context["cost"],
        "cheap_model": cheap_request["model"],
        "fallback_attempts": attempts,
        "rich_cost": rich_context["cost"],
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
    let output = temp_file("landfall-policy")?;
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
    let prepare_output = temp_file("landfall-self-release-prepare")?;
    let dist_target = rustc_host_target()?;
    let prepare = Command::new(current_exe())
        .args([
            "prepare-self-release",
            "--repo-root",
            repo.to_str().unwrap(),
            "--repository",
            "owner/repo",
            "--release-branch",
            "landfall/self-release",
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
        &repo.join("crates/landfall/Cargo.toml"),
        r#"version = "1.1.0""#,
    )?;
    assert_file_contains(&repo.join("Cargo.lock"), r#"version = "1.1.0""#)?;
    assert_file_contains(&repo.join("CHANGELOG.md"), "# [1.1.0]")?;
    let prepared_dist = fs::read(repo.join("dist/landfall"))?;
    if prepared_dist == b"stale fixture binary\n" {
        return Err("prepare-self-release did not refresh dist/landfall".into());
    }
    assert_file_contains(&repo.join("dist/landfall.sha256"), "  dist/landfall")?;
    let changed_files = prepare_plan["changed_files"]
        .as_array()
        .ok_or("prepare plan missing changed_files")?;
    for expected in ["dist/landfall", "dist/landfall.sha256"] {
        if !changed_files
            .iter()
            .any(|file| file.as_str() == Some(expected))
        {
            return Err(format!("prepare plan missing {expected}").into());
        }
    }
    let dist_sha256 = fs::read_to_string(repo.join("dist/landfall.sha256"))?
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
    let publish_output = temp_file("landfall-self-release-publish")?;
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
    fs::create_dir_all(path.join("crates/landfall/src"))?;
    fs::create_dir_all(path.join("dist"))?;
    run_ok("git", ["init", "-q"], path)?;
    run_ok("git", ["config", "user.name", "Landfall Replay"], path)?;
    run_ok(
        "git",
        ["config", "user.email", "replay@example.invalid"],
        path,
    )?;
    fs::write(path.join("README.md"), "# Fixture\n")?;
    fs::write(
        path.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/landfall\"]\nresolver = \"3\"\n",
    )?;
    fs::write(
        path.join("package.json"),
        serde_json::to_string_pretty(&json!({"name": "landfall", "version": "1.0.0"}))? + "\n",
    )?;
    fs::write(
        path.join("crates/landfall/Cargo.toml"),
        "[package]\nname = \"landfall\"\nversion = \"1.0.0\"\nedition = \"2024\"\n",
    )?;
    fs::write(
        path.join("crates/landfall/src/main.rs"),
        "fn main() { println!(\"landfall fixture {}\", env!(\"CARGO_PKG_VERSION\")); }\n",
    )?;
    fs::write(
        path.join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\nversion = 4\n\n[[package]]\nname = \"landfall\"\nversion = \"1.0.0\"\n",
    )?;
    fs::write(path.join("dist/landfall"), "stale fixture binary\n")?;
    fs::write(
        path.join("dist/landfall.sha256"),
        "1c8d630e34f92c015d86aacd405409334e6bf29b853d7af0d1952517cf8bc6cb  dist/landfall\n",
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
    let scan = FleetScan {
        generated_at: "2026-06-13T00:00:00Z".into(),
        owners: vec!["phrazzld".into(), "misty-step".into()],
        warnings: Vec::new(),
        repositories: vec![
            fleet_fixture_repo(
                "phrazzld/semantic-app",
                "semantic-release",
                false,
                false,
                "unprotected-or-unavailable",
                &[],
                &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
            ),
            fleet_fixture_repo(
                "misty-step/release-please-app",
                "release-please",
                false,
                false,
                "protected",
                &[],
                &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
            ),
            fleet_fixture_repo(
                "misty-step/changesets-app",
                "changesets",
                false,
                false,
                "unprotected-or-unavailable",
                &["package.json", "Cargo.toml"],
                &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
            ),
            fleet_fixture_repo(
                "phrazzld/manual-app",
                "manual-tag",
                false,
                false,
                "unprotected-or-unavailable",
                &[],
                &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
            ),
            fleet_fixture_repo(
                "phrazzld/no-release-app",
                "no-release-tool",
                false,
                false,
                "unprotected-or-unavailable",
                &[],
                &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
            ),
            fleet_fixture_repo(
                "misty-step/archived-app",
                "semantic-release",
                true,
                false,
                "unprotected-or-unavailable",
                &[],
                &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
            ),
            fleet_fixture_repo(
                "misty-step/private-app",
                "release-please",
                false,
                true,
                "unavailable: no GitHub token supplied",
                &[],
                &[],
            ),
            fleet_fixture_repo(
                "phrazzld/protected-app",
                "manual-tag",
                false,
                false,
                "protected",
                &[],
                &["GH_RELEASE_TOKEN"],
            ),
            fleet_existing_landfall_fixture(),
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
    let plan: FleetPlan = serde_json::from_str(&fs::read_to_string(plan_dir.join("plan.json"))?)?;
    let pr_plan: FleetPrPlan =
        serde_json::from_str(&fs::read_to_string(pr_dir.join("open-prs.json"))?)?;
    let mut modes = BTreeMap::new();
    let mut statuses = BTreeMap::new();
    for repo in &plan.repositories {
        modes.insert(repo.repository.clone(), repo.recommended_mode.clone());
        statuses.insert(repo.repository.clone(), repo.status.clone());
    }
    for (repo, expected) in [
        ("phrazzld/semantic-app", "full"),
        ("misty-step/release-please-app", "synthesis-only"),
        ("misty-step/changesets-app", "synthesis-only"),
        ("phrazzld/manual-app", "synthesis-only"),
        ("phrazzld/no-release-app", "backfill-first"),
        ("misty-step/archived-app", "skipped"),
        ("misty-step/existing-landfall-app", "manifest-only"),
    ] {
        if modes.get(repo).map(String::as_str) != Some(expected) {
            return Err(format!("{repo} expected mode {expected}").into());
        }
    }
    if statuses.get("phrazzld/no-release-app").map(String::as_str) != Some("blocked") {
        return Err("no-release-tool repository should be blocked".into());
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
    let scan_text = fs::read_to_string(&scan_output)?;
    if scan_text.contains("super-secret") || scan_text.contains("ghp_") {
        return Err("fleet scan leaked a secret-looking value".into());
    }
    Ok(json!({
        "repositories": plan.repositories.len(),
        "dry_run_prs": pr_plan.repositories.iter().filter(|repo| !repo.skipped).count(),
        "modes": modes,
        "statuses": statuses,
        "evidence": {
            "scan": scan_output,
            "plan": plan_dir.join("plan.json"),
            "dry_run": pr_dir.join("open-prs.json"),
        }
    }))
}

fn fleet_fixture_repo(
    name_with_owner: &str,
    release_tool: &str,
    archived: bool,
    private: bool,
    branch_protected: &str,
    extra_packages: &[&str],
    present_secrets: &[&str],
) -> FleetRepository {
    let (owner, name) = name_with_owner.split_once('/').unwrap();
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
        "release-please" => vec!["release-please.yml".into()],
        "changesets" => vec!["changesets.yml".into()],
        "manual-tag" => vec!["release.yml".into()],
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
        existing_landfall: false,
        required_secrets,
        signals: vec![format!("{release_tool} fixture")],
    }
}

fn fleet_existing_landfall_fixture() -> FleetRepository {
    let mut repo = fleet_fixture_repo(
        "misty-step/existing-landfall-app",
        "manual-tag",
        false,
        false,
        "unprotected-or-unavailable",
        &[],
        &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
    );
    repo.existing_landfall = true;
    repo.release_files.push(".landfall.yml".into());
    repo.workflows.push("landfall-release.yml".into());
    repo.signals.push(".landfall.yml present".into());
    repo
}

fn init_fixture_repo(path: &Path, release_tag: &str) -> Result<()> {
    fs::create_dir_all(path)?;
    run_ok("git", ["init", "-q"], path)?;
    run_ok("git", ["config", "user.name", "Landfall Replay"], path)?;
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
            attempts_file: attempts,
            context_metadata_file: PathBuf::from("."),
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
        assert!(workflow.contains("healthcheck: \"true\""));
        assert!(workflow.contains("pull-requests: write"));
        assert!(workflow.contains("NPM_TOKEN"));
    }

    #[test]
    fn setup_detects_semantic_release_and_reports_backfill_retired() {
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
        assert!(workflow.contains("healthcheck: \"true\""));
        assert!(workflow.contains("GH_RELEASE_TOKEN"));
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
            "# Atlas\n\nLandfall-managed release automation.\n",
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
            packages: vec!["landfall".into()],
            signals: Vec::new(),
        };
        let manifest = LandfallManifest {
            product: ProductManifest {
                name: Some("Landfall".into()),
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
            repo.path().join(".landfall.yml"),
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
        let mut manifest: LandfallManifest =
            serde_yaml::from_str(&fs::read_to_string(repo.path().join(".landfall.yml")).unwrap())
                .unwrap();
        manifest.model.primary = None;
        manifest.model.policy = Some("rich".into());
        fs::write(
            repo.path().join(".landfall.yml"),
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
        let manifest = LandfallManifest {
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
        let manifest = LandfallManifest {
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
        assert!(manual.contains("${{ github.event.release.tag_name || github.ref_name }}"));
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
    fn cargo_lock_version_update_targets_landfall_package_only() {
        let repo = tempfile::tempdir().unwrap();
        let path = repo.path().join("Cargo.lock");
        fs::write(
            &path,
            "[[package]]\nname = \"dep\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"landfall\"\nversion = \"1.2.3\"\n",
        )
        .unwrap();
        update_lock_package_version(&path, "landfall", "1.3.0").unwrap();
        let text = fs::read_to_string(path).unwrap();
        assert!(text.contains("name = \"dep\"\nversion = \"0.1.0\""));
        assert!(text.contains("name = \"landfall\"\nversion = \"1.3.0\""));
    }

    #[test]
    fn version_sync_allows_explicit_release_candidate() {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join("crates/landfall")).unwrap();
        run_ok("git", ["init", "-q"], repo.path()).unwrap();
        run_ok("git", ["config", "user.name", "Landfall Test"], repo.path()).unwrap();
        run_ok(
            "git",
            ["config", "user.email", "landfall@example.invalid"],
            repo.path(),
        )
        .unwrap();
        fs::write(
            repo.path().join("package.json"),
            r#"{"name":"landfall","version":"1.18.0"}"#,
        )
        .unwrap();
        fs::write(
            repo.path().join("crates/landfall/Cargo.toml"),
            "[package]\nname = \"landfall\"\nversion = \"1.18.0\"\nedition = \"2024\"\n",
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
            repo.path().join("crates/landfall/Cargo.toml"),
            "[package]\nname = \"landfall\"\nversion = \"1.17.9\"\nedition = \"2024\"\n",
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
