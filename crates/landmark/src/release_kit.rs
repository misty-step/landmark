use super::{
    ContextCommit, DeterministicReleaseContext, LandmarkManifest, Result, RunArgs,
    RunArtifactRecord, RunPublicationRecord, RunReleaseContext, RunVersionDecision,
    classify_release_context_with_deterministic, context_source, conventional_commit_type,
    is_breaking_commit, sha256_hex, trimmed_option,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::PathBuf;

const SCHEMA_VERSION: &str = "landmark.release-kit.v1";
const PRODUCER_EVIDENCE_DIR: &str = ".landmark/run/producers";

#[derive(Clone, Debug, Serialize)]
pub(super) struct ReleaseKit {
    schema_version: String,
    generated_at: String,
    product: ReleaseKitProduct,
    release: ReleaseKitRelease,
    classification: ReleaseKitClassification,
    artifacts: Vec<ReleaseKitArtifact>,
    producer_contracts: Vec<ReleaseKitProducerContract>,
    provenance: Vec<ReleaseKitProvenance>,
    approvals: Vec<ReleaseKitApproval>,
    status: ReleaseKitStatus,
}

#[derive(Clone, Debug, Serialize)]
struct ReleaseKitProduct {
    name: String,
    repository: String,
    audience: String,
    description: String,
}

#[derive(Clone, Debug, Serialize)]
struct ReleaseKitRelease {
    tag: String,
    version: String,
    previous_tag: String,
    repository: String,
    release_url: String,
    version_decision: RunVersionDecision,
}

#[derive(Clone, Debug, Serialize)]
struct ReleaseKitClassification {
    importance: String,
    audiences: Vec<String>,
    why_it_matters: String,
}

#[derive(Clone, Debug, Serialize)]
struct ReleaseKitArtifact {
    id: String,
    kind: String,
    audience: String,
    owner: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
    acceptance: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    depends_on: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocker: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    waiver: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct ReleaseKitProducerContract {
    id: String,
    producer: String,
    adapter_kind: String,
    input_artifacts: Vec<String>,
    output_artifacts: Vec<String>,
    command: String,
    mutates: bool,
    acceptance: Vec<String>,
    evidence_path: String,
}

#[derive(Clone, Debug, Serialize)]
struct ReleaseKitProvenance {
    artifact_id: String,
    sources: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct ReleaseKitApproval {
    artifact_id: String,
    state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    approver: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct ReleaseKitStatus {
    complete: bool,
    blocked: bool,
    summary: String,
}

pub(super) struct PlanInput<'a> {
    pub(super) args: &'a RunArgs,
    pub(super) manifest: &'a LandmarkManifest,
    pub(super) repository: &'a str,
    pub(super) release: &'a RunReleaseContext,
    pub(super) artifacts: &'a RunArtifactRecord,
    pub(super) publication: &'a RunPublicationRecord,
    pub(super) technical_changelog: &'a str,
    pub(super) notes: &'a str,
    pub(super) generated_at: &'a str,
}

struct ReleaseKitArtifactSpec<'a> {
    id: &'a str,
    kind: &'a str,
    audience: &'a str,
    owner: &'a str,
    status: &'a str,
    path: Option<String>,
    sha256: Option<String>,
    acceptance: &'a [&'a str],
    depends_on: &'a [&'a str],
}

pub(super) fn schema_version() -> &'static str {
    SCHEMA_VERSION
}

pub(super) fn artifact_path(args: &RunArgs) -> PathBuf {
    if args.output_dir.as_os_str().is_empty() {
        PathBuf::new()
    } else {
        args.repo_root
            .join(&args.output_dir)
            .join("release-kit.json")
    }
}

