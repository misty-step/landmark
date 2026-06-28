use crate::*;
pub(crate) fn setup(args: SetupArgs) -> Result<()> {
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

pub(crate) fn fleet(args: FleetArgs) -> Result<()> {
    match args.command {
        FleetCommand::Scan(args) => fleet_scan(args),
        FleetCommand::Plan(args) => fleet_plan(args),
        FleetCommand::OpenPrs(args) => fleet_open_prs(args),
    }
}

pub(crate) fn fleet_scan(args: FleetScanArgs) -> Result<()> {
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

pub(crate) fn fleet_plan(args: FleetPlanArgs) -> Result<()> {
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

pub(crate) fn fleet_open_prs(args: FleetOpenPrsArgs) -> Result<()> {
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
