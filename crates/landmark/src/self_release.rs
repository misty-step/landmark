use crate::*;

pub(crate) fn prepare_self_release(args: PrepareSelfReleaseArgs) -> Result<()> {
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
            decisive_commit: None,
            unknown_commits: Vec::new(),
        };
        return emit_self_release_plan(&plan, &args.github_output);
    }

    let classified = self_release_commits(&args.repo_root, &format!("v{latest_version}"))?;
    let decision = decide_version(&classified);
    let changelog_commits = release_worthy_commits(&classified);
    let unknown_commits: Vec<String> = decision
        .unknown_commits
        .iter()
        .map(ClassifiedCommit::evidence_line)
        .collect();
    let Some(bump) = decision.bump else {
        let reason = if unknown_commits.is_empty() {
            "no release-worthy conventional commits since latest tag".to_string()
        } else {
            format!(
                "refusing to release: {} commit(s) since latest tag do not follow conventional-commit format and no feat/fix/perf/breaking/revert signal was found: {}",
                unknown_commits.len(),
                unknown_commits.join("; ")
            )
        };
        let plan = SelfReleasePlan {
            released: false,
            reason,
            latest_version,
            next_version: package_version,
            release_tag: String::new(),
            release_branch: args.release_branch,
            pull_request_title: String::new(),
            commit_message: String::new(),
            changed_files: Vec::new(),
            changelog: String::new(),
            commits: changelog_commits,
            decisive_commit: None,
            unknown_commits,
        };
        return emit_self_release_plan(&plan, &args.github_output);
    };
    let decisive_commit = decision
        .decisive
        .as_ref()
        .map(ClassifiedCommit::evidence_line);

    let next_version = bump_version(&latest_version, bump)?;
    let release_tag = format!("v{next_version}");
    let changelog = render_self_release_changelog(
        &args.repository,
        &latest_version,
        &next_version,
        &release_tag,
        &changelog_commits,
    );
    prepend_changelog(&args.repo_root.join("CHANGELOG.md"), &changelog)?;
    update_version_metadata(UpdateVersionArgs {
        version: next_version.clone(),
        repo_root: args.repo_root.clone(),
    })?;
    update_lock_package_version(
        &args.repo_root.join("Cargo.lock"),
        "landmark",
        &next_version,
    )?;

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
            "crates/landmark/Cargo.toml".into(),
            "Cargo.lock".into(),
        ],
        changelog,
        commits: changelog_commits,
        decisive_commit,
        unknown_commits,
    };
    emit_self_release_plan(&plan, &args.github_output)
}

pub(crate) fn emit_self_release_plan(plan: &SelfReleasePlan, github_output: &str) -> Result<()> {
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

pub(crate) fn publish_self_release(args: PublishSelfReleaseArgs) -> Result<()> {
    validate_repo(&args.repository)?;
    validate_nonblank(&args.target_sha, "target-sha")?;
    let latest_version = latest_repo_version(&args.repo_root)?;
    let package_version = package_version(&args.repo_root)?;
    let cargo = cargo_version(&args.repo_root.join("crates/landmark/Cargo.toml"))
        .ok_or("crates/landmark/Cargo.toml missing package version")?;
    if cargo != package_version {
        return Err(format!(
            "package.json has {package_version}, crates/landmark/Cargo.toml has {cargo}"
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
    let provider = GitHubProvider::required(&args.api_base_url, &args.github_token);
    if let Some(value) = provider.release_by_tag(&args.repository, &release_tag)? {
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

    let body = changelog_section(&args.repo_root.join("CHANGELOG.md"), &package_version)?;
    let release_url =
        provider.create_release(&args.repository, &release_tag, &args.target_sha, &body)?;
    let publish = SelfReleasePublish {
        published: true,
        reason: "published release from landed release pull request".into(),
        latest_version,
        version: package_version,
        release_tag,
        release_url,
    };
    emit_self_release_publish(&publish, &args.github_output)
}

pub(crate) fn emit_self_release_publish(
    publish: &SelfReleasePublish,
    github_output: &str,
) -> Result<()> {
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

pub(crate) fn latest_repo_version(repo_root: &Path) -> Result<String> {
    let tags = run_ok("git", ["tag", "--merged", "HEAD"], repo_root)?;
    latest_semver_version(tags.lines()).ok_or("no semver tags found".into())
}

pub(crate) fn package_version(repo_root: &Path) -> Result<String> {
    let package: Value =
        serde_json::from_str(&fs::read_to_string(repo_root.join("package.json"))?)?;
    package["version"]
        .as_str()
        .map(str::to_string)
        .ok_or("package.json missing version".into())
}

pub(crate) fn self_release_commits(repo_root: &Path, tag: &str) -> Result<Vec<ClassifiedCommit>> {
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
        commits.push(classify_commit(&hash, &subject, &body));
    }
    Ok(commits)
}

/// The changelog only ever renders commits that actually drove (or could have
/// driven) a version bump; `chore`/`docs`/unparseable commits are excluded.
pub(crate) fn release_worthy_commits(commits: &[ClassifiedCommit]) -> Vec<SelfReleaseCommit> {
    commits
        .iter()
        .filter(|commit| {
            matches!(
                commit.category,
                CommitCategory::Breaking | CommitCategory::Feature | CommitCategory::Fix
            )
        })
        .map(|commit| SelfReleaseCommit {
            hash: commit.id.clone(),
            short_hash: commit.id.chars().take(7).collect(),
            subject: commit.subject.clone(),
            category: if commit.breaking {
                "breaking".to_string()
            } else {
                match commit.kind.as_str() {
                    "feat" => "features".to_string(),
                    "perf" => "performance".to_string(),
                    "revert" => "reverts".to_string(),
                    _ => "fixes".to_string(),
                }
            },
            scope: commit.scope.clone(),
            description: commit.description.clone(),
            breaking: commit.breaking,
        })
        .collect()
}

pub(crate) fn bump_version(version: &str, bump: VersionBump) -> Result<String> {
    let (major, minor, patch) = semver_key(version)?;
    Ok(match bump {
        VersionBump::Major => format!("{}.0.0", major + 1),
        VersionBump::Minor => format!("{major}.{}.0", minor + 1),
        VersionBump::Patch => format!("{major}.{minor}.{}", patch + 1),
    })
}

pub(crate) fn semver_key(version: &str) -> Result<(u64, u64, u64)> {
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

pub(crate) fn render_self_release_changelog(
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
        ("reverts", "### Reverts"),
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

pub(crate) fn prepend_changelog(path: &Path, entry: &str) -> Result<()> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    fs::write(
        path,
        format!("{}\n{}", entry.trim_end(), existing.trim_start()),
    )?;
    Ok(())
}

pub(crate) fn update_lock_package_version(
    path: &Path,
    package_name: &str,
    version: &str,
) -> Result<()> {
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

pub(crate) fn changelog_section(path: &Path, version: &str) -> Result<String> {
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

pub(crate) fn release_candidate_changelog_exists(path: &Path, version: &str) -> bool {
    changelog_section(path, version)
        .map(|section| {
            section
                .lines()
                .map(str::trim)
                .any(|line| line.starts_with("* ") || line.starts_with("- "))
        })
        .unwrap_or(false)
}