pub(super) fn plan(input: PlanInput<'_>) -> ReleaseKit {
    let PlanInput {
        args,
        manifest,
        repository,
        release,
        artifacts,
        publication,
        technical_changelog,
        notes,
        generated_at,
    } = input;
    let mut classification_text = technical_changelog.to_string();
    for commit in &release.commits {
        classification_text.push('\n');
        classification_text.push_str(&commit.subject);
        classification_text.push('\n');
        classification_text.push_str(&commit.body);
    }
    let context_sources = vec![
        context_source("technical_changelog", "git_range", technical_changelog),
        context_source("public_notes", "generated", notes),
    ];
    let deterministic = DeterministicReleaseContext {
        commits: release
            .commits
            .iter()
            .map(|commit| ContextCommit {
                conventional_type: conventional_commit_type(&commit.subject)
                    .unwrap_or("")
                    .to_string(),
                breaking: is_breaking_commit(commit),
                subject: commit.subject.clone(),
                body: commit.body.clone(),
                short_hash: commit.short_hash.clone(),
            })
            .collect(),
        ..DeterministicReleaseContext::default()
    };
    let release_classification = classify_release_context_with_deterministic(
        &classification_text,
        &context_sources,
        &deterministic,
    );
    let importance = release_kit_importance(&release_classification, &release.decision);
    let primary_audience = manifest
        .audience
        .as_deref()
        .and_then(trimmed_option)
        .unwrap_or_else(|| "general".into());
    let audiences = release_kit_audiences(&primary_audience, &importance);
    let product_name = manifest
        .product
        .name
        .as_deref()
        .and_then(trimmed_option)
        .unwrap_or_else(|| repository.to_string());
    let product_description = manifest
        .product
        .description
        .as_deref()
        .and_then(trimmed_option)
        .unwrap_or_default();
    let technical_status = artifact_status(args.dry_run, &artifacts.technical_changelog);
    let notes_status = artifact_status(args.dry_run, &artifacts.markdown);
    let feed_status = artifact_status(args.dry_run, &artifacts.rss);
    let technical_sha = sha256_hex(technical_changelog.as_bytes());
    let notes_sha = sha256_hex(notes.as_bytes());

    let mut kit_artifacts = vec![
        release_kit_artifact(ReleaseKitArtifactSpec {
            id: "technical-changelog",
            kind: "technical_changelog",
            audience: "developer-operator",
            owner: "landmark",
            status: &technical_status,
            path: trimmed_option(&artifacts.technical_changelog),
            sha256: Some(technical_sha.clone()),
            acceptance: &[
                "Preserves the raw commit subjects and hashes for operator review.",
                "Matches landmark.internal-technical-changelog.v1.",
            ],
            depends_on: &[],
        }),
        release_kit_artifact(ReleaseKitArtifactSpec {
            id: "release-notes",
            kind: "release_notes",
            audience: &primary_audience,
            owner: "landmark",
            status: &notes_status,
            path: trimmed_option(&artifacts.markdown),
            sha256: Some(notes_sha.clone()),
            acceptance: &[
                "Summarizes the release for the configured audience.",
                "Can be published without exposing internal-only commit detail.",
            ],
            depends_on: &["technical-changelog"],
        }),
    ];
    if !artifacts.rss.trim().is_empty() {
        kit_artifacts.push(release_kit_artifact(ReleaseKitArtifactSpec {
            id: "release-feed",
            kind: "feed",
            audience: "subscriber",
            owner: "landmark",
            status: &feed_status,
            path: trimmed_option(&artifacts.rss),
            sha256: None,
            acceptance: &[
                "Links to the release tag URL.",
                "Retains only the configured maximum number of feed entries.",
            ],
            depends_on: &["release-notes"],
        }));
    }

    let rich_artifacts = release_kit_needs_rich_artifacts(&importance);
    if rich_artifacts {
        kit_artifacts.extend(rich_release_artifacts(
            &primary_audience,
            &release.release_tag,
        ));
    }

    let producer_contracts = if rich_artifacts {
        producer_contracts(args, artifacts, &release.release_tag)
    } else {
        Vec::new()
    };
    let provenance = kit_artifacts
        .iter()
        .map(|artifact| {
            let mut sources = vec![
                format!("git:{}", release.decision.range),
                format!("technical_changelog_sha256:{technical_sha}"),
            ];
            if artifact.id != "technical-changelog" {
                sources.push(format!("notes_sha256:{notes_sha}"));
            }
            if artifact.owner == "producer-adapter" {
                sources.push(format!("release_kit:{}", artifacts.release_kit));
            }
            ReleaseKitProvenance {
                artifact_id: artifact.id.clone(),
                sources,
                notes: Some(format!("planned from {} release facts", importance)),
            }
        })
        .collect::<Vec<_>>();
    let approvals = kit_artifacts
        .iter()
        .map(|artifact| {
            if artifact.owner == "producer-adapter" {
                ReleaseKitApproval {
                    artifact_id: artifact.id.clone(),
                    state: "pending".into(),
                    approver: None,
                    reason: Some("producer output must be reviewed before publication".into()),
                }
            } else {
                ReleaseKitApproval {
                    artifact_id: artifact.id.clone(),
                    state: "not-required".into(),
                    approver: Some("landmark".into()),
                    reason: Some("Landmark-owned artifact generated from release facts".into()),
                }
            }
        })
        .collect::<Vec<_>>();
    let pending_approvals = approvals
        .iter()
        .filter(|approval| approval.state == "pending")
        .count();
    let complete = !args.dry_run
        && pending_approvals == 0
        && kit_artifacts
            .iter()
            .all(|artifact| matches!(artifact.status.as_str(), "produced" | "verified" | "waived"));
    let status_summary = if args.dry_run {
        "dry-run release kit printed in evidence; no artifacts were written".to_string()
    } else if pending_approvals > 0 {
        format!(
            "release kit generated with {pending_approvals} producer-owned artifact approvals pending"
        )
    } else if complete {
        "release kit complete for Landmark-owned outputs".into()
    } else {
        "release kit generated with planned Landmark outputs still pending".into()
    };

    ReleaseKit {
        schema_version: SCHEMA_VERSION.into(),
        generated_at: generated_at.to_string(),
        product: ReleaseKitProduct {
            name: product_name,
            repository: repository.to_string(),
            audience: primary_audience.clone(),
            description: product_description,
        },
        release: ReleaseKitRelease {
            tag: release.release_tag.clone(),
            version: release.version.clone(),
            previous_tag: release.previous_tag.clone(),
            repository: repository.to_string(),
            release_url: publication.release_url.clone(),
            version_decision: release.decision.clone(),
        },
        classification: ReleaseKitClassification {
            importance: importance.clone(),
            audiences,
            why_it_matters: release_kit_importance_reason(&importance, &release_classification),
        },
        artifacts: kit_artifacts,
        producer_contracts,
        provenance,
        approvals,
        status: ReleaseKitStatus {
            complete,
            blocked: false,
            summary: status_summary,
        },
    }
}

