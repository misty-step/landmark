use chrono::Utc;
use clap::{Args, Parser, Subcommand};
use hmac::{Hmac, Mac};
use pulldown_cmark::{Options, Parser as MarkdownParser, html};
use regex::Regex;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::Sha256;
use std::collections::{BTreeMap, BTreeSet};
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

#[derive(Parser)]
#[command(name = "landfall")]
#[command(about = "Rust runtime for the Landfall release action")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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
    #[arg(long)]
    model: String,
    #[arg(long = "api-url")]
    api_url: String,
    #[arg(long = "fallback-models", default_value = "")]
    fallback_models: String,
    #[arg(long = "product-name")]
    product_name: String,
    #[arg(long = "product-description", default_value = "")]
    product_description: String,
    #[arg(long = "voice-guide", default_value = "")]
    voice_guide: String,
    #[arg(long, default_value = "general")]
    audience: String,
    #[arg(long = "changelog-source", default_value = "auto")]
    changelog_source: String,
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

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    match Cli::parse().command {
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

#[derive(Serialize)]
struct SetupReport {
    diagnosis: SetupDiagnosis,
    recommendation: SetupRecommendation,
    required_permissions: BTreeMap<String, String>,
    required_secrets: Vec<String>,
    workflows: BTreeMap<String, WorkflowCandidate>,
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

fn setup(args: SetupArgs) -> Result<()> {
    let diagnosis = diagnose_setup(&args.repo_root);
    let recommendation = recommend_setup(&diagnosis);
    let workflows = setup_workflows(&diagnosis);
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
        backfill: "retired: use release re-run or synthesis-only mode; no Python backfill script is part of the maintenance surface".into(),
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
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

fn recommend_setup(diagnosis: &SetupDiagnosis) -> SetupRecommendation {
    let workflow = match diagnosis.release_tool.as_str() {
        "semantic-release" => "semantic-release",
        "release-please" => "release-please",
        "changesets" if diagnosis.monorepo => "changesets-monorepo",
        "changesets" => "changesets",
        _ => "manual-tag",
    };
    let mode = if workflow == "semantic-release" {
        "full"
    } else {
        "synthesis-only"
    };
    let mut rationale = vec![format!("detected release tool: {}", diagnosis.release_tool)];
    rationale.push(format!("default branch: {}", diagnosis.default_branch));
    rationale.push(format!("tag format: {}", diagnosis.tag_format));
    if diagnosis.monorepo {
        rationale.push("monorepo outputs enabled".into());
    }
    SetupRecommendation {
        mode: mode.into(),
        workflow: workflow.into(),
        rationale,
    }
}

fn setup_workflows(diagnosis: &SetupDiagnosis) -> BTreeMap<String, WorkflowCandidate> {
    let branch = &diagnosis.default_branch;
    let mut workflows = BTreeMap::new();
    for (name, tool, mode, content) in [
        (
            "semantic-release",
            "semantic-release",
            "full",
            workflow_semantic_release(branch),
        ),
        (
            "release-please",
            "release-please",
            "synthesis-only",
            workflow_release_please(branch),
        ),
        (
            "changesets",
            "changesets",
            "synthesis-only",
            workflow_changesets(branch, false),
        ),
        (
            "changesets-monorepo",
            "changesets",
            "synthesis-only",
            workflow_changesets(branch, true),
        ),
        (
            "manual-tag",
            "manual-tag",
            "synthesis-only",
            workflow_manual_tag(),
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

fn workflow_semantic_release(branch: &str) -> String {
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
"#
    )
}

fn workflow_release_please(branch: &str) -> String {
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
          changelog-source: release-body
"#
    )
}

fn workflow_changesets(branch: &str, monorepo: bool) -> String {
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
          changelog-source: release-body
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
          changelog-source: release-body
"#
        )
    }
}

fn workflow_manual_tag() -> String {
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
          release-tag: ${{ github.event.release.tag_name || github.ref_name }}
          github-token: ${{ secrets.GH_RELEASE_TOKEN }}
          llm-api-key: ${{ secrets.OPENROUTER_API_KEY }}
          changelog-source: auto
"#
    .to_string()
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
    validate_nonblank(&args.api_key, "api-key")?;
    validate_nonblank(&args.model, "model")?;
    let technical = resolve_technical_changelog(&args)?;
    let prompt = render_prompt(&args, &technical)?;
    let mut models = vec![args.model.clone()];
    models.extend(
        args.fallback_models
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
                }));
            }
            Err(error) => {
                last_error = format!("model {model} failed: {error}");
                attempts.push(json!({
                    "model": model,
                    "succeeded": false,
                    "quality": "failed",
                    "message": last_error,
                }));
            }
        }
    }
    write_json_if_requested(&args.attempts_file, &attempts)?;
    Err(last_error.into())
}

