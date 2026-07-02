use crate::*;

fn paginated_pr(number: i64, title: &str, created_at: &str, merged_at: Option<&str>) -> Value {
    json!({
        "number": number,
        "title": title,
        "user": {"login": "octocat"},
        "created_at": created_at,
        "merged_at": merged_at,
    })
}

fn commit_with_date(repo: &Path, message: &str, date: &str) -> Result<()> {
    let status = Command::new("git")
        .args(["commit", "-q", "-m", message])
        .current_dir(repo)
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .status()?;
    if !status.success() {
        return Err(format!("git commit failed for {message}").into());
    }
    Ok(())
}

/// Regression for backlog.d/012's pagination gap: `closed_pull_requests`
/// fetched a single page (`per_page=100`) of closed PRs, sorted by GitHub's
/// default of creation date descending. A repo with more than 100 closed PRs
/// created since a real in-range PR — even PRs that never merged into this
/// release — could push that in-range PR off page 1 and silently drop it
/// from the changelog. `closed_pull_requests` must paginate past page 1
/// until it either exhausts GitHub's pages or walks back past the release's
/// `since` bound.
pub(crate) fn scenario_pr_fetch_paginates_past_first_page(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("pr-fetch-paginates-past-first-page");
    init_fixture_repo(&repo, "v1.0.0")?;
    fs::write(repo.join("feature.txt"), "second release\n")?;
    run_ok("git", ["add", "feature.txt"], &repo)?;
    commit_with_date(&repo, "feat: second release", "2030-06-01T00:00:00+00:00")?;
    run_ok("git", ["tag", "v1.1.0"], &repo)?;

    let previous = git_commit_date(&repo, "v1.0.0").ok_or("missing v1.0.0 commit date")?;
    let target = git_commit_date(&repo, "v1.1.0").ok_or("missing v1.1.0 commit date")?;

    // 105 noise PRs: closed without merging, created after this release's
    // window closed, so GitHub's created-desc sort puts every one of them
    // ahead of the real in-range PR below. That's more than one page.
    let mut pull_requests: Vec<Value> = (0..105)
        .map(|index| {
            paginated_pr(
                1000 + index,
                "closed without merging, unrelated to this release",
                &(target + chrono::Duration::days(30) - chrono::Duration::hours(index))
                    .to_rfc3339(),
                None,
            )
        })
        .collect();
    // The real fix, created and merged inside the release window. It sorts
    // after all 105 noise PRs (older created_at), landing on page 2.
    pull_requests.push(paginated_pr(
        2,
        "the actual shipped fix",
        &(previous + chrono::Duration::hours(1)).to_rfc3339(),
        Some(&(previous + chrono::Duration::hours(2)).to_rfc3339()),
    ));

    let fake = FakeState {
        pull_requests,
        ..Default::default()
    };
    let server = start_fake_server(fake)?;

    let output_file = repo.join("pr-changelog.md");
    let result = Command::new(current_exe())
        .args([
            "extract-prs",
            "--github-token",
            "token",
            "--repository",
            "owner/repo",
            "--release-tag",
            "v1.1.0",
            "--api-base-url",
            &server.url,
            "--repo-root",
        ])
        .arg(&repo)
        .args(["--output-file"])
        .arg(&output_file)
        .output()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).to_string().into());
    }
    let rendered = fs::read_to_string(&output_file)?;
    if !rendered.contains("the actual shipped fix") {
        return Err(format!(
            "in-range PR sitting on page 2 was dropped from the changelog:\n{rendered}"
        )
        .into());
    }

    let requests = server.state.lock().unwrap().requests.clone();
    let pr_requests = requests
        .iter()
        .filter(|request| request["path"].as_str().unwrap_or("").contains("/pulls"))
        .count();
    if pr_requests < 2 {
        return Err(format!(
            "expected closed_pull_requests to fetch more than one page, saw {pr_requests} request(s)"
        )
        .into());
    }

    Ok(json!({
        "output_file": output_file,
        "pr_fetch_requests": pr_requests,
        "checked": ["in-range PR beyond page 1 survives pagination"],
    }))
}