fn rich_release_artifacts(primary_audience: &str, release_tag: &str) -> Vec<ReleaseKitArtifact> {
    let docs_dir = "docs/releases";
    vec![
        release_kit_artifact(ReleaseKitArtifactSpec {
            id: "migration-guide",
            kind: "migration_guide",
            audience: "developer",
            owner: "producer-adapter",
            status: "planned",
            path: Some(format!("{docs_dir}/{release_tag}-migration.md")),
            sha256: None,
            acceptance: &[
                "Names required user or operator migration steps.",
                "Links back to the release facts and technical changelog.",
            ],
            depends_on: &["technical-changelog", "release-notes"],
        }),
        release_kit_artifact(ReleaseKitArtifactSpec {
            id: "docs-update",
            kind: "docs_update",
            audience: "developer",
            owner: "producer-adapter",
            status: "planned",
            path: Some(format!("{docs_dir}/{release_tag}-docs.patch")),
            sha256: None,
            acceptance: &[
                "Updates user-facing setup or upgrade documentation.",
                "Leaves a reviewable patch and evidence receipt.",
            ],
            depends_on: &["migration-guide"],
        }),
        release_kit_artifact(ReleaseKitArtifactSpec {
            id: "blog-draft",
            kind: "blog_post",
            audience: primary_audience,
            owner: "producer-adapter",
            status: "planned",
            path: Some(format!("{docs_dir}/{release_tag}-blog.md")),
            sha256: None,
            acceptance: &[
                "Explains why the release matters to the target audience.",
                "Keeps claims grounded in release provenance.",
            ],
            depends_on: &["release-notes"],
        }),
        release_kit_artifact(ReleaseKitArtifactSpec {
            id: "demo-video",
            kind: "video",
            audience: primary_audience,
            owner: "producer-adapter",
            status: "planned",
            path: Some(format!("{docs_dir}/{release_tag}-demo.mp4")),
            sha256: None,
            acceptance: &[
                "Demonstrates the changed workflow from the release kit brief.",
                "Returns a path, hash, and review evidence before publication.",
            ],
            depends_on: &["demo-script"],
        }),
        release_kit_artifact(ReleaseKitArtifactSpec {
            id: "demo-script",
            kind: "demo_script",
            audience: "developer-operator",
            owner: "landmark",
            status: "planned",
            path: Some(format!("{docs_dir}/{release_tag}-demo-script.md")),
            sha256: None,
            acceptance: &[
                "Scopes the demo to release facts and user-visible workflow changes.",
                "Can be handed to a video or browser-capture producer.",
            ],
            depends_on: &["release-notes"],
        }),
    ]
}

