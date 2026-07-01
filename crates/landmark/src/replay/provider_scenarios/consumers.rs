use crate::*;
pub(crate) fn consumer_success(tmp_root: &Path, name: &str, write_artifact: bool) -> Result<Value> {
    let repo = tmp_root.join(name);
    init_fixture_repo(&repo, "v1.2.3")?;
    let templates_dir = env::current_dir()?.join("templates/prompts");
    let mut fake = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.to_string(),
        update_status: 200,
        ..Default::default()
    };
    fake.releases.insert("v1.2.3".to_string(), json!({"id": 1, "tag_name": "v1.2.3", "body": "## Technical\n\n- Old", "html_url": "https://example.invalid/releases/v1.2.3"}));
    let server = start_fake_server(fake)?;
    let notes_file = repo.join("notes.md");
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
        return Err(String::from_utf8_lossy(&synth.stderr).to_string().into());
    }
    fs::write(&notes_file, &synth.stdout)?;
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
    if !update.status.success() {
        return Err(String::from_utf8_lossy(&update.stderr).to_string().into());
    }
    let artifact = if write_artifact {
        let result = Command::new(current_exe())
            .args([
                "write-artifacts",
                "--notes-file",
                notes_file.to_str().unwrap(),
                "--version",
                "v1.2.3",
                "--output-file",
                "docs/releases/{version}.md",
            ])
            .current_dir(&repo)
            .output()?;
        Some(
            json!({"returncode": result.status.code(), "stdout": String::from_utf8_lossy(&result.stdout).trim()}),
        )
    } else {
        None
    };
    let state = server.state.lock().unwrap();
    Ok(json!({
        "quality": fs::read_to_string(quality_file)?.trim(),
        "generated_notes": String::from_utf8(synth.stdout)?,
        "release_body": state.releases["v1.2.3"]["body"],
        "requests": state.requests,
        "artifact": artifact,
        "tags": git_tags(&repo)?,
    }))
}

pub(crate) fn scenario_manifest_defaults_and_overrides(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("manifest-defaults");
    init_fixture_repo(&repo, "v1.2.3")?;
    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Manifest Product
  description: Manifest description
audience: enterprise
voice: Manifest voice
changelog:
  source: release-body
model:
  policy: cheap
  primary: manifest/model
  fallbacks:
    - manifest/fallback
"#,
    )?;
    let release_body = repo.join("release-body.md");
    fs::write(
        &release_body,
        "## Manifest Technical\n\n- Manifest source\n",
    )?;
    let explicit_changelog = repo.join("CHANGELOG.md");
    fs::write(&explicit_changelog, "## [1.2.3]\n\n- Explicit source\n")?;
    let templates_dir = env::current_dir()?.join("templates/prompts");
    let fake = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.to_string(),
        update_status: 200,
        ..Default::default()
    };
    let server = start_fake_server(fake)?;
    let defaults_quality = repo.join("defaults-quality.txt");
    let defaults = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "test-key",
            "--api-url",
            &format!("{}/chat/completions", server.url),
            "--version",
            "v1.2.3",
            "--changelog-file",
        ])
        .arg(repo.join("missing-changelog.md"))
        .args(["--release-body-file"])
        .arg(&release_body)
        .args(["--templates-dir"])
        .arg(&templates_dir)
        .args(["--quality-file"])
        .arg(&defaults_quality)
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !defaults.status.success() {
        return Err(String::from_utf8_lossy(&defaults.stderr).to_string().into());
    }

    let override_quality = repo.join("override-quality.txt");
    let overrides = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "test-key",
            "--model",
            "explicit/model",
            "--api-url",
            &format!("{}/chat/completions", server.url),
            "--product-name",
            "Explicit Product",
            "--product-description",
            "Explicit description",
            "--voice-guide",
            "Explicit voice",
            "--audience",
            "developer",
            "--changelog-source",
            "changelog",
            "--version",
            "v1.2.3",
            "--changelog-file",
        ])
        .arg(&explicit_changelog)
        .args(["--templates-dir"])
        .arg(&templates_dir)
        .args(["--quality-file"])
        .arg(&override_quality)
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !overrides.status.success() {
        return Err(String::from_utf8_lossy(&overrides.stderr)
            .to_string()
            .into());
    }

    let requests = server.state.lock().unwrap().requests.clone();
    let synthesis_requests = request_payloads_with_system(&requests, "release notes")?;
    if synthesis_requests.len() != 2 {
        return Err(format!(
            "expected two synthesis requests after classifier preflights, got {}",
            synthesis_requests.len()
        )
        .into());
    }
    let default_request = &synthesis_requests[0];
    let override_request = &synthesis_requests[1];
    let default_prompt = default_request["messages"][1]["content"]
        .as_str()
        .unwrap_or_default();
    let override_prompt = override_request["messages"][1]["content"]
        .as_str()
        .unwrap_or_default();
    if default_request["model"] != "manifest/model"
        || !default_prompt.contains("Manifest Product")
        || !default_prompt.contains("Manifest description")
        || !default_prompt.contains("Manifest voice")
        || !default_prompt.contains("Manifest source")
    {
        return Err("manifest defaults did not reach synthesis prompt".into());
    }
    if override_request["model"] != "explicit/model"
        || !override_prompt.contains("Explicit Product")
        || !override_prompt.contains("Explicit description")
        || !override_prompt.contains("Explicit voice")
        || !override_prompt.contains("Explicit source")
        || override_prompt.contains("Manifest source")
    {
        return Err("explicit synthesis inputs did not override manifest defaults".into());
    }

    Ok(json!({
        "default_model": default_request["model"],
        "override_model": override_request["model"],
        "default_quality": fs::read_to_string(defaults_quality)?.trim(),
        "override_quality": fs::read_to_string(override_quality)?.trim(),
        "checked": [
            ".landmark.yml",
            "manifest model/product/audience/voice/changelog defaults",
            "explicit CLI override precedence"
        ],
    }))
}
