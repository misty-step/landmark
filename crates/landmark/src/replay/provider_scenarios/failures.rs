use crate::*;
pub(crate) fn scenario_consumer_degraded_required_fails(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("consumer-degraded");
    init_fixture_repo(&repo, "v1.2.3")?;
    let templates_dir = env::current_dir()?.join("templates/prompts");
    let mut fake = FakeState {
        llm_status: 200,
        llm_notes: INVALID_NOTES.to_string(),
        update_status: 200,
        ..Default::default()
    };
    fake.releases.insert(
        "v1.2.3".to_string(),
        json!({"id": 1, "tag_name": "v1.2.3", "body": "body"}),
    );
    let server = start_fake_server(fake)?;
    let quality_file = repo.join("quality.txt");
    let synth = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "test-key",
            "--model",
            "test/model",
            "--api-url",
            &format!("{}/chat/completions", server.url),
            "--product-name",
            "fixture",
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file"])
        .arg(&quality_file)
        .current_dir(&repo)
        .output()?;
    if !synth.status.success() {
        return Err("degraded synthesis should still emit notes".into());
    }
    let output = temp_file("landmark-policy")?;
    let policy = Command::new(current_exe())
        .args([
            "release-policy",
            "publication",
            "--synthesis-required",
            "true",
            "--synthesis-strict",
            "false",
            "--synth-succeeded",
            "true",
            "--synth-quality",
            "degraded",
            "--github-output",
        ])
        .arg(&output)
        .output()?;
    if policy.status.success() {
        return Err("required degraded policy should fail".into());
    }
    Ok(
        json!({"quality": fs::read_to_string(quality_file)?.trim(), "outputs": parse_outputs(&output)?}),
    )
}

pub(crate) fn scenario_consumer_release_update_failure(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("consumer-update-fail");
    init_fixture_repo(&repo, "v1.2.3")?;
    let mut fake = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.to_string(),
        update_status: 500,
        ..Default::default()
    };
    fake.releases.insert(
        "v1.2.3".to_string(),
        json!({"id": 1, "tag_name": "v1.2.3", "body": "body"}),
    );
    let server = start_fake_server(fake)?;
    let notes_file = repo.join("notes.md");
    fs::write(&notes_file, VALID_NOTES)?;
    let update = Command::new(current_exe())
        .args([
            "update-release",
            "--github-token",
            "token",
            "--repository",
            "owner/repo",
            "--tag",
            "v1.2.3",
            "--notes-file",
        ])
        .arg(&notes_file)
        .args(["--api-base-url", &server.url])
        .current_dir(&repo)
        .output()?;
    if update.status.success() {
        return Err("release update should fail".into());
    }
    Ok(
        json!({"returncode": update.status.code(), "stderr": String::from_utf8_lossy(&update.stderr).trim()}),
    )
}

pub(crate) fn scenario_consumer_floating_tag_behavior(tmp_root: &Path) -> Result<Value> {
    let stable = Command::new(current_exe())
        .args(["floating-tag", "--release-tag", "v2.3.4"])
        .output()?;
    let prerelease = Command::new(current_exe())
        .args(["floating-tag", "--release-tag", "v2.3.4-beta.1"])
        .output()?;
    let stable_tag = String::from_utf8(stable.stdout)?.trim().to_string();
    let pre_tag = String::from_utf8(prerelease.stdout)?.trim().to_string();
    if stable_tag != "v2" || !pre_tag.is_empty() {
        return Err("floating tag parsing mismatch".into());
    }
    let repo = tmp_root.join("floating");
    init_fixture_repo(&repo, "v2.3.4")?;
    Ok(json!({"stable": stable_tag, "prerelease": pre_tag, "tags": git_tags(&repo)?}))
}
