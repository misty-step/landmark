use crate::*;
pub(crate) fn scenario_http_resilience_policy(_: &Path) -> Result<Value> {
    let token = "ghp_123456789abcdef";
    let webhook_url = "https://hooks.slack.invalid/services/T000/B000/secret";
    let invocation = build_curl_invocation(
        "POST",
        webhook_url,
        Some(token),
        Some(&json!({"ok": true})),
        HttpPolicy::default(),
    );
    let argv = invocation.args.join(" ");
    if argv.contains(token) || argv.contains(webhook_url) || argv.contains("Authorization") {
        return Err("curl argv contains secret-bearing request data".into());
    }
    if !invocation.config.contains(token) || !invocation.config.contains(webhook_url) {
        return Err("curl stdin config missing request secret data".into());
    }
    let redacted = redact_secret_values(
        &format!("token={token} slack={webhook_url} short=abc123"),
        [
            token.to_string(),
            webhook_url.to_string(),
            "abc123".to_string(),
        ],
    );
    if redacted.contains(token) || redacted.contains(webhook_url) {
        return Err("configured secret redaction left a secret in evidence text".into());
    }
    if !redacted.contains("short=abc123") {
        return Err("configured secret redaction removed a short non-secret value".into());
    }

    let mut retry_429 = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.into(),
        ..FakeState::default()
    };
    retry_429.llm_responses.push_back((429, String::new()));
    retry_429.llm_responses.push_back((200, VALID_NOTES.into()));
    let retry_429_server = start_fake_server(retry_429)?;
    let retry_429_response = curl_json_with_policy(
        "POST",
        &format!("{}/chat/completions", retry_429_server.url),
        Some(token),
        Some(&json!({"messages": []})),
        HttpPolicy {
            retry_delay_ms: 1,
            ..HttpPolicy::default()
        },
    )?;
    if retry_429_response.status != 200 {
        return Err("429 retry did not recover to HTTP 200".into());
    }
    let retry_429_requests = retry_429_server.state.lock().unwrap().requests.len();
    if retry_429_requests != 2 {
        return Err(format!("429 retry expected 2 requests, got {retry_429_requests}").into());
    }

    let mut retry_500 = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.into(),
        ..FakeState::default()
    };
    retry_500.llm_responses.push_back((500, String::new()));
    retry_500.llm_responses.push_back((200, VALID_NOTES.into()));
    let retry_500_server = start_fake_server(retry_500)?;
    let retry_500_response = curl_json_with_policy(
        "POST",
        &format!("{}/chat/completions", retry_500_server.url),
        Some(token),
        Some(&json!({"messages": []})),
        HttpPolicy {
            retry_delay_ms: 1,
            ..HttpPolicy::default()
        },
    )?;
    if retry_500_response.status != 200 {
        return Err("5xx retry did not recover to HTTP 200".into());
    }

    let slow_url = start_slow_http_server(Duration::from_millis(1500))?;
    let slow = curl_json_with_policy(
        "GET",
        &slow_url,
        None,
        None,
        HttpPolicy {
            connect_timeout_seconds: 1,
            max_time_seconds: 1,
            attempts: 1,
            retry_delay_ms: 1,
        },
    );
    if slow.is_ok() {
        return Err("slow provider request unexpectedly succeeded".into());
    }
    let slow_error = redact_known_secrets(&slow.unwrap_err().to_string());
    if slow_error.contains(token) {
        return Err("timeout error leaked token-like content".into());
    }

    Ok(json!({
        "argv_secret_free": true,
        "retry_429_requests": retry_429_requests,
        "retry_500_status": retry_500_response.status,
        "configured_secret_redaction": true,
        "slow_timeout": true
    }))
}

pub(crate) fn scenario_action_side_effect_coverage(_: &Path) -> Result<Value> {
    let action = fs::read_to_string("action.yml")?;
    let invoked = action_landmark_subcommands(&action);
    let coverage = action_subcommand_replay_coverage();
    let scenarios = scenario_map();
    let mut missing = Vec::new();
    for command in &invoked {
        match coverage.get(command.as_str()) {
            Some(names) if names.iter().all(|name| scenarios.contains_key(*name)) => {}
            Some(_) => missing.push(format!("{command}: coverage references unknown scenario")),
            None => missing.push(format!("{command}: no replay coverage mapping")),
        }
    }
    if !missing.is_empty() {
        return Err(missing.join("\n").into());
    }
    Ok(json!({
        "covered_commands": invoked,
        "coverage": coverage
    }))
}

