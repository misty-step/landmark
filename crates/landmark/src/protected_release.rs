use crate::*;

const MARKER_PREFIX: &str = "<!-- landmark:protected-release ";

#[derive(Serialize)]
struct ProtectedReleasePlan {
    released: bool,
    release_tag: String,
    release_branch: String,
    pull_request_title: String,
    commit_message: String,
}

#[derive(Serialize)]
struct ProtectedReleasePublish {
    pending: bool,
    published: bool,
    release_tag: String,
}

fn marker(previous: &str, commits: &[ClassifiedCommit]) -> String {
    let mut digest = Sha256::new();
    for commit in commits {
        digest.update(commit.id.as_bytes());
        digest.update([0]);
        digest.update(commit.subject.as_bytes());
        digest.update([0]);
    }
    format!(
        "{MARKER_PREFIX}previous=v{previous} source={} -->",
        hex::encode(digest.finalize())
    )
}

fn previous_from_marker(section: &str) -> Result<Option<String>> {
    let mut matches = section
        .lines()
        .filter(|line| line.starts_with(MARKER_PREFIX));
    let Some(line) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() {
        return Err("duplicate protected release markers".into());
    }
    let previous = line
        .strip_prefix(MARKER_PREFIX)
        .and_then(|value| value.split_once(" source="))
        .map(|(previous, _)| previous)
        .and_then(|value| value.strip_prefix("previous=v"))
        .ok_or("invalid protected release marker")?;
    semver_key(previous)?;
    Ok(Some(previous.to_string()))
}

fn checked_candidate(
    repo_root: &Path,
    repository: &str,
    previous: &str,
    section: &str,
    until: &str,
) -> Result<String> {
    let commits = self_release_commits_until(repo_root, &format!("v{previous}"), until)?;
    let bump = decide_version(&commits)
        .bump
        .ok_or("protected release has no release-worthy commits")?;
    let version = bump_version(previous, apply_stability(bump, previous))?;
    let title = format!(
        "# [{version}](https://github.com/{repository}/compare/v{previous}...v{version}) ("
    );
    let first_line = section
        .lines()
        .next()
        .ok_or("empty protected release section")?;
    first_line
        .strip_prefix(&title)
        .and_then(|value| value.strip_suffix(')'))
        .filter(|value| {
            static DATE: std::sync::LazyLock<Regex> =
                std::sync::LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap());
            DATE.is_match(value)
        })
        .ok_or("protected release heading differs from candidate")?;
    let expected = render_self_release_changelog(
        repository,
        previous,
        &version,
        &format!("v{version}"),
        &release_worthy_commits(&commits),
    );
    let expected_heading = expected.lines().next().ok_or("empty rendered changelog")?;
    let expected = expected.replacen(expected_heading, first_line, 1);
    let expected = expected.replacen("\n\n", &format!("\n{}\n\n", marker(previous, &commits)), 1);
    if section.trim_end() != expected.trim_end() {
        return Err("protected release changelog differs from classified candidate commits".into());
    }
    Ok(version)
}

fn top_section(path: &Path) -> Result<Option<String>> {
    let changelog = fs::read_to_string(path)?;
    let Some(heading) = changelog.lines().next() else {
        return Ok(None);
    };
    let Some(version) = heading
        .strip_prefix("# [")
        .and_then(|value| value.split_once(']').map(|(version, _)| version))
    else {
        return Ok(None);
    };
    Ok(Some(changelog_section(path, version)?))
}

