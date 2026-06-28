use crate::*;
pub(crate) fn workflow_invokes_landmark(text: &str) -> bool {
    workflow_invokes_landmark_action(text)
}

pub(crate) fn workflow_invokes_landmark_action(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("misty-step/landmark") || lower.contains("misty-step/landmark")
}

pub(crate) fn fleet_workflow_file(path: &str, text: &str) -> Option<FleetWorkflowFile> {
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

pub(crate) fn workflow_job_invoking(text: &str, marker: &str) -> Option<String> {
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

pub(crate) fn fleet_release_tool(
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

pub(crate) fn fleet_tag_format(tags: &[String], packages: &[String]) -> String {
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

pub(crate) fn classify_fleet_repository_kind(
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

pub(crate) fn classify_fleet_release_surface(
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

pub(crate) fn normalized_repo_kind(repo: &FleetRepository) -> String {
    trimmed_option(&repo.repository_kind)
        .unwrap_or_else(|| {
            classify_fleet_repository_kind(&repo.name, &repo.package_topology, &repo.release_files)
        })
        .to_ascii_lowercase()
}

pub(crate) fn normalized_release_surface(repo: &FleetRepository) -> String {
    trimmed_option(&repo.release_surface)
        .unwrap_or_else(|| classify_fleet_release_surface(&repo.release_tool, &[], &[]))
        .to_ascii_lowercase()
}

pub(crate) fn fleet_workflow_file_for_tool<'a>(
    repo: &'a FleetRepository,
    release_tool: &str,
) -> Option<&'a FleetWorkflowFile> {
    repo.workflow_files
        .iter()
        .find(|workflow| workflow.release_tool == release_tool)
}

pub(crate) fn workflow_content_blocker(
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

pub(crate) fn workflow_has_jobs_mapping(text: &str) -> bool {
    let Ok(raw) = serde_yaml::from_str::<serde_yaml::Value>(text) else {
        return false;
    };
    raw.get("jobs")
        .and_then(serde_yaml::Value::as_mapping)
        .is_some()
}

pub(crate) fn fleet_integration_mode(
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

pub(crate) fn fleet_required_secret_names(integration_mode: &str) -> Vec<String> {
    match integration_mode {
        "github-full" | "github-synthesis-only" => {
            vec!["GH_RELEASE_TOKEN".into(), "OPENROUTER_API_KEY".into()]
        }
        _ => Vec::new(),
    }
}

pub(crate) fn fleet_initial_version(recommended_mode: &str, status: &str) -> String {
    if recommended_mode == "backfill-first" && status == "ready" {
        "0.1.0".into()
    } else {
        String::new()
    }
}

pub(crate) fn fleet_initial_tag(repo: &FleetRepository, version: &str) -> String {
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

pub(crate) fn fleet_manifest_artifact_paths(manifest: &LandmarkManifest) -> Vec<String> {
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

pub(crate) fn fleet_historical_preview_command(tag: &str) -> String {
    if tag.is_empty() {
        String::new()
    } else {
        format!("landmark backfill --repo-root . --since {tag} --mode artifacts-only --dry-run")
    }
}

pub(crate) fn fleet_rollback_guidance(recommended_mode: &str) -> String {
    if recommended_mode == "backfill-first" {
        "close the PR and delete the adoption branch; remove .landmark.yml and any previewed local artifact files, and remove the operator-approved initial tag only if it was created before any release was published".into()
    } else {
        String::new()
    }
}

pub(crate) fn plan_fleet_repository(repo: &FleetRepository) -> FleetRepositoryPlan {
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