fn resolve_technical_changelog(args: &SynthesizeArgs) -> Result<String> {
    let source = args.changelog_source.to_ascii_lowercase();
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

fn render_prompt(args: &SynthesizeArgs, technical: &str) -> Result<String> {
    let template = if args.prompt_template.is_file() {
        fs::read_to_string(&args.prompt_template)?
    } else {
        let filename = match args.audience.as_str() {
            "developer" | "end-user" | "enterprise" | "general" => format!("{}.md", args.audience),
            _ => return Err(format!("invalid audience {}", args.audience).into()),
        };
        let path = args.templates_dir.join(filename);
        if path.is_file() {
            fs::read_to_string(path)?
        } else {
            fs::read_to_string("templates/synthesis-prompt.md")?
        }
    };
    let product_context = if args.product_description.trim().is_empty() {
        String::new()
    } else {
        format!("Product context: {}\n", args.product_description.trim())
    };
    let voice_guide = if args.voice_guide.trim().is_empty() {
        String::new()
    } else {
        format!("Voice guide: {}\n", args.voice_guide.trim())
    };
    Ok(template
        .replace("{{PRODUCT_NAME}}", &args.product_name)
        .replace("{{VERSION}}", &args.version)
        .replace("{{TECHNICAL_CHANGELOG}}", technical)
        .replace("{{PRODUCT_CONTEXT}}", &product_context)
        .replace("{{VOICE_GUIDE}}", &voice_guide)
        .replace("{{BULLET_TARGET}}", "4")
        .replace("{{BREAKING_CHANGES}}", &render_breaking_changes(technical)))
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
    if succeeded && quality == "degraded" && required {
        can_update_release = false;
        can_publish_artifacts = false;
        failure_stage = "validation".to_string();
        failure_message = "Synthesis quality is degraded and synthesis is required.".to_string();
        exit_failure = true;
    } else if !succeeded && required {
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
            ("succeeded", succeeded.to_string()),
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
    let (succeeded, failure_stage, failure_message) = if !synthesis_enabled || !released {
        (true, "", "")
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
            enabled: synthesis_enabled && released && synth_succeeded,
            succeeded: update_succeeded,
            failure_stage: sanitize_text(&args.update_failure_stage),
            failure_message: sanitize_text(&args.update_failure_message),
        },
    );
    destinations.insert(
        "artifacts".to_string(),
        DestinationStatus {
            enabled: synthesis_enabled && released && synth_succeeded && update_succeeded,
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
    if errors.is_empty() {
        println!("action contract ok");
        Ok(())
    } else {
        Err(errors.join("\n").into())
    }
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
        "consumer_degraded_required_fails",
        "consumer_floating_tag_behavior",
        "consumer_full_mode_success",
        "consumer_release_update_failure",
        "consumer_synthesis_only_success",
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
                    if state.llm_status >= 400 {
                        json_response(
                            state.llm_status,
                            json!({"error": {"message": "fake LLM failure"}}),
                        )
                    } else {
                        json_response(
                            200,
                            json!({"choices": [{"message": {"content": state.llm_notes}}]}),
                        )
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
        let recommendation = recommend_setup(&diagnosis);
        assert_eq!(recommendation.workflow, "changesets-monorepo");
        let workflows = setup_workflows(&diagnosis);
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
        assert_eq!(recommend_setup(&diagnosis).mode, "full");
        let workflow = &setup_workflows(&diagnosis)["semantic-release"].content;
        assert!(workflow.contains("mode: full"));
        assert!(workflow.contains("healthcheck: \"true\""));
        assert!(workflow.contains("GH_RELEASE_TOKEN"));
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
        for candidate in setup_workflows(&diagnosis).values() {
            let parsed: serde_yaml::Value = serde_yaml::from_str(&candidate.content).unwrap();
            assert!(parsed["jobs"].is_mapping(), "{}", candidate.path);
        }
    }

    #[test]
    fn floating_tag_skips_prerelease() {
        assert_eq!(parse_major_tag("v1.2.3").as_deref(), Some("v1"));
        assert_eq!(parse_major_tag("v1.2.3-beta.1"), None);
    }
}
