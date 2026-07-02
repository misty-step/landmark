use crate::*;
pub(crate) fn scenario_self_release_pr_path(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("self-release-pr");
    init_self_release_fixture(&repo)?;
    let prepare_output = temp_file("landmark-self-release-prepare")?;
    let prepare = Command::new(current_exe())
        .args([
            "prepare-self-release",
            "--repo-root",
            repo.to_str().unwrap(),
            "--repository",
            "owner/repo",
            "--release-branch",
            "landmark/self-release",
            "--github-output",
        ])
        .arg(&prepare_output)
        .output()?;
    if !prepare.status.success() {
        return Err(String::from_utf8_lossy(&prepare.stderr).to_string().into());
    }
    let prepare_plan: Value = serde_json::from_slice(&prepare.stdout)?;
    let prepare_outputs = parse_outputs(&prepare_output)?;
    if prepare_outputs.get("released").map(String::as_str) != Some("true") {
        return Err("prepare-self-release did not mark a release PR ready".into());
    }
    assert_file_contains(&repo.join("package.json"), r#""version": "1.1.0""#)?;
    assert_file_contains(
        &repo.join("crates/landmark/Cargo.toml"),
        r#"version = "1.1.0""#,
    )?;
    assert_file_contains(&repo.join("Cargo.lock"), r#"version = "1.1.0""#)?;
    assert_file_contains(&repo.join("CHANGELOG.md"), "# [1.1.0]")?;
    if repo.join("dist").exists() {
        return Err("prepare-self-release must not recreate dist/".into());
    }
    let changed_files = prepare_plan["changed_files"]
        .as_array()
        .ok_or("prepare plan missing changed_files")?;
    for unexpected in ["dist/landmark", "dist/landmark.sha256"] {
        if changed_files
            .iter()
            .any(|file| file.as_str() == Some(unexpected))
        {
            return Err(format!("prepare plan must not list {unexpected}").into());
        }
    }

    run_ok("git", ["add", "."], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "chore(release): 1.1.0"],
        &repo,
    )?;
    let target_sha = run_ok("git", ["rev-parse", "HEAD"], &repo)?
        .trim()
        .to_string();
    let mut fake = FakeState::default();
    fake.releases.insert(
        "v1.0.0".to_string(),
        json!({"id": 1, "tag_name": "v1.0.0", "body": "old", "html_url": "https://example.invalid/releases/v1.0.0"}),
    );
    let server = start_fake_server(fake)?;
    let publish_output = temp_file("landmark-self-release-publish")?;
    let publish = Command::new(current_exe())
        .args([
            "publish-self-release",
            "--repo-root",
            repo.to_str().unwrap(),
            "--github-token",
            "token",
            "--repository",
            "owner/repo",
            "--target-sha",
            &target_sha,
            "--api-base-url",
            &server.url,
            "--github-output",
        ])
        .arg(&publish_output)
        .output()?;
    if !publish.status.success() {
        return Err(String::from_utf8_lossy(&publish.stderr).to_string().into());
    }
    let publish_outputs = parse_outputs(&publish_output)?;
    if publish_outputs.get("published").map(String::as_str) != Some("true") {
        return Err("publish-self-release did not publish the landed release".into());
    }
    let state = server.state.lock().unwrap();
    let created = state
        .releases
        .get("v1.1.0")
        .ok_or("fake GitHub release was not created")?;
    Ok(json!({
        "prepare": prepare_outputs,
        "publish": publish_outputs,
        "release": created,
        "requests": state.requests,
        "target_sha": target_sha,
        "changed_files": changed_files,
    }))
}

pub(crate) fn assert_file_contains(path: &Path, needle: &str) -> Result<()> {
    let text = fs::read_to_string(path)?;
    if text.contains(needle) {
        Ok(())
    } else {
        Err(format!("{} did not contain {needle}", path.display()).into())
    }
}

pub(crate) fn init_self_release_fixture(path: &Path) -> Result<()> {
    fs::create_dir_all(path.join("crates/landmark/src"))?;
    run_ok("git", ["init", "-q"], path)?;
    run_ok("git", ["config", "user.name", "Landmark Replay"], path)?;
    run_ok(
        "git",
        ["config", "user.email", "replay@example.invalid"],
        path,
    )?;
    fs::write(path.join("README.md"), "# Fixture\n")?;
    fs::write(
        path.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/landmark\"]\nresolver = \"3\"\n",
    )?;
    fs::write(
        path.join("package.json"),
        serde_json::to_string_pretty(&json!({"name": "landmark", "version": "1.0.0"}))? + "\n",
    )?;
    fs::write(
        path.join("crates/landmark/Cargo.toml"),
        "[package]\nname = \"landmark\"\nversion = \"1.0.0\"\nedition = \"2024\"\n",
    )?;
    fs::write(
        path.join("crates/landmark/src/main.rs"),
        "fn main() { println!(\"landmark fixture {}\", env!(\"CARGO_PKG_VERSION\")); }\n",
    )?;
    fs::write(
        path.join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\nversion = 4\n\n[[package]]\nname = \"landmark\"\nversion = \"1.0.0\"\n",
    )?;
    fs::write(
        path.join("CHANGELOG.md"),
        "## [1.0.0](https://github.com/owner/repo/releases/tag/v1.0.0) (2026-01-01)\n\n### Features\n\n* seed\n",
    )?;
    run_ok("git", ["add", "."], path)?;
    run_ok("git", ["commit", "-q", "-m", "chore: seed release"], path)?;
    run_ok("git", ["tag", "v1.0.0"], path)?;
    fs::write(
        path.join("README.md"),
        "# Fixture\n\nRelease PR protected branch flow.\n",
    )?;
    run_ok("git", ["add", "README.md"], path)?;
    run_ok(
        "git",
        [
            "commit",
            "-q",
            "-m",
            "feat(release): add protected branch self release",
        ],
        path,
    )?;
    Ok(())
}