fn artifact_status(dry_run: bool, path: &str) -> String {
    if dry_run || path.trim().is_empty() {
        "planned".into()
    } else {
        "produced".into()
    }
}

fn release_kit_importance(
    classification: &super::ReleaseClassification,
    decision: &RunVersionDecision,
) -> String {
    if classification.security {
        "security".into()
    } else if decision.bump == "major" || classification.breaking || classification.migration_heavy
    {
        "migration".into()
    } else if classification.significance == "high" {
        "high".into()
    } else if decision.latest_tag.is_empty() && decision.bump != "none" {
        "launch".into()
    } else if classification.significance == "low" {
        "low".into()
    } else {
        "medium".into()
    }
}

fn release_kit_audiences(primary: &str, importance: &str) -> Vec<String> {
    let mut audiences = BTreeSet::new();
    audiences.insert(primary.to_string());
    audiences.insert("developer-operator".into());
    if release_kit_needs_rich_artifacts(importance) {
        audiences.insert("release-operator".into());
        audiences.insert("docs-owner".into());
    }
    audiences.into_iter().collect()
}

fn release_kit_needs_rich_artifacts(importance: &str) -> bool {
    matches!(importance, "high" | "launch" | "migration" | "security")
}

fn release_kit_importance_reason(
    importance: &str,
    classification: &super::ReleaseClassification,
) -> String {
    match importance {
        "security" => "security-sensitive release needs explicit review and downstream handoffs",
        "migration" => {
            "breaking or migration-heavy release needs upgrade guidance and launch assets"
        }
        "launch" => "first managed release needs adoption-facing launch artifacts",
        "high" => "high-importance release needs richer final-mile artifact planning",
        "low" => "internal or low-significance release should keep the final-mile kit small",
        _ => "user-visible release needs Landmark-owned notes and feed evidence",
    }
    .to_string()
        + if classification.reasons.is_empty() {
            ""
        } else {
            "; signals: "
        }
        + &classification.reasons.join("; ")
}

fn release_kit_artifact(spec: ReleaseKitArtifactSpec<'_>) -> ReleaseKitArtifact {
    ReleaseKitArtifact {
        id: spec.id.into(),
        kind: spec.kind.into(),
        audience: spec.audience.into(),
        owner: spec.owner.into(),
        status: spec.status.into(),
        path: spec.path,
        sha256: spec.sha256,
        acceptance: spec.acceptance.iter().map(|item| (*item).into()).collect(),
        depends_on: spec.depends_on.iter().map(|item| (*item).into()).collect(),
        blocker: None,
        waiver: None,
    }
}

#[derive(Clone, Copy)]
struct ProducerSpec {
    id: &'static str,
    producer: &'static str,
    adapter_kind: &'static str,
    input_artifacts: &'static [&'static str],
    output_artifacts: &'static [&'static str],
    command: ProducerCommand,
    acceptance: &'static [&'static str],
    evidence_file: &'static str,
}

#[derive(Clone, Copy)]
enum ProducerCommand {
    LandmarkDocs,
    HarnessReleaseCopy,
    HumanDemoVideo,
}

