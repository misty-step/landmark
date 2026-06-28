use crate::*;
pub(crate) fn print_fleet_scan_result(path: &Path, scan: &FleetScan, format: &str) -> Result<()> {
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

pub(crate) fn gh_repo_list(
    owner: &str,
    max_repos: usize,
    token: Option<&str>,
) -> Result<Vec<Value>> {
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

pub(crate) fn scan_fleet_repositories_bounded(
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

pub(crate) fn scan_fleet_repository(
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

pub(crate) fn run_gh_ok(args: Vec<String>, token: Option<&str>) -> Result<String> {
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

pub(crate) fn unavailable_secret_statuses(
    required: &[&str],
    detail: &str,
) -> Vec<FleetSecretStatus> {
    required
        .iter()
        .map(|name| FleetSecretStatus {
            name: (*name).into(),
            status: "unavailable".into(),
            detail: detail.into(),
        })
        .collect()
}

pub(crate) fn secret_names_from_array(value: &Value) -> BTreeSet<String> {
    value["secrets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|secret| secret["name"].as_str())
        .map(str::to_string)
        .collect()
}

pub(crate) fn org_secret_names_for_repo(
    value: &Value,
    repository: &str,
    repo_name: &str,
) -> BTreeSet<String> {
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
