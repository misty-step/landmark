use super::{
    ContextCommit, DeterministicReleaseContext, LandmarkManifest, RunArgs, RunArtifactRecord,
    RunPublicationRecord, RunReleaseContext, RunVersionDecision,
    classify_release_context_with_deterministic, context_changed_files, context_source,
    conventional_commit_type, is_breaking_commit, sha256_hex, trimmed_option,
};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub(crate) const SCHEMA_VERSION: &str = "landmark.release-kit.v1";
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
        changed_files: context_changed_files(&args.repo_root, &release.version),
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

pub(crate) fn release_kit_importance(
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

pub(crate) fn release_kit_audiences(primary: &str, importance: &str) -> Vec<String> {
    let mut audiences = BTreeSet::new();
    audiences.insert(primary.to_string());
    audiences.insert("developer-operator".into());
    if release_kit_needs_rich_artifacts(importance) {
        audiences.insert("release-operator".into());
        audiences.insert("docs-owner".into());
    }
    audiences.into_iter().collect()
}

pub(crate) fn release_kit_needs_rich_artifacts(importance: &str) -> bool {
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
