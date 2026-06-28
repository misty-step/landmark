use crate::*;
#[derive(Serialize)]
pub(crate) struct SetupReport {
    pub(crate) diagnosis: SetupDiagnosis,
    pub(crate) recommendation: SetupRecommendation,
    pub(crate) required_permissions: BTreeMap<String, String>,
    pub(crate) required_secrets: Vec<String>,
    pub(crate) workflows: BTreeMap<String, WorkflowCandidate>,
    pub(crate) manifest: Option<LandmarkManifest>,
    pub(crate) backfill: String,
}

#[derive(Serialize)]
pub(crate) struct SetupDiagnosis {
    pub(crate) release_tool: String,
    pub(crate) default_branch: String,
    pub(crate) tag_format: String,
    pub(crate) conventional_commits: String,
    pub(crate) monorepo: bool,
    pub(crate) packages: Vec<String>,
    pub(crate) signals: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct SetupRecommendation {
    pub(crate) mode: String,
    pub(crate) workflow: String,
    pub(crate) rationale: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct WorkflowCandidate {
    pub(crate) path: String,
    pub(crate) release_tool: String,
    pub(crate) mode: String,
    pub(crate) rationale: Vec<String>,
    pub(crate) content: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct FleetScan {
    pub(crate) generated_at: String,
    pub(crate) owners: Vec<String>,
    pub(crate) repositories: Vec<FleetRepository>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct FleetRepository {
    pub(crate) owner: String,
    pub(crate) name: String,
    pub(crate) name_with_owner: String,
    #[serde(default)]
    pub(crate) repository_kind: String,
    #[serde(default)]
    pub(crate) release_surface: String,
    pub(crate) private: bool,
    pub(crate) archived: bool,
    pub(crate) pushed_at: String,
    pub(crate) default_branch: String,
    pub(crate) branch_protected: String,
    pub(crate) release_tool: String,
    pub(crate) tag_format: String,
    pub(crate) package_topology: Vec<String>,
    pub(crate) release_files: Vec<String>,
    pub(crate) workflows: Vec<String>,
    #[serde(default)]
    pub(crate) workflow_files: Vec<FleetWorkflowFile>,
    pub(crate) existing_landmark: bool,
    pub(crate) required_secrets: Vec<FleetSecretStatus>,
    pub(crate) signals: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct FleetWorkflowFile {
    pub(crate) path: String,
    #[serde(default)]
    pub(crate) release_tool: String,
    #[serde(default)]
    pub(crate) release_job: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) content: String,
    #[serde(default)]
    pub(crate) content_redacted: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct FleetSecretStatus {
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) detail: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FleetSecretNames {
    pub(crate) names: BTreeSet<String>,
    pub(crate) repo_names: BTreeSet<String>,
    pub(crate) org_names: BTreeSet<String>,
    pub(crate) org_unavailable: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct FleetPlan {
    pub(crate) generated_at: String,
    pub(crate) source: String,
    pub(crate) summary: BTreeMap<String, usize>,
    pub(crate) repositories: Vec<FleetRepositoryPlan>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct FleetRepositoryPlan {
    pub(crate) repository: String,
    pub(crate) repository_kind: String,
    pub(crate) release_surface: String,
    pub(crate) rank: u64,
    pub(crate) default_branch: String,
    pub(crate) recommended_mode: String,
    pub(crate) integration_mode: String,
    pub(crate) integration_rationale: Vec<String>,
    pub(crate) workflow: String,
    pub(crate) status: String,
    pub(crate) skip_reason: String,
    pub(crate) risk_flags: Vec<String>,
    pub(crate) required_secrets: Vec<String>,
    pub(crate) missing_secrets: Vec<String>,
    pub(crate) unavailable_secret_metadata: Vec<String>,
    pub(crate) migration_notes: Vec<String>,
    #[serde(default)]
    pub(crate) initial_version_recommendation: String,
    #[serde(default)]
    pub(crate) initial_tag_recommendation: String,
    #[serde(default)]
    pub(crate) artifact_paths: Vec<String>,
    #[serde(default)]
    pub(crate) historical_preview_command: String,
    #[serde(default)]
    pub(crate) rollback_guidance: String,
    #[serde(default)]
    pub(crate) workflow_patches: Vec<FleetWorkflowPatch>,
    pub(crate) manifest: LandmarkManifest,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct FleetWorkflowPatch {
    pub(crate) path: String,
    pub(crate) description: String,
    pub(crate) content: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct FleetPrPlan {
    pub(crate) generated_at: String,
    pub(crate) dry_run: bool,
    pub(crate) repositories: Vec<FleetRepositoryPrPlan>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct FleetRepositoryPrPlan {
    pub(crate) repository: String,
    pub(crate) branch: String,
    pub(crate) title: String,
    pub(crate) commit_message: String,
    pub(crate) files: Vec<String>,
    pub(crate) skipped: bool,
    pub(crate) reason: String,
    pub(crate) disposition: String,
    pub(crate) rollback: String,
    pub(crate) monitor_status: String,
    pub(crate) evidence_dir: String,
}