pub(crate) fn action_landmark_subcommands(action: &str) -> BTreeSet<String> {
    let re = Regex::new(r#"dist/landmark"\s+([a-z][a-z-]*)(?:\s+([a-z][a-z-]*))?"#).unwrap();
    re.captures_iter(action)
        .map(|caps| {
            let command = caps.get(1).unwrap().as_str();
            let nested = caps.get(2).map(|value| value.as_str()).unwrap_or("");
            if command == "fleet" || command == "release-policy" {
                format!("{command} {nested}")
            } else {
                command.to_string()
            }
        })
        .collect()
}

pub(crate) fn action_subcommand_replay_coverage() -> BTreeMap<&'static str, Vec<&'static str>> {
    BTreeMap::from([
        (
            "manifest-defaults",
            vec![
                "manifest_defaults_and_overrides",
                "action_manifest_defaults_precedence",
            ],
        ),
        ("healthcheck", vec!["http_resilience_policy"]),
        ("preflight-tags", vec!["action_side_effect_coverage"]),
        ("fetch-release-body", vec!["consumer_full_mode_success"]),
        ("extract-prs", vec!["consumer_full_mode_success"]),
        (
            "synthesize",
            vec!["synthesis_cost_policy", "consumer_synthesis_only_success"],
        ),
        (
            "release-policy publication",
            vec![
                "publication_degraded_optional",
                "publication_degraded_required",
            ],
        ),
        (
            "release-policy summary",
            vec![
                "summary_artifact_failed",
                "summary_release_update_failed",
                "summary_rss_failed",
            ],
        ),
        (
            "run",
            vec![
                "local_provider_run",
                "github_provider_run",
                "provider_run_parity",
            ],
        ),
        ("notify-webhook", vec!["http_resilience_policy"]),
        ("notify-slack", vec!["http_resilience_policy"]),
        ("floating-tag", vec!["consumer_floating_tag_behavior"]),
        (
            "close-resolved-failures",
            vec!["consumer_release_update_failure"],
        ),
        (
            "report-synthesis-failure",
            vec!["consumer_degraded_required_fails"],
        ),
    ])
}

pub(crate) fn scenario_agent_native_contracts(tmp_root: &Path) -> Result<Value> {
    let repo_root = env::current_dir()?;
    let schema_errors = validate_agent_native_contracts(&repo_root)?;
    if !schema_errors.is_empty() {
        return Err(schema_errors.join("\n").into());
    }

    let describe = Command::new(current_exe())
        .args(["describe", "--json"])
        .output()?;
    if !describe.status.success() {
        return Err(format!(
            "describe --json failed: {}",
            String::from_utf8_lossy(&describe.stderr)
        )
        .into());
    }
    let describe_json: Value = serde_json::from_slice(&describe.stdout)?;
    assert_json_eq(
        &describe_json,
        "/schema_version",
        "landmark.describe.v1",
        "describe schema version",
    )?;
    let schema_paths: BTreeSet<String> = describe_json["schemas"]
        .as_array()
        .ok_or("describe schemas must be an array")?
        .iter()
        .filter_map(|schema| schema["path"].as_str().map(str::to_string))
        .collect();
    for descriptor in schema_descriptors() {
        if !schema_paths.contains(descriptor.path) {
            return Err(format!("describe missing schema {}", descriptor.path).into());
        }
    }
    let contracts = describe_json["contracts"]
        .as_array()
        .ok_or("describe contracts must be an array")?;
    let run_contract = contracts
        .iter()
        .find(|contract| contract["command"] == "run")
        .ok_or("describe missing run contract")?;
    assert_json_eq(
        run_contract,
        "/preview",
        "--dry-run computes evidence without writing artifacts or mutating releases",
        "run preview",
    )?;

    let invalid = Command::new(current_exe())
        .args([
            "--error-format",
            "json",
            "run",
            "--provider",
            "unsupported",
            "--dry-run",
        ])
        .output()?;
    if invalid.status.success() {
        return Err("invalid provider unexpectedly succeeded".into());
    }
    let failure: Value = serde_json::from_slice(&invalid.stderr)?;
    assert_json_eq(
        &failure,
        "/error/code",
        "invalid_input",
        "invalid provider error code",
    )?;
    assert_json_eq(
        &failure,
        "/error/stage",
        "configuration",
        "invalid provider error stage",
    )?;
    if failure["error"]["context"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("ghp_")
    {
        return Err("failure context leaked token-like content".into());
    }

    let repo = tmp_root.join("agent-native-run");
    init_fixture_repo(&repo, "v1.0.0")?;
    fs::write(repo.join("feature.txt"), "feature\n")?;
    run_ok("git", ["add", "feature.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat: agent native run"],
        &repo,
    )?;
    let dry_run = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "local",
            "--repo-root",
            repo.to_str().unwrap(),
            "--dry-run",
            "--output-file",
            "docs/releases/{version}.md",
            "--output-json",
            "docs/releases/releases.json",
            "--rss-feed-file",
            "docs/releases/feed.xml",
        ])
        .output()?;
    if !dry_run.status.success() {
        return Err(format!(
            "run --dry-run failed: {}",
            String::from_utf8_lossy(&dry_run.stderr)
        )
        .into());
    }
    let evidence: Value = serde_json::from_slice(&dry_run.stdout)?;
    assert_json_eq(&evidence, "/provider", "local", "dry-run provider")?;
    release_kit::assert_contract(&evidence["release_kit"], "dry-run release kit")?;
    assert_json_eq(
        &evidence,
        "/artifacts/release_kit_schema",
        "landmark.release-kit.v1",
        "dry-run release kit schema",
    )?;
    assert_json_eq(
        &evidence,
        "/publication/status",
        "dry-run; release-body publication previewed but not mutated",
        "dry-run publication status",
    )?;
    if repo.join("docs/releases/releases.json").exists() {
        return Err("run --dry-run wrote JSON artifact".into());
    }
    let dry_release_kit_path = evidence["artifacts"]["release_kit"]
        .as_str()
        .ok_or("dry-run missing planned release-kit path")?;
    if Path::new(dry_release_kit_path).exists() {
        return Err("run --dry-run wrote release-kit artifact".into());
    }

    let backfill = Command::new(current_exe())
        .args([
            "backfill",
            "--repo-root",
            repo.to_str().unwrap(),
            "--dry-run",
            "--max-tags",
            "1",
            "--output-json",
            "docs/releases/releases.json",
            "--rss-feed-file",
            "docs/releases/feed.xml",
        ])
        .output()?;
    if !backfill.status.success() {
        return Err(format!(
            "backfill --dry-run failed: {}",
            String::from_utf8_lossy(&backfill.stderr)
        )
        .into());
    }
    let backfill_manifest: Value = serde_json::from_slice(&backfill.stdout)?;
    if backfill_manifest["dry_run"] != json!(true) {
        return Err("backfill --dry-run did not emit dry_run=true".into());
    }

    let fleet_fixture_path = tmp_root.join("agent-native-fleet.json");
    let fleet_scan_path = tmp_root.join("agent-native-fleet-scan.json");
    let fleet_plan_dir = tmp_root.join("agent-native-fleet-plan");
    let fleet_pr_dir = fleet_plan_dir.join("prs");
    let fleet_fixture = FleetScan {
        generated_at: "2026-06-15T00:00:00Z".into(),
        owners: vec!["phrazzld".into()],
        repositories: vec![fleet_fixture_repo(
            "phrazzld/semantic-app",
            "semantic-release",
            ("application", "github-release+semantic-release"),
            (false, false),
            "unprotected-or-unavailable",
            &[],
            &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
        )],
        warnings: Vec::new(),
    };
    fs::write(
        &fleet_fixture_path,
        serde_json::to_string_pretty(&fleet_fixture)? + "\n",
    )?;
    let fleet_scan = Command::new(current_exe())
        .args([
            "fleet",
            "scan",
            "--fixture",
            fleet_fixture_path.to_str().unwrap(),
            "--output",
            fleet_scan_path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()?;
    if !fleet_scan.status.success() {
        return Err(format!(
            "fleet scan --format json failed: {}",
            String::from_utf8_lossy(&fleet_scan.stderr)
        )
        .into());
    }
    let fleet_scan_json: Value = serde_json::from_slice(&fleet_scan.stdout)?;
    if fleet_scan_json["repositories"].as_array().map(Vec::len) != Some(1) {
        return Err("fleet scan JSON stdout did not include one repository".into());
    }
    let fleet_plan = Command::new(current_exe())
        .args([
            "fleet",
            "plan",
            "--input",
            fleet_scan_path.to_str().unwrap(),
            "--output-dir",
            fleet_plan_dir.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()?;
    if !fleet_plan.status.success() {
        return Err(format!(
            "fleet plan --format json failed: {}",
            String::from_utf8_lossy(&fleet_plan.stderr)
        )
        .into());
    }
    let fleet_plan_json: Value = serde_json::from_slice(&fleet_plan.stdout)?;
    if fleet_plan_json["repositories"].as_array().map(Vec::len) != Some(1) {
        return Err("fleet plan JSON stdout did not include one repository".into());
    }
    let fleet_prs = Command::new(current_exe())
        .args([
            "fleet",
            "open-prs",
            "--plan-dir",
            fleet_plan_dir.to_str().unwrap(),
            "--output-dir",
            fleet_pr_dir.to_str().unwrap(),
            "--dry-run",
            "--format",
            "json",
        ])
        .output()?;
    if !fleet_prs.status.success() {
        return Err(format!(
            "fleet open-prs --format json failed: {}",
            String::from_utf8_lossy(&fleet_prs.stderr)
        )
        .into());
    }
    let fleet_prs_json: Value = serde_json::from_slice(&fleet_prs.stdout)?;
    if fleet_prs_json["dry_run"] != json!(true) {
        return Err("fleet open-prs JSON stdout did not include dry_run=true".into());
    }

    Ok(json!({
        "describe_schemas": schema_paths.len(),
        "backfill_dry_run": backfill_manifest["dry_run"],
        "json_error_code": failure["error"]["code"],
        "dry_run_release_tag": evidence["release_tag"],
        "dry_run_artifacts": evidence["artifacts"],
        "dry_run_release_kit": evidence["release_kit"],
        "fleet_json_paths": {
            "scan_repositories": fleet_scan_json["repositories"].as_array().map(Vec::len).unwrap_or(0),
            "plan_repositories": fleet_plan_json["repositories"].as_array().map(Vec::len).unwrap_or(0),
            "prs_dry_run": fleet_prs_json["dry_run"]
        }
    }))
}

pub(crate) fn assert_json_eq(
    value: &Value,
    pointer: &str,
    expected: &str,
    label: &str,
) -> Result<()> {
    let actual = value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{label} missing JSON string at {pointer}"))?;
    if actual != expected {
        return Err(format!("{label} expected `{expected}`, got `{actual}`").into());
    }
    Ok(())
}

pub(crate) fn scenario_backfill_release_history(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("backfill-release-history");
    init_fixture_repo(&repo, "v1.0.0")?;
    fs::write(
        repo.join("CHANGELOG.md"),
        "## [1.4.0]\n\n- feat(pkg-a): package artifact history\n\n## [1.3.0]\n\n- feat: first duplicate section\n\n## [1.3.0]\n\n- fix: second duplicate section\n\n## [1.1.0]\n\n- feat: historical changelog entry\n\n## [1.0.0]\n\n- feat: initial release\n",
    )?;
    run_ok("git", ["add", "CHANGELOG.md"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "docs: expand historical changelog"],
        &repo,
    )?;
    run_ok("git", ["tag", "v1.1.0"], &repo)?;
    fs::write(repo.join("beta.txt"), "beta\n")?;
    run_ok("git", ["add", "beta.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat: prerelease beta"],
        &repo,
    )?;
    run_ok("git", ["tag", "v1.2.0-beta.1"], &repo)?;
    fs::write(repo.join("managed.txt"), "managed\n")?;
    run_ok("git", ["add", "managed.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat: already managed release"],
        &repo,
    )?;
    run_ok("git", ["tag", "v1.2.0"], &repo)?;
    fs::write(repo.join("duplicate.txt"), "duplicate\n")?;
    run_ok("git", ["add", "duplicate.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat: duplicate changelog release"],
        &repo,
    )?;
    run_ok("git", ["tag", "v1.3.0"], &repo)?;
    fs::create_dir_all(repo.join("packages/pkg-a"))?;
    fs::write(repo.join("packages/pkg-a/README.md"), "# pkg-a\n")?;
    run_ok("git", ["add", "packages/pkg-a/README.md"], &repo)?;
    run_ok(
        "git",
        [
            "commit",
            "-q",
            "-m",
            "feat(pkg-a): monorepo package release",
        ],
        &repo,
    )?;
    run_ok("git", ["tag", "pkg-a@v1.4.0"], &repo)?;
    fs::write(repo.join("existing-body.txt"), "existing body\n")?;
    run_ok("git", ["add", "existing-body.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat: existing release body source"],
        &repo,
    )?;
    run_ok("git", ["tag", "v1.5.0"], &repo)?;

    let mut fake = FakeState {
        update_status: 200,
        ..Default::default()
    };
    fake.releases.insert("v1.2.0".to_string(), json!({"id": 2, "tag_name": "v1.2.0", "body": "## What's New\n\n- Managed already\n\n## Technical\n\n- Existing", "html_url": "https://example.invalid/releases/v1.2.0"}));
    fake.releases.insert("v1.3.0".to_string(), json!({"id": 3, "tag_name": "v1.3.0", "body": "", "html_url": "https://example.invalid/releases/v1.3.0"}));
    fake.releases.insert("pkg-a@v1.4.0".to_string(), json!({"id": 4, "tag_name": "pkg-a@v1.4.0", "body": "", "html_url": "https://example.invalid/releases/pkg-a@v1.4.0"}));
    fake.releases.insert("v1.5.0".to_string(), json!({"id": 5, "tag_name": "v1.5.0", "body": "## Technical\n\n- Existing release-body source", "html_url": "https://example.invalid/releases/v1.5.0"}));
    let server = start_fake_server(fake)?;

    let dry_run = Command::new(current_exe())
        .args([
            "backfill",
            "--repo-root",
            repo.to_str().unwrap(),
            "--since",
            "v1.0.0",
            "--dry-run",
            "--repository",
            "owner/repo",
            "--github-token",
            "token",
            "--api-base-url",
            &server.url,
        ])
        .output()?;
    if !dry_run.status.success() {
        return Err(String::from_utf8_lossy(&dry_run.stderr).to_string().into());
    }
    let dry_manifest: Value = serde_json::from_slice(&dry_run.stdout)?;
    let skipped = dry_manifest["skipped_tags"].as_array().unwrap();
    if !skipped.iter().any(|entry| entry["tag"] == "v1.2.0-beta.1")
        || !skipped.iter().any(|entry| entry["tag"] == "v1.2.0")
    {
        return Err("dry-run did not report prerelease and already-managed skips".into());
    }
    if !dry_manifest["processed_tags"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["tag"] == "pkg-a@v1.4.0")
    {
        return Err("dry-run did not include monorepo package tag".into());
    }
    if dry_manifest["estimated_cost"]["llm_calls"] != 0 {
        return Err("backfill dry-run should not schedule LLM calls".into());
    }
    if !dry_manifest["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| {
            entry["tag"] == "v1.1.0"
                && entry["markdown"]
                    .as_str()
                    .unwrap_or("")
                    .ends_with("docs/releases/v1.1.0.md")
        })
    {
        return Err("dry-run did not include artifact path plan".into());
    }
    if !dry_manifest["processed_tags"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["tag"] == "v1.1.0" && entry["release_status"] == "missing")
    {
        return Err("dry-run did not preserve missing release status".into());
    }

    let artifact_run = Command::new(current_exe())
        .args([
            "backfill",
            "--repo-root",
            repo.to_str().unwrap(),
            "--since",
            "v1.0.0",
            "--mode",
            "artifacts-only",
            "--repository",
            "owner/repo",
            "--github-token",
            "token",
            "--api-base-url",
            &server.url,
        ])
        .output()?;
    if !artifact_run.status.success() {
        return Err(String::from_utf8_lossy(&artifact_run.stderr)
            .to_string()
            .into());
    }
    if !repo.join("docs/releases/v1.1.0.md").is_file()
        || !repo.join("docs/releases/v1.1.0.txt").is_file()
        || !repo.join("docs/releases/v1.1.0.html").is_file()
        || repo.join("docs/releases/v1.3.0.md").is_file()
        || !repo.join("docs/releases/releases.json").is_file()
        || !repo.join("docs/releases/feed.xml").is_file()
        || !repo.join(".landmark/backfill-manifest.json").is_file()
    {
        return Err("artifact-only backfill did not write the expected artifact set or wrote an ambiguous duplicate".into());
    }

    let release_body_preview = Command::new(current_exe())
        .args([
            "backfill",
            "--repo-root",
            repo.to_str().unwrap(),
            "--since",
            "v1.2.0",
            "--mode",
            "release-body",
            "--dry-run",
            "--repository",
            "owner/repo",
            "--github-token",
            "token",
            "--api-base-url",
            &server.url,
        ])
        .output()?;
    if !release_body_preview.status.success() {
        return Err(String::from_utf8_lossy(&release_body_preview.stderr)
            .to_string()
            .into());
    }
    let release_manifest: Value = serde_json::from_slice(&release_body_preview.stdout)?;
    if !release_manifest["skipped_tags"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| {
            entry["tag"] == "v1.3.0"
                && entry["reason"]
                    .as_str()
                    .unwrap_or("")
                    .contains("duplicate changelog")
        })
    {
        return Err("release-body dry-run did not refuse duplicate changelog mapping".into());
    }
    if !release_manifest["skipped_tags"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| {
            entry["tag"] == "v1.5.0"
                && entry["reason"]
                    .as_str()
                    .unwrap_or("")
                    .contains("refusing to duplicate")
        })
    {
        return Err("release-body dry-run did not refuse existing-body source duplication".into());
    }
    if !release_manifest["release_body_updates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["tag"] == "pkg-a@v1.4.0" && entry["dry_run"] == true)
    {
        return Err("release-body dry-run did not preview package tag update".into());
    }

    let confirmed_update = Command::new(current_exe())
        .args([
            "backfill",
            "--repo-root",
            repo.to_str().unwrap(),
            "--since",
            "v1.3.0",
            "--mode",
            "release-body",
            "--confirm-release-body",
            "--repository",
            "owner/repo",
            "--github-token",
            "token",
            "--api-base-url",
            &server.url,
        ])
        .output()?;
    if !confirmed_update.status.success() {
        return Err(String::from_utf8_lossy(&confirmed_update.stderr)
            .to_string()
            .into());
    }
    let updated_body = server
        .state
        .lock()
        .unwrap()
        .releases
        .get("pkg-a@v1.4.0")
        .and_then(|release| release["body"].as_str())
        .unwrap_or("")
        .to_string();
    if !updated_body.contains("## What's New") || !updated_body.contains("package artifact history")
    {
        return Err("confirmed release-body update did not patch fake GitHub release".into());
    }

    let empty_template_preview = Command::new(current_exe())
        .args([
            "backfill",
            "--repo-root",
            repo.to_str().unwrap(),
            "--since",
            "v1.3.0",
            "--dry-run",
            "--output-text-file",
            "",
            "--output-html-file",
            "",
            "--output-json",
            "",
            "--rss-feed-file",
            "",
        ])
        .output()?;
    if !empty_template_preview.status.success() {
        return Err(String::from_utf8_lossy(&empty_template_preview.stderr)
            .to_string()
            .into());
    }
    let empty_template_manifest: Value = serde_json::from_slice(&empty_template_preview.stdout)?;
    if !empty_template_manifest["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| {
            entry["tag"] == "pkg-a@v1.4.0"
                && entry["plaintext"] == ""
                && entry["html"] == ""
                && entry["json"] == ""
                && entry["rss"] == ""
        })
    {
        return Err("empty artifact templates were not treated as disabled outputs".into());
    }

    let missing_since = Command::new(current_exe())
        .args([
            "backfill",
            "--repo-root",
            repo.to_str().unwrap(),
            "--since",
            "v9.9.9",
            "--dry-run",
        ])
        .output()?;
    if !missing_since.status.success() {
        return Err(String::from_utf8_lossy(&missing_since.stderr)
            .to_string()
            .into());
    }
    let missing_manifest: Value = serde_json::from_slice(&missing_since.stdout)?;
    if missing_manifest["skipped_tags"][0]["reason"] != "since tag not found" {
        return Err("missing since tag was not reported as a skip reason".into());
    }

    let private_preview = Command::new(current_exe())
        .args([
            "backfill",
            "--repo-root",
            repo.to_str().unwrap(),
            "--since",
            "v1.3.0",
            "--dry-run",
            "--repository",
            "owner/private",
        ])
        .output()?;
    if !private_preview.status.success() {
        return Err(String::from_utf8_lossy(&private_preview.stderr)
            .to_string()
            .into());
    }
    let private_manifest: Value = serde_json::from_slice(&private_preview.stdout)?;
    if private_manifest["processed_tags"][0]["release_status"]
        .as_str()
        .unwrap_or("")
        .contains("github token not configured")
    {
        Ok(json!({
            "dry_run": dry_manifest,
            "artifact_manifest": serde_json::from_slice::<Value>(&artifact_run.stdout)?,
            "release_body_preview": release_manifest,
            "confirmed_update": serde_json::from_slice::<Value>(&confirmed_update.stdout)?,
            "empty_template_preview": empty_template_manifest,
            "missing_since": missing_manifest,
            "private_preview": private_manifest,
        }))
    } else {
        Err("private/no-token release lookup was not reported".into())
    }
}

pub(crate) fn scenario_action_static_contract(_: &Path) -> Result<Value> {
    let action = fs::read_to_string("action.yml")?;
    if action.contains("python ") || action.contains("setup-python") {
        return Err("action.yml still invokes Python".into());
    }
    if !action.contains("dist/landmark") {
        return Err("action.yml does not invoke dist/landmark".into());
    }
    if !action.contains("dist/landmark\" run")
        || !action.contains("--provider github")
        || !action.contains("--release-tag \"${RELEASE_TAG}\"")
    {
        return Err("action.yml does not invoke the provider-neutral run command".into());
    }
    if action.contains("dist/landmark\" update-release") {
        return Err("action.yml still mutates GitHub releases through update-release".into());
    }
    if action.contains("dist/landmark\" write-artifacts")
        || action.contains("dist/landmark\" update-feed")
    {
        return Err("action.yml still writes release artifacts outside the run command".into());
    }
    Ok(json!({"checked": ["action.yml", "provider-neutral run"]}))
}

pub(crate) fn scenario_action_manifest_defaults_precedence(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("action-manifest-defaults");
    fs::create_dir_all(&repo)?;
    fs::write(
        repo.join(".landmark.yml"),
        r#"product:
  name: Manifest Product
  description: Manifest description
audience: enterprise
voice: Manifest voice
changelog:
  source: prs
artifacts:
  markdown: docs/releases/{version}.md
  plaintext: docs/releases/{version}.txt
  html: docs/releases/{version}.html
  json: docs/releases/releases.json
  rss: docs/releases/feed.xml
model:
  policy: cheap
  fallbacks:
    - manifest/fallback
"#,
    )?;
    let output = temp_file("landmark-manifest-defaults")?;
    let result = Command::new(current_exe())
        .args([
            "manifest-defaults",
            "--repo-root",
            repo.to_str().unwrap(),
            "--github-output",
        ])
        .arg(&output)
        .output()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).to_string().into());
    }
    let defaults = parse_outputs(&output)?;
    let effective_audience = action_value("", defaults.get("audience"), "general");
    let explicit_audience = action_value("developer", defaults.get("audience"), "general");
    let effective_model = action_value("", defaults.get("llm_model"), "anthropic/claude-sonnet-4");
    let explicit_model = action_value(
        "explicit/model",
        defaults.get("llm_model"),
        "anthropic/claude-sonnet-4",
    );
    let effective_markdown = action_value("", defaults.get("notes_output_file"), "");
    let explicit_markdown = action_value("docs/explicit.md", defaults.get("notes_output_file"), "");

    if effective_audience != "enterprise"
        || explicit_audience != "developer"
        || effective_model != "openai/gpt-4o-mini"
        || explicit_model != "explicit/model"
        || effective_markdown != "docs/releases/{version}.md"
        || explicit_markdown != "docs/explicit.md"
    {
        return Err("manifest default precedence did not match action semantics".into());
    }

    Ok(json!({
        "checked": [
            "manifest-defaults github output",
            "empty action input uses manifest default",
            "explicit action input overrides manifest default",
            "model.policy cheap selects openai/gpt-4o-mini"
        ],
        "defaults": defaults,
        "effective": {
            "audience": effective_audience,
            "llm_model": effective_model,
            "notes_output_file": effective_markdown,
        },
        "explicit": {
            "audience": explicit_audience,
            "llm_model": explicit_model,
            "notes_output_file": explicit_markdown,
        }
    }))
}

