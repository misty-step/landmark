use crate::*;

pub(crate) fn scenario_release_grounding_unified_path(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("release-grounding-unified-path");
    init_fixture_repo(&repo, "v1.0.0")?;
    fs::write(repo.join("release.txt"), "recover canary check-ins\n")?;
    run_ok("git", ["add", "release.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "fix: recover canary check-ins"],
        &repo,
    )?;
    run_ok("git", ["tag", "v1.1.0"], &repo)?;

    fs::write(
        repo.join("CHANGELOG.md"),
        "## [0.9.0]\n\n- feat: stale dashboard rewrite\n",
    )?;
    fs::write(
        repo.join("pr-changelog.md"),
        "- ancient unrelated dashboard rewrite (#41) by @octocat\n",
    )?;
    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Grounding Demo
  description: Release grounding replay.
model:
  policy: cheap
"#,
    )?;

    let mut fake = FakeState {
        llm_status: 200,
        update_status: 200,
        ..Default::default()
    };
    fake.llm_responses.push_back((
        200,
        json!({
            "categories": ["user-visible"],
            "significance": "medium",
            "user_visible": true,
            "breaking": false,
            "security": false,
            "migration_heavy": false,
            "reasons": ["release commit is a user-visible fix"]
        })
        .to_string(),
    ));
    fake.llm_responses.push_back((200, VALID_NOTES.to_string()));
    let server = start_fake_server(fake)?;
    let context_file = repo.join("context.json");
    let quality_file = repo.join("quality.txt");
    let templates_dir = env::current_dir()?.join("templates/prompts");
    let result = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "test-key",
            "--api-url",
            &format!("{}/chat/completions", server.url),
            "--version",
            "v1.1.0",
            "--changelog-file",
            "CHANGELOG.md",
            "--pr-changelog-file",
            "pr-changelog.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file"])
        .arg(&quality_file)
        .args(["--context-metadata-file"])
        .arg(&context_file)
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).to_string().into());
    }

    let requests = server.state.lock().unwrap().requests.clone();
    let synthesis_request = request_payloads_with_system(&requests, "release notes")?
        .into_iter()
        .next()
        .ok_or("grounding replay did not send a synthesis request")?;
    let prompt = synthesis_request["messages"][1]["content"]
        .as_str()
        .unwrap_or_default();
    if !prompt.contains("fix: recover canary check-ins") {
        return Err(format!("synthesis prompt missed release commit:\n{prompt}").into());
    }
    if prompt.contains("stale dashboard rewrite")
        || prompt.contains("ancient unrelated dashboard rewrite")
    {
        return Err(format!("synthesis prompt included stale grounding text:\n{prompt}").into());
    }
    if !prompt.contains("Grounding rule:") {
        return Err("synthesis prompt did not include the grounding rule".into());
    }

    let context: Value = serde_json::from_str(&fs::read_to_string(&context_file)?)?;
    if context["grounding"]["selected_source"] != "git-range"
        || context["grounding"]["selected_source_status"] != "commit-range"
        || context["grounding"]["warnings"]
            .as_array()
            .unwrap_or(&Vec::new())
            .is_empty()
    {
        return Err(format!(
            "context did not record git-range grounding fallback: {}",
            context["grounding"]
        )
        .into());
    }

    Ok(json!({
        "selected_source": context["grounding"]["selected_source"],
        "selected_source_status": context["grounding"]["selected_source_status"],
        "warning_count": context["grounding"]["warnings"].as_array().map(Vec::len).unwrap_or(0),
        "quality": fs::read_to_string(quality_file)?.trim(),
    }))
}
