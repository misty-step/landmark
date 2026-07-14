use crate::*;

#[derive(Clone, Serialize)]
pub(crate) struct SchemaDescriptor {
    pub(crate) name: &'static str,
    pub(crate) path: &'static str,
    pub(crate) id: &'static str,
    pub(crate) version: &'static str,
    pub(crate) artifact: &'static str,
}

#[derive(Clone, Serialize)]
pub(crate) struct CommandContract {
    pub(crate) command: &'static str,
    pub(crate) mode: &'static str,
    pub(crate) mutates: bool,
    pub(crate) preview: &'static str,
    pub(crate) stdout: &'static str,
    pub(crate) stderr: &'static str,
    pub(crate) json_output: bool,
}

pub(crate) fn describe(args: DescribeArgs) -> Result<()> {
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
        "product_boundary": "Rust CLI release-intelligence runtime; GitHub Action and rich artifact producers are adapter wrappers",
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

pub(crate) fn describe_clap_command(command: &clap::Command) -> Value {
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

pub(crate) fn schema_descriptors() -> Vec<SchemaDescriptor> {
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
            name: "release_kit",
            path: "schemas/release-kit.v1.schema.json",
            id: "https://landmark.dev/schemas/release-kit.v1.schema.json",
            version: "v1",
            artifact: "release kit plan",
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
        SchemaDescriptor {
            name: "release_transaction",
            path: "schemas/release-transaction.v1.schema.json",
            id: "https://landmark.dev/schemas/release-transaction.v1.schema.json",
            version: "v1",
            artifact: "prepared or artifact-bound release transaction",
        },
        SchemaDescriptor {
            name: "release_artifact_manifest",
            path: "schemas/release-artifact-manifest.v1.schema.json",
            id: "https://landmark.dev/schemas/release-artifact-manifest.v1.schema.json",
            version: "v1",
            artifact: "product-supplied immutable release artifact manifest",
        },
        SchemaDescriptor {
            name: "release_publication_manifest",
            path: "schemas/release-publication-manifest.v1.schema.json",
            id: "https://landmark.dev/schemas/release-publication-manifest.v1.schema.json",
            version: "v1",
            artifact: "signed release candidate and OCI digest binding",
        },
    ]
}

pub(crate) fn command_contracts() -> Vec<CommandContract> {
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
            command: "notify-release-feed",
            mode: "producer-adapter",
            mutates: true,
            preview: "cleanly skips when receiver URL or secret config is absent; otherwise sends one signed release-kit event",
            stdout: "ReleaseFeedReceipt JSON",
            stderr: "logs and errors only",
            json_output: true,
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
            command: "release-transaction prepare|bind",
            mode: "portable-release-transaction",
            mutates: true,
            preview: "prepare and bind mutate one canonical local transaction file but never a remote provider",
            stdout: "ReleaseTransaction JSON",
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

pub(crate) fn failure_taxonomy() -> Vec<Value> {
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
