use crate::*;
pub(crate) fn scenario_synthesis_cost_policy(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("synthesis-cost-policy");
    init_fixture_repo(&repo, "v1.2.3")?;
    let templates_dir = env::current_dir()?.join("templates/prompts");

    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Cost Policy Demo
  description: Demo release automation.
model:
  policy: balanced
"#,
    )?;
    fs::write(
        repo.join("CHANGELOG.md"),
        "## [1.2.3]\n\n- docs: update README.md\n",
    )?;
    let dry_run = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "",
            "--api-url",
            "http://127.0.0.1:1/chat/completions",
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file", "quality-dry.txt", "--dry-run-cost"])
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !dry_run.status.success() {
        return Err(String::from_utf8_lossy(&dry_run.stderr).to_string().into());
    }
    let dry_context: Value = serde_json::from_slice(&dry_run.stdout)?;
    if dry_context["cost"]["skip"] != true
        || dry_context["cost"]["model_tier"] != "off"
        || dry_context["decision"]["action"] != "skipped"
        || dry_context["deterministic"]["docs"]
            .as_array()
            .unwrap()
            .is_empty()
        || dry_context["deterministic"]["artifacts"]["internal_technical_changelog"]
            != "landmark.internal-technical-changelog.v1"
        || dry_context["classification"]["categories"]
            .as_array()
            .unwrap()
            .iter()
            .all(|category| category != "docs-only")
    {
        return Err("dry-run cost policy did not skip docs-only release".into());
    }

    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Cost Policy Demo
  description: Demo release automation.
model:
  policy: cheap
"#,
    )?;
    fs::write(
        repo.join("CHANGELOG.md"),
        "## [1.2.3]\n\n- feat(cli): add a fleet command\n",
    )?;
    let cheap_context_file = repo.join("cheap-context.json");
    let cheap_attempts = repo.join("cheap-attempts.json");
    let cheap_quality = repo.join("cheap-quality.txt");
    let fake = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.to_string(),
        update_status: 200,
        ..Default::default()
    };
    let server = start_fake_server(fake)?;
    let cheap = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "test-key",
            "--api-url",
            &format!("{}/chat/completions", server.url),
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file"])
        .arg(&cheap_quality)
        .args(["--attempts-file"])
        .arg(&cheap_attempts)
        .args(["--context-metadata-file"])
        .arg(&cheap_context_file)
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !cheap.status.success() {
        return Err(String::from_utf8_lossy(&cheap.stderr).to_string().into());
    }
    let cheap_requests = server.state.lock().unwrap().requests.clone();
    let cheap_request = request_payloads_with_system(&cheap_requests, "release notes")?
        .into_iter()
        .next()
        .ok_or("cheap policy did not send a synthesis request")?;
    let cheap_context: Value = serde_json::from_str(&fs::read_to_string(&cheap_context_file)?)?;
    if cheap_request["model"] != "openai/gpt-4o-mini"
        || cheap_context["cost"]["model_tier"] != "cheap"
        || cheap_context["decision"]["action"] != "used"
        || cheap_context["deterministic"]["manifest"]["present"] != true
        || cheap_context["sources"]
            .as_array()
            .unwrap()
            .iter()
            .all(|source| source["name"] != "technical_changelog")
    {
        return Err("cheap policy did not use cheap model with context metadata".into());
    }

    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Cost Policy Demo
  description: Demo release automation.
model:
  policy: balanced
  primary: primary/model
  fallbacks:
    - fallback/model
"#,
    )?;
    let mut fallback_fake = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.to_string(),
        update_status: 200,
        ..Default::default()
    };
    fallback_fake.llm_responses.push_back((
        200,
        json!({
            "categories": ["user-visible"],
            "significance": "medium",
            "user_visible": true,
            "breaking": false,
            "security": false,
            "migration_heavy": false,
            "reasons": ["fake classifier preserved feature release"]
        })
        .to_string(),
    ));
    for _ in 0..HttpPolicy::default().attempts {
        fallback_fake.llm_responses.push_back((500, String::new()));
    }
    fallback_fake
        .llm_responses
        .push_back((200, VALID_NOTES.to_string()));
    let fallback_server = start_fake_server(fallback_fake)?;
    let fallback_attempts = repo.join("fallback-attempts.json");
    let fallback = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "test-key",
            "--api-url",
            &format!("{}/chat/completions", fallback_server.url),
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file", "fallback-quality.txt"])
        .args(["--attempts-file"])
        .arg(&fallback_attempts)
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !fallback.status.success() {
        return Err(String::from_utf8_lossy(&fallback.stderr).to_string().into());
    }
    let attempts: Value = serde_json::from_str(&fs::read_to_string(&fallback_attempts)?)?;
    if attempts.as_array().unwrap().len() != 2
        || attempts[0]["succeeded"] != false
        || attempts[1]["model"] != "fallback/model"
        || attempts[1]["succeeded"] != true
    {
        return Err("fallback attempt sequence was not recorded".into());
    }
    let fallback_requests = fallback_server.state.lock().unwrap().requests.len();
    if fallback_requests != HttpPolicy::default().attempts + 2 {
        return Err(format!(
            "fallback replay expected classifier request, primary HTTP retries, and fallback request, got {fallback_requests}"
        )
        .into());
    }

    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Cost Policy Demo
  description: Demo release automation.