pub(crate) fn prepare_protected_release(args: PrepareProtectedReleaseArgs) -> Result<()> {
    validate_repo(&args.repository)?;
    validate_nonblank(&args.release_branch, "release-branch")?;
    let latest = latest_repo_version(&args.repo_root)?;
    let path = args.repo_root.join("CHANGELOG.md");
    if let Some(section) = top_section(&path)?
        && let Some(previous) = previous_from_marker(&section)?
    {
        let claimed_version = section
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("# ["))
            .and_then(|line| line.split_once(']').map(|(version, _)| version))
            .ok_or("invalid protected release heading")?;
        if latest == claimed_version {
            let version = checked_candidate(
                &args.repo_root,
                &args.repository,
                &previous,
                &section,
                &format!("v{latest}"),
            )?;
            if version != latest {
                return Err("protected release marker conflicts with latest tag".into());
            }
        } else {
            checked_candidate(
                &args.repo_root,
                &args.repository,
                &previous,
                &section,
                "HEAD",
            )?;
            if latest != previous {
                return Err("protected release marker conflicts with latest tag".into());
            }
            return emit_prepare(
                ProtectedReleasePlan {
                    released: false,
                    release_tag: String::new(),
                    release_branch: args.release_branch,
                    pull_request_title: String::new(),
                    commit_message: String::new(),
                },
                &args.github_output,
            );
        }
    }
    let commits = self_release_commits(&args.repo_root, &format!("v{latest}"))?;
    let Some(bump) = decide_version(&commits).bump else {
        return emit_prepare(
            ProtectedReleasePlan {
                released: false,
                release_tag: String::new(),
                release_branch: args.release_branch,
                pull_request_title: String::new(),
                commit_message: String::new(),
            },
            &args.github_output,
        );
    };
    let version = bump_version(&latest, apply_stability(bump, &latest))?;
    let tag = format!("v{version}");
    let entry = render_self_release_changelog(
        &args.repository,
        &latest,
        &version,
        &tag,
        &release_worthy_commits(&commits),
    );
    let entry = entry.replacen("\n\n", &format!("\n{}\n\n", marker(&latest, &commits)), 1);
    // Check the existing section before writing, so a stale or manually
    // modified candidate cannot be silently replaced by a new release.
    if let Some(section) = top_section(&path)?
        && section
            .lines()
            .next()
            .is_some_and(|line| line.starts_with(&format!("# [{version}]")))
    {
        return Err("unmarked changelog section already claims candidate version".into());
    }
    prepend_changelog(&path, &entry)?;
    emit_prepare(
        ProtectedReleasePlan {
            released: true,
            release_tag: tag,
            release_branch: args.release_branch,
            pull_request_title: format!("chore(release): {version}"),
            commit_message: format!("chore(release): {version}"),
        },
        &args.github_output,
    )
}

fn emit_prepare(plan: ProtectedReleasePlan, github_output: &str) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&plan)?);
    if !github_output.is_empty() {
        write_outputs(
            Path::new(github_output),
            &[
                ("released", plan.released.to_string()),
                ("release_tag", plan.release_tag),
                ("release_branch", plan.release_branch),
                ("pull_request_title", plan.pull_request_title),
                ("commit_message", plan.commit_message),
            ],
        )?;
    }
    Ok(())
}

pub(crate) fn publish_protected_release(args: PublishProtectedReleaseArgs) -> Result<()> {
    validate_repo(&args.repository)?;
    validate_nonblank(&args.github_token, "github-token")?;
    if !is_full_hex(&args.target_sha)
        || !args.target_sha.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("target-sha must be a full hexadecimal commit SHA".into());
    }
    let head = run_ok("git", ["rev-parse", "HEAD"], &args.repo_root)?;
    if head.trim() != args.target_sha.to_ascii_lowercase() {
        return Err("target-sha differs from the checked-out commit".into());
    }
    let mut result = ProtectedReleasePublish {
        pending: false,
        published: false,
        release_tag: String::new(),
    };
    let Some(section) = top_section(&args.repo_root.join("CHANGELOG.md"))? else {
        return emit_publish(result, &args.github_output);
    };
    let Some(previous) = previous_from_marker(&section)? else {
        return emit_publish(result, &args.github_output);
    };
    let landed_changelog = run_ok("git", ["show", "HEAD:CHANGELOG.md"], &args.repo_root)?;
    if fs::read_to_string(args.repo_root.join("CHANGELOG.md"))? != landed_changelog {
        return Err("protected release changelog is not committed at target-sha".into());
    }
    let latest = latest_repo_version(&args.repo_root)?;
    let claimed_version = section
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("# ["))
        .and_then(|line| line.split_once(']').map(|(version, _)| version))
        .ok_or("invalid protected release heading")?;
    let version = if latest == claimed_version {
        let version = checked_candidate(
            &args.repo_root,
            &args.repository,
            &previous,
            &section,
            &format!("v{latest}"),
        )?;
        let local_tag = run_ok(
            "git",
            ["rev-parse", &format!("refs/tags/v{version}^{{commit}}")],
            &args.repo_root,
        )?;
        if local_tag.trim() != head.trim() {
            let provider = GitHubProvider::required(&args.api_base_url, &args.github_token);
            let identity = ReleaseIdentity {
                release_tag: format!("v{version}"),
                source_revision: local_tag.trim().to_string(),
            };
            let state = observe_release_state(&provider, &args.repository, &identity.release_tag)?;
            if !matches!(
                plan_publication(&identity, &state)?,
                CommitPlan::Reconcile { .. }
            ) {
                return Err(
                    "previous protected release is not published at its tagged commit".into(),
                );
            }
            return emit_publish(result, &args.github_output);
        }
        version
    } else {
        let version = checked_candidate(
            &args.repo_root,
            &args.repository,
            &previous,
            &section,
            "HEAD",
        )?;
        if latest != previous || version != claimed_version {
            return Err("protected release candidate conflicts with latest tag".into());
        }
        version
    };
    result.pending = true;
    result.release_tag = format!("v{version}");
    let provider = GitHubProvider::required(&args.api_base_url, &args.github_token);
    let identity = ReleaseIdentity {
        release_tag: result.release_tag.clone(),
        source_revision: head.trim().to_string(),
    };
    let state = observe_release_state(&provider, &args.repository, &identity.release_tag)?;
    if matches!(
        plan_publication(&identity, &state)?,
        CommitPlan::Reconcile { .. }
    ) {
        result.pending = false;
        return emit_publish(result, &args.github_output);
    }
    provider.create_release(
        &args.repository,
        &identity.release_tag,
        head.trim(),
        &section,
    )?;
    let observed = observe_release_state(&provider, &args.repository, &identity.release_tag)?;
    if !matches!(
        plan_publication(&identity, &observed)?,
        CommitPlan::Reconcile { .. }
    ) {
        return Err("protected release publication did not reconcile after creation".into());
    }
    result.pending = false;
    result.published = true;
    emit_publish(result, &args.github_output)
}