pub(crate) fn action_value(input: &str, manifest: Option<&String>, fallback: &str) -> String {
    trimmed_option(input)
        .or_else(|| manifest.and_then(|value| trimmed_option(value)))
        .unwrap_or_else(|| fallback.to_string())
}

pub(crate) fn scenario_publication_degraded_required(_: &Path) -> Result<Value> {
    let output = temp_file("landmark-policy")?;
    let result = Command::new(current_exe())
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
    if result.status.success() {
        return Err("degraded required synthesis should fail".into());
    }
    Ok(json!({"outputs": parse_outputs(&output)?}))
}

pub(crate) fn scenario_publication_degraded_optional(_: &Path) -> Result<Value> {
    let output = temp_file("landmark-policy")?;
    let result = Command::new(current_exe())
        .args([
            "release-policy",
            "publication",
            "--synthesis-required",
            "false",
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
    if !result.status.success() {
        return Err("degraded optional synthesis should pass".into());
    }
    Ok(json!({"outputs": parse_outputs(&output)?}))
}

pub(crate) fn scenario_summary_release_update_failed(_: &Path) -> Result<Value> {
    scenario_summary_failure("release_update", "update failed")
}

pub(crate) fn scenario_summary_artifact_failed(_: &Path) -> Result<Value> {
    scenario_summary_failure("artifact_write", "artifact failed")
}

pub(crate) fn scenario_summary_rss_failed(_: &Path) -> Result<Value> {
    scenario_summary_failure("rss_update", "rss failed")
}

pub(crate) fn scenario_summary_failure(stage: &str, message: &str) -> Result<Value> {
    let output = temp_file("landmark-summary")?;
    let result = Command::new(current_exe())
        .args([
            "release-policy",
            "summary",
            "--synthesis-enabled",
            "true",
            "--released",
            "true",
            "--synth-succeeded",
            "true",
            "--synth-quality",
            "valid",
            "--update-succeeded",
            if stage == "release_update" {
                "false"
            } else {
                "true"
            },
            "--update-failure-stage",
            if stage == "release_update" { stage } else { "" },
            "--update-failure-message",
            if stage == "release_update" {
                message
            } else {
                ""
            },
            "--artifact-succeeded",
            if stage == "artifact_write" {
                "false"
            } else {
                "true"
            },
            "--artifact-failure-stage",
            if stage == "artifact_write" { stage } else { "" },
            "--artifact-failure-message",
            if stage == "artifact_write" {
                message
            } else {
                ""
            },
            "--rss-enabled",
            if stage == "rss_update" {
                "true"
            } else {
                "false"
            },
            "--rss-succeeded",
            if stage == "rss_update" {
                "false"
            } else {
                "true"
            },
            "--rss-failure-stage",
            if stage == "rss_update" { stage } else { "" },
            "--rss-failure-message",
            if stage == "rss_update" { message } else { "" },
            "--github-output",
        ])
        .arg(&output)
        .output()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).to_string().into());
    }
    Ok(json!({"outputs": parse_outputs(&output)?}))
}