const PRODUCER_SPECS: &[ProducerSpec] = &[
    ProducerSpec {
        id: "docs-producer",
        producer: "release docs producer",
        adapter_kind: "local-cli",
        input_artifacts: &["technical-changelog", "release-notes"],
        output_artifacts: &["migration-guide", "docs-update"],
        command: ProducerCommand::LandmarkDocs,
        acceptance: &[
            "Returns a reviewable docs patch or migration guide path.",
            "Writes an evidence receipt before any repository mutation.",
        ],
        evidence_file: "docs-producer.json",
    },
    ProducerSpec {
        id: "launch-copy-producer",
        producer: "launch copy skill",
        adapter_kind: "harness-skill",
        input_artifacts: &["release-notes", "migration-guide"],
        output_artifacts: &["blog-draft"],
        command: ProducerCommand::HarnessReleaseCopy,
        acceptance: &[
            "Draft copy cites release-kit provenance instead of inventing claims.",
            "Draft is marked pending until human or producer review approves it.",
        ],
        evidence_file: "launch-copy-producer.json",
    },
    ProducerSpec {
        id: "demo-video-producer",
        producer: "demo video handoff",
        adapter_kind: "human",
        input_artifacts: &["demo-script", "release-notes"],
        output_artifacts: &["demo-video"],
        command: ProducerCommand::HumanDemoVideo,
        acceptance: &[
            "Returned video has a path, hash, and review evidence.",
            "Demo follows the release-kit acceptance checks.",
        ],
        evidence_file: "demo-video-producer.json",
    },
];

fn producer_contracts(
    args: &RunArgs,
    artifacts: &RunArtifactRecord,
    release_tag: &str,
) -> Vec<ReleaseKitProducerContract> {
    let evidence_dir = producer_evidence_dir(args);
    PRODUCER_SPECS
        .iter()
        .map(|spec| spec.contract(&artifacts.release_kit, release_tag, &evidence_dir))
        .collect()
}

fn producer_evidence_dir(args: &RunArgs) -> PathBuf {
    if args.output_dir.as_os_str().is_empty() {
        args.repo_root.join(PRODUCER_EVIDENCE_DIR)
    } else {
        args.repo_root.join(&args.output_dir).join("producers")
    }
}

impl ProducerSpec {
    fn contract(
        &self,
        release_kit_path: &str,
        release_tag: &str,
        evidence_dir: &std::path::Path,
    ) -> ReleaseKitProducerContract {
        ReleaseKitProducerContract {
            id: self.id.into(),
            producer: self.producer.into(),
            adapter_kind: self.adapter_kind.into(),
            input_artifacts: self
                .input_artifacts
                .iter()
                .map(|artifact| (*artifact).into())
                .collect(),
            output_artifacts: self
                .output_artifacts
                .iter()
                .map(|artifact| (*artifact).into())
                .collect(),
            command: self.command.render(release_kit_path, release_tag),
            mutates: false,
            acceptance: self
                .acceptance
                .iter()
                .map(|check| (*check).into())
                .collect(),
            evidence_path: evidence_dir.join(self.evidence_file).display().to_string(),
        }
    }
}

