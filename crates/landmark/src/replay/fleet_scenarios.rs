use crate::*;

pub(crate) fn scenario_fleet_adoption_planner(tmp_root: &Path) -> Result<Value> {
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

pub(crate) fn existing_release_please_workflow() -> String {
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

pub(crate) fn existing_changesets_workflow() -> String {
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

pub(crate) fn existing_semantic_release_workflow() -> String {
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

pub(crate) fn fleet_fixture_repo(
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

pub(crate) fn fleet_fixture_repo_with_packages(
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

pub(crate) fn fleet_existing_landmark_fixture() -> FleetRepository {
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

pub(crate) fn fleet_existing_landmark_workflow_fixture() -> FleetRepository {
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

pub(crate) fn fleet_existing_semantic_release_workflow_fixture() -> FleetRepository {
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

pub(crate) fn fleet_incomplete_secret_fixture() -> FleetRepository {
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