fn emit_publish(result: ProtectedReleasePublish, github_output: &str) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&result)?);
    if !github_output.is_empty() {
        write_outputs(
            Path::new(github_output),
            &[
                ("pending", result.pending.to_string()),
                ("published", result.published.to_string()),
                ("release_tag", result.release_tag),
            ],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepare(repo: &Path, output: &Path) -> Result<()> {
        prepare_protected_release(PrepareProtectedReleaseArgs {
            repo_root: repo.to_path_buf(),
            repository: "owner/repo".into(),
            release_branch: "landmark/release".into(),
            github_output: output.display().to_string(),
        })
    }

    fn publish(repo: &Path, sha: &str, url: &str, output: &Path) -> Result<()> {
        publish_protected_release(PublishProtectedReleaseArgs {
            repo_root: repo.to_path_buf(),
            repository: "owner/repo".into(),
            github_token: "token".into(),
            target_sha: sha.into(),
            api_base_url: url.into(),
            github_output: output.display().to_string(),
        })
    }

    #[test]
    fn prepared_candidate_is_immutable_and_publishes_only_after_landing() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path().join("repo");
        init_self_release_fixture(&repo)?;
        let output = temp.path().join("output");
        let previous = fs::read_to_string(repo.join("CHANGELOG.md"))?;
        prepare(&repo, &output)?;
        let outputs = parse_outputs(&output)?;
        assert_eq!(outputs["released"], "true");
        assert_eq!(outputs["release_tag"], "v1.1.0");
        assert_eq!(outputs["pull_request_title"], "chore(release): 1.1.0");
        let section = fs::read_to_string(repo.join("CHANGELOG.md"))?;
        assert!(section.starts_with("# [1.1.0]"));
        assert!(section.contains(MARKER_PREFIX));
        assert!(section.ends_with(&previous));
        prepare(&repo, &output)?;
        assert_eq!(parse_outputs(&output)?["released"], "false");
        assert_eq!(fs::read_to_string(repo.join("CHANGELOG.md"))?, section);
        let server = start_fake_server(FakeState::default())?;
        let unlanded_sha = run_ok("git", ["rev-parse", "HEAD"], &repo)?
            .trim()
            .to_string();
        assert!(publish(&repo, &unlanded_sha, &server.url, &output).is_err());
        assert!(server.state.lock().unwrap().requests.is_empty());
        run_ok("git", ["add", "CHANGELOG.md"], &repo)?;
        run_ok(
            "git",
            ["commit", "-q", "-m", "chore(release): 1.1.0"],
            &repo,
        )?;
        let sha = run_ok("git", ["rev-parse", "HEAD"], &repo)?
            .trim()
            .to_string();
        publish(&repo, &sha, &server.url, &output)?;
        let outputs = parse_outputs(&output)?;
        assert_eq!(outputs["published"], "true");
        assert_eq!(outputs["pending"], "false");
        assert_eq!(outputs["release_tag"], "v1.1.0");
        let state = server.state.lock().unwrap();
        assert_eq!(state.releases["v1.1.0"]["target_commitish"], sha);
        assert_eq!(
            state.releases["v1.1.0"]["body"],
            changelog_section(&repo.join("CHANGELOG.md"), "1.1.0")?
        );
        drop(state);
        publish(&repo, &sha, &server.url, &output)?;
        assert_eq!(parse_outputs(&output)?["published"], "false");
        assert_eq!(server.state.lock().unwrap().releases.len(), 1);
        run_ok("git", ["tag", "v1.1.0", &sha], &repo)?;
        fs::write(repo.join("README.md"), "# Fixture\n\nAnother feature.\n")?;
        run_ok("git", ["add", "README.md"], &repo)?;
        run_ok(
            "git",
            ["commit", "-q", "-m", "feat: add another feature"],
            &repo,
        )?;
        let later_sha = run_ok("git", ["rev-parse", "HEAD"], &repo)?
            .trim()
            .to_string();
        publish(&repo, &later_sha, &server.url, &output)?;
        assert_eq!(parse_outputs(&output)?["published"], "false");
        assert_eq!(server.state.lock().unwrap().releases.len(), 1);
        prepare(&repo, &output)?;
        assert_eq!(parse_outputs(&output)?["release_tag"], "v1.2.0");
        Ok(())
    }

    #[test]
    fn no_bump_and_prestable_bump_and_contradictions() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path().join("repo");
        init_self_release_fixture(&repo)?;
        let output = temp.path().join("output");
        run_ok("git", ["tag", "-d", "v1.0.0"], &repo)?;
        let root = run_ok("git", ["rev-list", "--max-parents=0", "HEAD"], &repo)?;
        run_ok("git", ["tag", "v0.3.0", root.trim()], &repo)?;
        prepare(&repo, &output)?;
        assert_eq!(parse_outputs(&output)?["release_tag"], "v0.3.1");
        let section = fs::read_to_string(repo.join("CHANGELOG.md"))?;
        fs::write(
            repo.join("CHANGELOG.md"),
            section.replace("add protected branch self release", "altered entry"),
        )?;
        run_ok("git", ["add", "CHANGELOG.md"], &repo)?;
        run_ok(
            "git",
            ["commit", "-q", "-m", "chore(release): 0.3.1"],
            &repo,
        )?;
        let sha = run_ok("git", ["rev-parse", "HEAD"], &repo)?
            .trim()
            .to_string();
        let server = start_fake_server(FakeState::default())?;
        assert!(publish(&repo, &sha, &server.url, &output).is_err());
        assert!(server.state.lock().unwrap().releases.is_empty());
        fs::write(repo.join("CHANGELOG.md"), section)?;
        run_ok("git", ["add", "CHANGELOG.md"], &repo)?;
        run_ok(
            "git",
            ["commit", "-q", "-m", "chore(release): repair candidate"],
            &repo,
        )?;
        let sha = run_ok("git", ["rev-parse", "HEAD"], &repo)?
            .trim()
            .to_string();
        let mut conflicting = FakeState::default();
        conflicting.releases.insert(
            "v0.3.1".into(),
            json!({
                "id": 1, "tag_name": "v0.3.1", "target_commitish": "a".repeat(40),
                "html_url": "https://example.invalid/release/v0.3.1"
            }),
        );
        let conflicting_server = start_fake_server(conflicting)?;
        assert!(publish(&repo, &sha, &conflicting_server.url, &output).is_err());
        assert_eq!(conflicting_server.state.lock().unwrap().releases.len(), 1);

        let quiet = temp.path().join("quiet");
        init_self_release_fixture(&quiet)?;
        run_ok("git", ["tag", "v1.1.0"], &quiet)?;
        prepare(&quiet, &output)?;
        assert_eq!(parse_outputs(&output)?["released"], "false");
        let quiet_sha = run_ok("git", ["rev-parse", "HEAD"], &quiet)?
            .trim()
            .to_string();
        let quiet_server = start_fake_server(FakeState::default())?;
        publish(&quiet, &quiet_sha, &quiet_server.url, &output)?;
        assert_eq!(parse_outputs(&output)?["pending"], "false");
        assert!(quiet_server.state.lock().unwrap().requests.is_empty());
        Ok(())
    }
    #[test]
    fn source_drift_blocks_pending_release() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let repo = temp.path().join("repo");
        init_self_release_fixture(&repo)?;
        let output = temp.path().join("output");
        prepare(&repo, &output)?;
        run_ok("git", ["add", "CHANGELOG.md"], &repo)?;
        run_ok(
            "git",
            ["commit", "-q", "-m", "chore(release): 1.1.0"],
            &repo,
        )?;
        fs::write(
            repo.join("README.md"),
            "# Fixture\n\nUnexpected later change.\n",
        )?;
        run_ok("git", ["add", "README.md"], &repo)?;
        run_ok(
            "git",
            [
                "commit",
                "-q",
                "-m",
                "fix: change candidate after preparation",
            ],
            &repo,
        )?;
        let sha = run_ok("git", ["rev-parse", "HEAD"], &repo)?
            .trim()
            .to_string();
        let server = start_fake_server(FakeState::default())?;
        assert!(prepare(&repo, &output).is_err());
        assert!(publish(&repo, &sha, &server.url, &output).is_err());
        assert!(server.state.lock().unwrap().requests.is_empty());
        Ok(())
    }
}