model:
  policy: balanced
"#,
    )?;
    fs::write(
        repo.join("CHANGELOG.md"),
        "## [1.2.3]\n\n- feat(api)!: rotate security-sensitive release token configuration\n\nBREAKING CHANGE: tokens moved to a new manifest field.\n",
    )?;
    let rich = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "",
            "--api-url",
            "http://127.0.0.1:1/chat/completions",
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file", "rich-quality.txt", "--dry-run-cost"])
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !rich.status.success() {
        return Err(String::from_utf8_lossy(&rich.stderr).to_string().into());
    }
    let rich_context: Value = serde_json::from_slice(&rich.stdout)?;
    if rich_context["cost"]["model_tier"] != "rich"
        || rich_context["decision"]["action"] != "escalated"
        || rich_context["classification"]["security"] != true
        || rich_context["classification"]["breaking"] != true
    {
        return Err("balanced policy did not escalate high-significance release".into());
    }

    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Cost Policy Demo
  description: Demo release automation.
model:
  policy: rich
"#,
    )?;
    let direct_rich = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "",
            "--api-url",
            "http://127.0.0.1:1/chat/completions",
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args([
            "--quality-file",
            "direct-rich-quality.txt",
            "--dry-run-cost",
        ])
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !direct_rich.status.success() {
        return Err(String::from_utf8_lossy(&direct_rich.stderr)
            .to_string()
            .into());
    }
    let direct_rich_context: Value = serde_json::from_slice(&direct_rich.stdout)?;
    if direct_rich_context["cost"]["model_tier"] != "rich"
        || direct_rich_context["decision"]["action"] != "used"
    {
        return Err("direct rich policy should use, not escalate, rich synthesis".into());
    }

    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Cost Policy Demo
  description: Demo release automation.
model:
  policy: off
"#,
    )?;
    let off_attempts = repo.join("off-attempts.json");
    let off = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "",
            "--api-url",
            "http://127.0.0.1:1/chat/completions",
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file", "off-quality.txt"])
        .args(["--attempts-file"])
        .arg(&off_attempts)
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if !off.status.success() {
        return Err(String::from_utf8_lossy(&off.stderr).to_string().into());
    }
    let off_attempts_json: Value = serde_json::from_str(&fs::read_to_string(&off_attempts)?)?;
    if off_attempts_json[0]["quality"] != "skipped"
        || off_attempts_json[0]["decision"]["action"] != "skipped"
    {
        return Err("off policy did not explain skipped synthesis".into());
    }

    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Cost Policy Demo
  description: Demo release automation.
model:
  policy: cheap
  primary: primary/model
"#,
    )?;
    let provider_failure_server = start_fake_server(FakeState {
        llm_status: 500,
        llm_notes: String::new(),
        update_status: 200,
        ..Default::default()
    })?;
    let provider_failure_attempts = repo.join("provider-failure-attempts.json");
    let provider_failure = Command::new(current_exe())
        .args([
            "synthesize",
            "--api-key",
            "test-key",
            "--api-url",
            &format!("{}/chat/completions", provider_failure_server.url),
            "--version",
            "v1.2.3",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file", "provider-failure-quality.txt"])
        .args(["--attempts-file"])
        .arg(&provider_failure_attempts)
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;
    if provider_failure.status.success() {
        return Err("provider failure synthesis should return a failed exit".into());
    }
    let provider_failure_json: Value =
        serde_json::from_str(&fs::read_to_string(&provider_failure_attempts)?)?;
    if provider_failure_json[0]["quality"] != "failed"
        || provider_failure_json[0]["decision"]["action"] != "used"
        || !provider_failure_json[0]["message"]
            .as_str()
            .unwrap_or("")
            .contains("failed")
    {
        return Err("provider failure path did not record failed attempt metadata".into());
    }

    Ok(json!({
        "dry_run_skip": dry_context["cost"],
        "cheap_model": cheap_request["model"],
        "fallback_attempts": attempts,
        "rich_cost": rich_context["cost"],
        "direct_rich_decision": direct_rich_context["decision"],
        "off_policy": off_attempts_json,
        "provider_failure": provider_failure_json,
    }))
}

pub(crate) fn request_payload(requests: &[Value], index: usize) -> Result<Value> {
    let body = requests
        .get(index)
        .and_then(|request| request["body"].as_str())
        .ok_or_else(|| format!("missing fake LLM request {index}"))?;
    Ok(serde_json::from_str(body)?)
}

pub(crate) fn request_payloads_with_system(requests: &[Value], needle: &str) -> Result<Vec<Value>> {
    let mut payloads = Vec::new();
    for index in 0..requests.len() {
        let Ok(payload) = request_payload(requests, index) else {
            continue;
        };
        let system = payload["messages"][0]["content"]
            .as_str()
            .unwrap_or_default();
        if system.contains(needle) {
            payloads.push(payload);
        }
    }
    Ok(payloads)
}