impl ProducerCommand {
    fn render(self, release_kit_path: &str, release_tag: &str) -> String {
        match self {
            ProducerCommand::LandmarkDocs => {
                format!(
                    "landmark-producer docs --release-kit {release_kit_path} --release-tag {release_tag}"
                )
            }
            ProducerCommand::HarnessReleaseCopy => {
                format!(
                    "harness-skill://release-copy?release-kit={release_kit_path}&release-tag={release_tag}"
                )
            }
            ProducerCommand::HumanDemoVideo => {
                format!(
                    "human handoff: produce demo video from {release_kit_path} for {release_tag}"
                )
            }
        }
    }
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

pub(super) fn assert_contract(value: &Value, label: &str) -> Result<()> {
    assert_schema_contract(value, label)?;
    assert_json_eq(value, "/schema_version", SCHEMA_VERSION, label)?;
    for pointer in [
        "/generated_at",
        "/product/name",
        "/release/tag",
        "/classification/importance",
        "/classification/why_it_matters",
        "/status/summary",
    ] {
        if value
            .pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            return Err(format!("{label} missing required string at {pointer}").into());
        }
    }
    for pointer in [
        "/classification/audiences",
        "/artifacts",
        "/provenance",
        "/approvals",
    ] {
        if value
            .pointer(pointer)
            .and_then(Value::as_array)
            .is_none_or(|items| items.is_empty())
        {
            return Err(format!("{label} missing non-empty array at {pointer}").into());
        }
    }
    if !value["producer_contracts"].is_array() {
        return Err(format!("{label} producer_contracts must be an array").into());
    }
    let artifacts = value["artifacts"]
        .as_array()
        .ok_or_else(|| format!("{label} artifacts must be an array"))?;
    let artifact_ids: BTreeSet<String> = artifacts
        .iter()
        .filter_map(|artifact| artifact["id"].as_str().map(str::to_string))
        .collect();
    for artifact in artifacts {
        for pointer in ["/id", "/kind", "/audience", "/owner", "/status"] {
            if artifact
                .pointer(pointer)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                return Err(format!("{label} artifact missing string at {pointer}").into());
            }
        }
        if artifact["acceptance"]
            .as_array()
            .is_none_or(|items| items.is_empty())
        {
            return Err(format!("{label} artifact missing acceptance checks").into());
        }
    }
    for provenance in value["provenance"]
        .as_array()
        .ok_or_else(|| format!("{label} provenance must be an array"))?
    {
        let artifact_id = provenance["artifact_id"].as_str().unwrap_or_default();
        if !artifact_ids.contains(artifact_id) {
            return Err(
                format!("{label} provenance references unknown artifact `{artifact_id}`").into(),
            );
        }
        if provenance["sources"]
            .as_array()
            .is_none_or(|items| items.is_empty())
        {
            return Err(format!("{label} provenance missing sources").into());
        }
    }
    for approval in value["approvals"]
        .as_array()
        .ok_or_else(|| format!("{label} approvals must be an array"))?
    {
        let artifact_id = approval["artifact_id"].as_str().unwrap_or_default();
        if !artifact_ids.contains(artifact_id) {
            return Err(
                format!("{label} approval references unknown artifact `{artifact_id}`").into(),
            );
        }
        if approval["state"].as_str().unwrap_or_default().is_empty() {
            return Err(format!("{label} approval missing state").into());
        }
    }
    for contract in value["producer_contracts"]
        .as_array()
        .ok_or_else(|| format!("{label} producer_contracts must be an array"))?
    {
        for pointer in [
            "/id",
            "/producer",
            "/adapter_kind",
            "/command",
            "/evidence_path",
        ] {
            if contract
                .pointer(pointer)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                return Err(
                    format!("{label} producer contract missing string at {pointer}").into(),
                );
            }
        }
        if !contract["mutates"].is_boolean() {
            return Err(format!("{label} producer contract missing mutates boolean").into());
        }
        for pointer in ["/input_artifacts", "/output_artifacts", "/acceptance"] {
            if contract
                .pointer(pointer)
                .and_then(Value::as_array)
                .is_none_or(|items| items.is_empty())
            {
                return Err(format!(
                    "{label} producer contract missing non-empty array at {pointer}"
                )
                .into());
            }
        }
        for pointer in ["/input_artifacts", "/output_artifacts"] {
            for artifact_id in contract
                .pointer(pointer)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if !artifact_ids.contains(artifact_id) {
                    return Err(format!(
                        "{label} producer contract references unknown artifact `{artifact_id}`"
                    )
                    .into());
                }
            }
        }
    }
    Ok(())
}

fn assert_schema_contract(value: &Value, label: &str) -> Result<()> {
    let schema_path = env::current_dir()?.join("schemas/release-kit.v1.schema.json");
    let schema: Value = serde_json::from_str(&fs::read_to_string(&schema_path)?)?;
    assert_supported_schema_keywords(&schema, &schema_path)?;
    let mut errors = Vec::new();
    validate_contract_schema_node(&schema, value, "$", &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{label} does not validate against {}:\n{}",
            schema_path.display(),
            errors.join("\n")
        )
        .into())
    }
}

fn assert_supported_schema_keywords(schema: &Value, schema_path: &std::path::Path) -> Result<()> {
    let mut unsupported = Vec::new();
    collect_unsupported_schema_keywords(schema, "$", &mut unsupported);
    if unsupported.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} uses unsupported JSON Schema keywords for Landmark's replay contract checker:\n{}",
            schema_path.display(),
            unsupported.join("\n")
        )
        .into())
    }
}

fn collect_unsupported_schema_keywords(schema: &Value, path: &str, errors: &mut Vec<String>) {
    let Some(object) = schema.as_object() else {
        return;
    };
    for key in object.keys() {
        if key.starts_with("x-") || supported_schema_keyword(key) {
            continue;
        }
        errors.push(format!("{path} unsupported keyword `{key}`"));
    }
    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for (property, property_schema) in properties {
            collect_unsupported_schema_keywords(
                property_schema,
                &format!("{path}/properties/{property}"),
                errors,
            );
        }
    }
    if let Some(item_schema) = object.get("items") {
        collect_unsupported_schema_keywords(item_schema, &format!("{path}/items"), errors);
    }
}

fn supported_schema_keyword(keyword: &str) -> bool {
    matches!(
        keyword,
        "$schema"
            | "$id"
            | "title"
            | "type"
            | "additionalProperties"
            | "required"
            | "properties"
            | "const"
            | "enum"
            | "items"
    )
}

fn validate_contract_schema_node(
    schema: &Value,
    value: &Value,
    path: &str,
    errors: &mut Vec<String>,
) {
    if let Some(expected) = schema.get("const")
        && value != expected
    {
        errors.push(format!("{path} expected const {expected}, got {value}"));
    }
    if let Some(variants) = schema.get("enum").and_then(Value::as_array)
        && !variants.iter().any(|variant| variant == value)
    {
        errors.push(format!("{path} expected one of {variants:?}, got {value}"));
    }
    let Some(schema_type) = schema.get("type").and_then(Value::as_str) else {
        return;
    };
    match schema_type {
        "object" => validate_object_schema_node(schema, value, path, errors),
        "array" => validate_array_schema_node(schema, value, path, errors),
        "string" if !value.is_string() => {
            errors.push(format!("{path} expected string, got {}", json_type(value)));
        }
        "boolean" if !value.is_boolean() => {
            errors.push(format!("{path} expected boolean, got {}", json_type(value)));
        }
        "integer" if !value.is_i64() && !value.is_u64() => {
            errors.push(format!("{path} expected integer, got {}", json_type(value)));
        }
        "number" if !value.is_number() => {
            errors.push(format!("{path} expected number, got {}", json_type(value)));
        }
        _ => {}
    }
}

fn validate_object_schema_node(
    schema: &Value,
    value: &Value,
    path: &str,
    errors: &mut Vec<String>,
) {
    let Some(object) = value.as_object() else {
        errors.push(format!("{path} expected object, got {}", json_type(value)));
        return;
    };
    let required: BTreeSet<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    for key in &required {
        if !object.contains_key(*key) {
            errors.push(format!("{path} missing required property `{key}`"));
        }
    }
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
        for key in object.keys() {
            if !properties.contains_key(key) {
                errors.push(format!("{path} has unexpected property `{key}`"));
            }
        }
    }
    for (key, property_schema) in properties {
        if let Some(child) = object.get(&key) {
            validate_contract_schema_node(
                &property_schema,
                child,
                &format!("{path}/{key}"),
                errors,
            );
        }
    }
}

fn validate_array_schema_node(schema: &Value, value: &Value, path: &str, errors: &mut Vec<String>) {
    let Some(items) = value.as_array() else {
        errors.push(format!("{path} expected array, got {}", json_type(value)));
        return;
    };
    if let Some(item_schema) = schema.get("items") {
        for (index, item) in items.iter().enumerate() {
            validate_contract_schema_node(item_schema, item, &format!("{path}/{index}"), errors);
        }
    }
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
