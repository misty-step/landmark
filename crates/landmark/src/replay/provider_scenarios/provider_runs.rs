use crate::*;
pub(crate) fn scenario_consumer_full_mode_success(tmp_root: &Path) -> Result<Value> {
    consumer_success(tmp_root, "consumer-full", true)
}

pub(crate) fn scenario_consumer_synthesis_only_success(tmp_root: &Path) -> Result<Value> {
    consumer_success(tmp_root, "consumer-synthesis-only", false)
}

pub(crate) fn scenario_first_run_local_preview(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("first-run-local-preview");
    init_fixture_repo(&repo, "v0.1.0")?;
    fs::write(repo.join("README.md"), "# First Run Demo\n")?;
    fs::write(repo.join("feature.txt"), "first run adoption\n")?;
    run_ok("git", ["add", "README.md", "feature.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "fix(cli): make first run obvious"],
        &repo,
    )?;

    let result = Command::new(current_exe())
        .args(["run", "--provider", "local", "--repo-root"])
        .arg(&repo)
        .output()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).to_string().into());
    }
    let stdout_evidence: Value = serde_json::from_slice(&result.stdout)?;
    let evidence_path = repo.join(".landmark/run/evidence.json");
    let evidence: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    if stdout_evidence != evidence {
        return Err("first-run preview stdout did not match written evidence packet".into());
    }
    if evidence["provider"] != "local"
        || evidence["publication"]["release_body_updated"] != false
        || evidence["version_decision"]["bump"] != "patch"
        || evidence["release_tag"] != "v0.1.1"
    {
        return Err("first-run preview evidence did not record local patch preview".into());
    }
    let expected = [
        repo.join(".landmark/run/technical-changelog.md"),
        repo.join(".landmark/run/evidence.json"),
        repo.join("docs/releases/v0.1.1.md"),
        repo.join("docs/releases/v0.1.1.txt"),
        repo.join("docs/releases/v0.1.1.html"),
        repo.join("docs/releases/releases.json"),
        repo.join("docs/releases/feed.xml"),
    ];
    for path in expected {
        if !path.is_file() {
            return Err(format!("first-run preview did not write {}", path.display()).into());
        }
    }
    let notes = fs::read_to_string(repo.join("docs/releases/v0.1.1.md"))?;
    let technical = fs::read_to_string(repo.join(".landmark/run/technical-changelog.md"))?;
    if !notes.contains("Make first run obvious")
        || !technical.contains("fix(cli): make first run obvious")
    {
        return Err("first-run preview artifacts did not include release context".into());
    }
    Ok(json!({
        "release_tag": evidence["release_tag"],
        "provider": evidence["provider"],
        "evidence": evidence_path,
        "markdown": repo.join("docs/releases/v0.1.1.md"),
        "technical_changelog": repo.join(".landmark/run/technical-changelog.md")
    }))
}

pub(crate) fn scenario_local_provider_run(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("local-provider-run");
    init_fixture_repo(&repo, "v1.0.0")?;
    fs::write(repo.join("feature.txt"), "portable release\n")?;
    run_ok("git", ["add", "feature.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat(cli): add portable release run"],
        &repo,
    )?;
    fs::write(repo.join("lane.txt"), "agent lane maintenance\n")?;
    run_ok("git", ["add", "lane.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "chore(ci): refresh agent lane"],
        &repo,
    )?;
    let result = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "local",
            "--repo-root",
            repo.to_str().unwrap(),
            "--repository",
            "local-provider-run",
            "--output-dir",
            ".landmark/run",
            "--technical-changelog-file",
            ".landmark/run/technical.md",
            "--evidence-file",
            ".landmark/run/evidence.json",
            "--output-file",
            "docs/releases/{version}.md",
            "--output-text-file",
            "docs/releases/{version}.txt",
            "--output-html-file",
            "docs/releases/{version}.html",
            "--output-json",
            "docs/releases/releases.json",
            "--rss-feed-file",
            "docs/releases/feed.xml",
        ])
        .output()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).to_string().into());
    }
    let evidence_path = repo.join(".landmark/run/evidence.json");
    let evidence: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    if evidence["provider"] != "local" {
        return Err("local provider evidence did not record provider=local".into());
    }
    if evidence["release_tag"] != "v1.1.0" {
        return Err(format!(
            "expected local run to compute v1.1.0, got {}",
            evidence["release_tag"]
        )
        .into());
    }
    if evidence["version_decision"]["bump"] != "minor" {
        return Err("local run did not classify feat commit as a minor bump".into());
    }
    if evidence["artifacts"]["technical_changelog_schema"]
        != "landmark.internal-technical-changelog.v1"
        || evidence["artifacts"]["public_notes_schema"] != "landmark.public-release-notes.v1"
        || evidence["artifacts"]["technical_changelog_audience"] != "internal-developer-operator"
        || evidence["artifacts"]["release_kit_schema"] != "landmark.release-kit.v1"
    {
        return Err(
            "local run evidence did not separate internal, public, and release-kit artifact schemas"
                .into(),
        );
    }
    release_kit_contract::assert_contract(&evidence["release_kit"], "local run release kit")?;
    let markdown = repo.join("docs/releases/v1.1.0.md");
    let plaintext = repo.join("docs/releases/v1.1.0.txt");
    let html = repo.join("docs/releases/v1.1.0.html");
    let json_path = repo.join("docs/releases/releases.json");
    let feed = repo.join("docs/releases/feed.xml");
    let release_kit = repo.join(".landmark/run/release-kit.json");
    for path in [
        &markdown,
        &plaintext,
        &html,
        &json_path,
        &feed,
        &release_kit,
    ] {
        if !path.is_file() {
            return Err(format!("local run did not write {}", path.display()).into());
        }
    }
    let release_kit_file: Value = serde_json::from_str(&fs::read_to_string(&release_kit)?)?;
    if release_kit_file != evidence["release_kit"] {
        return Err(
            "local run evidence release kit did not match written release-kit artifact".into(),
        );
    }
    let social_draft = release_kit_artifact(&release_kit_file, "social-post-drafts")
        .ok_or("local run did not emit social-post-drafts for a feature release")?;
    let social_draft_payload = &social_draft["draft"];
    if social_draft["kind"] != "social_copy"
        || social_draft["status"] != "produced"
        || social_draft_payload["variants"]
            .as_array()
            .is_none_or(|variants| variants.len() != 2)
        || social_draft_payload["angle"]
            .as_str()
            .is_none_or(|angle| !angle.contains("user-facing capability"))
        || social_draft_payload["voice_card"]
            .as_str()
            .is_none_or(|voice_card| voice_card.trim().is_empty())
        || social_draft_payload["evidence_link"] != "local://local-provider-run/releases/v1.1.0"
    {
        return Err(format!(
            "local run social draft did not include two variants, angle, and evidence link: {social_draft}"
        )
        .into());
    }
    if !release_kit_file["approvals"]
        .as_array()
        .is_some_and(|approvals| {
            approvals.iter().any(|approval| {
                approval["artifact_id"] == "social-post-drafts" && approval["state"] == "pending"
            })
        })
    {
        return Err("local run social draft was not gated behind pending operator review".into());
    }
    if release_kit_file["producer_contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|contract| {
            contract["output_artifacts"]
                .as_array()
                .into_iter()
                .flatten()
        })
        .any(|artifact| artifact == "social-post-drafts")
    {
        return Err("local run social draft unexpectedly had a producer/autopost contract".into());
    }
    let social_text = serde_json::to_string(social_draft_payload)?;
    for leaked in ["feat(", "chore(ci)", "agent lane", "autopost"] {
        if social_text.contains(leaked) {
            return Err(format!(
                "local run social draft leaked internal or publication detail `{leaked}`: {social_text}"
            )
            .into());
        }
    }
    let notes = fs::read_to_string(&markdown)?;
    if !notes.contains("Add portable release run") {
        return Err("local run release notes did not include the feature commit".into());
    }
    let releases: Value = serde_json::from_str(&fs::read_to_string(&json_path)?)?;
    let entry = releases
        .as_array()
        .and_then(|entries| entries.first())
        .ok_or("local run release JSON did not include a release entry")?;
    assert_public_release_entry_contract(entry)?;
    if entry["schema_version"] != "landmark.public-release-notes.v1"
        || entry["repository"] != "local-provider-run"
        || entry["release_url"] != "local://local-provider-run/releases/v1.1.0"
        || entry["audience"] != "general"
    {
        return Err(format!(
            "local run release JSON is not a self-contained public site artifact: {entry}"
        )
        .into());
    }
    let entry_text = serde_json::to_string(entry)?;
    for leaked in ["feat(", "chore(ci)", "agent lane", "Technical Changelog"] {
        if entry_text.contains(leaked) {
            return Err(format!(
                "local run release JSON leaked internal release-process detail `{leaked}`: {entry_text}"
            )
            .into());
        }
    }
    let plaintext = entry["plaintext"].as_str().unwrap_or_default();
    if plaintext.contains("Technical Changelog") {
        return Err(format!(
            "local run release JSON leaked internal release-process detail: {plaintext}"
        )
        .into());
    }
    let technical = fs::read_to_string(repo.join(".landmark/run/technical.md"))?;
    if !technical.contains("feat(cli): add portable release run") {
        return Err("local run technical changelog did not include the raw commit".into());
    }
    if !technical.contains("chore(ci): refresh agent lane") {
        return Err("local run technical changelog dropped the internal commit".into());
    }
    run_ok("git", ["tag", "v1.1.0"], &repo)?;
    fs::write(repo.join("after-release.txt"), "post release work\n")?;
    run_ok("git", ["add", "after-release.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "fix(cli): post-release patch"],
        &repo,
    )?;
    let tagged_result = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "local",
            "--repo-root",
            repo.to_str().unwrap(),
            "--repository",
            "local-provider-run",
            "--release-tag",
            "v1.1.0",
            "--output-dir",
            ".landmark/tagged-run",
            "--technical-changelog-file",
            ".landmark/tagged-run/technical.md",
            "--evidence-file",
            ".landmark/tagged-run/evidence.json",
            "--output-file",
            "",
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
    if !tagged_result.status.success() {
        return Err(String::from_utf8_lossy(&tagged_result.stderr)
            .to_string()
            .into());
    }
    let tagged_evidence_path = repo.join(".landmark/tagged-run/evidence.json");
    let tagged_evidence: Value = serde_json::from_str(&fs::read_to_string(&tagged_evidence_path)?)?;
    if tagged_evidence["version_decision"]["range"] != "v1.0.0..v1.1.0" {
        return Err(format!(
            "expected existing-tag run to end at v1.1.0, got {}",
            tagged_evidence["version_decision"]["range"]
        )
        .into());
    }
    if tagged_evidence["version_decision"]["commit_count"] != 2 {
        return Err("existing-tag run included commits outside the tagged range".into());
    }
    let tagged_technical = fs::read_to_string(repo.join(".landmark/tagged-run/technical.md"))?;
    if tagged_technical.contains("post-release patch") {
        return Err("existing-tag run included a post-release commit".into());
    }
    let breaking_repo = tmp_root.join("local-provider-breaking-footer");
    init_fixture_repo(&breaking_repo, "v1.2.3")?;
    fs::write(breaking_repo.join("api.txt"), "breaking api\n")?;
    run_ok("git", ["add", "api.txt"], &breaking_repo)?;
    run_ok(
        "git",
        [
            "commit",
            "-q",
            "-m",
            "feat(api): rename field",
            "-m",
            "BREAKING CHANGE: clients must migrate field names",
        ],
        &breaking_repo,
    )?;
    let breaking_result = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "local",
            "--repo-root",
            breaking_repo.to_str().unwrap(),
            "--repository",
            "local-provider-breaking-footer",
            "--output-dir",
            ".landmark/run",
            "--technical-changelog-file",
            ".landmark/run/technical.md",
            "--evidence-file",
            ".landmark/run/evidence.json",
            "--output-file",
            "",
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
    if !breaking_result.status.success() {
        return Err(String::from_utf8_lossy(&breaking_result.stderr)
            .to_string()
            .into());
    }
    let breaking_evidence_path = breaking_repo.join(".landmark/run/evidence.json");
    let breaking_evidence: Value =
        serde_json::from_str(&fs::read_to_string(&breaking_evidence_path)?)?;
    if breaking_evidence["version_decision"]["bump"] != "major"
        || breaking_evidence["release_tag"] != "v2.0.0"
    {
        return Err("local run did not treat BREAKING CHANGE footer as a major bump".into());
    }
    release_kit_contract::assert_contract(
        &breaking_evidence["release_kit"],
        "breaking release kit",
    )?;
    let breaking_artifacts = breaking_evidence["release_kit"]["artifacts"]
        .as_array()
        .ok_or("breaking release kit artifacts missing")?;
    if !breaking_artifacts.iter().any(|artifact| {
        artifact["owner"] == "producer-adapter" && artifact["kind"] == "migration_guide"
    }) || !breaking_artifacts
        .iter()
        .any(|artifact| artifact["owner"] == "producer-adapter" && artifact["kind"] == "video")
    {
        return Err(
            "high-importance release kit did not plan richer adapter-owned artifacts".into(),
        );
    }
    if !breaking_evidence["release_kit"]["producer_contracts"]
        .as_array()
        .is_some_and(|contracts| {
            contracts.iter().any(|contract| {
                contract["adapter_kind"] == "local-cli"
                    && contract["mutates"] == false
                    && contract["evidence_path"]
                        .as_str()
                        .unwrap_or_default()
                        .contains(".landmark/run")
            })
        })
    {
        return Err("high-importance release kit did not name a non-mutating producer contract with evidence path".into());
    }

    let internal_repo = tmp_root.join("local-provider-internal");
    init_fixture_repo(&internal_repo, "v1.0.0")?;
    fs::create_dir_all(internal_repo.join(".github/workflows"))?;
    fs::write(internal_repo.join(".github/workflows/ci.yml"), "name: CI\n")?;
    run_ok("git", ["add", ".github/workflows/ci.yml"], &internal_repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "ci: refresh workflow"],
        &internal_repo,
    )?;
    let internal_result = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "local",
            "--repo-root",
            internal_repo.to_str().unwrap(),
            "--repository",
            "local-provider-internal",
            "--output-dir",
            ".landmark/run",
            "--technical-changelog-file",
            ".landmark/run/technical.md",
            "--evidence-file",
            ".landmark/run/evidence.json",
            "--output-file",
            "",
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
    if !internal_result.status.success() {
        return Err(String::from_utf8_lossy(&internal_result.stderr)
            .to_string()
            .into());
    }
    let internal_evidence_path = internal_repo.join(".landmark/run/evidence.json");
    let internal_evidence: Value =
        serde_json::from_str(&fs::read_to_string(&internal_evidence_path)?)?;
    release_kit_contract::assert_contract(
        &internal_evidence["release_kit"],
        "internal release kit",
    )?;
    let internal_artifacts = internal_evidence["release_kit"]["artifacts"]
        .as_array()
        .ok_or("internal release kit artifacts missing")?;
    if internal_evidence["release_kit"]["classification"]["importance"] != "low" {
        return Err("internal release kit did not classify as low importance".into());
    }
    if internal_artifacts.len() > 3
        || internal_artifacts
            .iter()
            .any(|artifact| artifact["owner"] == "producer-adapter")
        || release_kit_artifact(&internal_evidence["release_kit"], "social-post-drafts").is_some()
    {
        return Err("low-importance internal release kit did not stay small".into());
    }
    Ok(json!({
        "evidence": evidence,
        "tagged_evidence": tagged_evidence,
        "breaking_footer_evidence": breaking_evidence,
        "internal_evidence": internal_evidence,
        "stdout": String::from_utf8_lossy(&result.stdout).trim(),
        "artifacts": {
            "markdown": markdown,
            "plaintext": plaintext,
            "html": html,
            "json": json_path,
            "rss": feed,
            "technical_changelog": repo.join(".landmark/run/technical.md"),
            "evidence": evidence_path,
            "release_kit": release_kit,
        }
    }))
}

fn release_kit_artifact<'a>(release_kit: &'a Value, id: &str) -> Option<&'a Value> {
    release_kit["artifacts"]
        .as_array()?
        .iter()
        .find(|artifact| artifact["id"] == id)
}

pub(crate) fn assert_public_release_entry_contract(entry: &Value) -> Result<()> {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../../schemas/release-entry.v1.schema.json"
    ))?;
    let required = schema["required"]
        .as_array()
        .ok_or("release-entry schema missing required field list")?;
    for field in required {
        let field = field
            .as_str()
            .ok_or("release-entry required field was not a string")?;
        if entry.get(field).is_none() {
            return Err(
                format!("public release entry missing schema-required field `{field}`").into(),
            );
        }
    }
    for field in ["schema_version", "repository", "release_url", "audience"] {
        if entry[field].as_str().unwrap_or_default().trim().is_empty() {
            return Err(format!("public release entry missing static-site field `{field}`").into());
        }
    }
    for field in [
        "version",
        "tag",
        "notes",
        "markdown",
        "html",
        "plaintext",
        "slack",
        "published_at",
    ] {
        if entry[field].as_str().unwrap_or_default().trim().is_empty() {
            return Err(format!("public release entry has empty required field `{field}`").into());
        }
    }
    if entry["sections"].as_array().is_none_or(Vec::is_empty) {
        return Err("public release entry has no parsed sections".into());
    }
    Ok(())
}

pub(crate) fn scenario_github_provider_run(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("github-provider-run");
    init_fixture_repo(&repo, "v1.0.0")?;
    fs::write(repo.join("feature.txt"), "github provider release\n")?;
    run_ok("git", ["add", "feature.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat(action): add provider run"],
        &repo,
    )?;
    let mut fake = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.to_string(),
        update_status: 200,
        ..Default::default()
    };
    fake.releases.insert(
        "v1.1.0".to_string(),
        json!({"id": 11, "tag_name": "v1.1.0", "body": "## Technical\n\n- Existing release body", "html_url": "https://example.invalid/releases/v1.1.0"}),
    );
    let server = start_fake_server(fake)?;
    let notes_file = repo.join("notes.md");
    fs::write(
        &notes_file,
        "## Improvements in v1.1.0\n\n- Add provider run\n",
    )?;
    let result = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "github",
            "--repo-root",
            repo.to_str().unwrap(),
            "--repository",
            "owner/repo",
            "--release-tag",
            "v1.1.0",
            "--notes-file",
            notes_file.to_str().unwrap(),
            "--github-token",
            "token",
            "--api-base-url",
            &server.url,
            "--publish-release-body",
            "--output-dir",
            ".landmark/run",
            "--technical-changelog-file",
            ".landmark/run/technical.md",
            "--evidence-file",
            ".landmark/run/evidence.json",
            "--output-file",
            "docs/releases/{version}.md",
            "--output-json",
            "docs/releases/releases.json",
            "--rss-feed-file",
            "",
        ])
        .output()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).to_string().into());
    }
    let evidence_path = repo.join(".landmark/run/evidence.json");
    let evidence: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    if evidence["provider"] != "github" {
        return Err("github provider evidence did not record provider=github".into());
    }
    if evidence["publication"]["release_body_updated"] != true {
        return Err("github provider did not report release-body update".into());
    }
    let state = server.state.lock().unwrap();
    let body = state.releases["v1.1.0"]["body"].as_str().unwrap_or("");
    if !body.contains("## What's New") || !body.contains("Add provider run") {
        return Err("github provider did not update the fake release body with run notes".into());
    }
    Ok(json!({
        "evidence": evidence,
        "release_body": body,
        "requests": state.requests,
        "artifacts": {
            "markdown": repo.join("docs/releases/v1.1.0.md"),
            "json": repo.join("docs/releases/releases.json"),
            "technical_changelog": repo.join(".landmark/run/technical.md"),
            "evidence": evidence_path,
        }
    }))
}

pub(crate) fn scenario_provider_run_parity(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("provider-run-parity");
    init_fixture_repo(&repo, "v1.0.0")?;
    fs::write(repo.join("feature.txt"), "provider parity\n")?;
    run_ok("git", ["add", "feature.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat(release): add provider parity"],
        &repo,
    )?;

    let local = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "local",
            "--repo-root",
            repo.to_str().unwrap(),
            "--repository",
            "provider-run-parity",
            "--output-dir",
            ".landmark/local",
            "--technical-changelog-file",
            ".landmark/local/technical.md",
            "--evidence-file",
            ".landmark/local/evidence.json",
            "--output-file",
            "docs/local/{version}.md",
            "--output-text-file",
            "docs/local/{version}.txt",
            "--output-html-file",
            "docs/local/{version}.html",
            "--output-json",
            "docs/local/releases.json",
            "--rss-feed-file",
            "docs/local/feed.xml",
        ])
        .output()?;
    if !local.status.success() {
        return Err(String::from_utf8_lossy(&local.stderr).to_string().into());
    }
    let local_evidence_path = repo.join(".landmark/local/evidence.json");
    let local_evidence: Value = serde_json::from_str(&fs::read_to_string(&local_evidence_path)?)?;
    if local_evidence["provider"] != "local" || local_evidence["release_tag"] != "v1.1.0" {
        return Err("provider parity local run did not produce v1.1.0 local evidence".into());
    }
    for path in [
        repo.join("docs/local/v1.1.0.md"),
        repo.join("docs/local/v1.1.0.txt"),
        repo.join("docs/local/v1.1.0.html"),
        repo.join("docs/local/releases.json"),
        repo.join("docs/local/feed.xml"),
    ] {
        if !path.is_file() {
            return Err(
                format!("provider parity local run did not write {}", path.display()).into(),
            );
        }
    }

    let mut fake = FakeState {
        llm_status: 200,
        llm_notes: VALID_NOTES.to_string(),
        update_status: 200,
        ..Default::default()
    };
    fake.releases.insert(
        "v1.1.0".to_string(),
        json!({"id": 22, "tag_name": "v1.1.0", "body": "## Technical\n\n- Existing", "html_url": "https://example.invalid/releases/v1.1.0"}),
    );
    let server = start_fake_server(fake)?;
    let local_notes = repo.join("docs/local/v1.1.0.md");
    let github = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "github",
            "--repo-root",
            repo.to_str().unwrap(),
            "--repository",
            "owner/repo",
            "--release-tag",
            "v1.1.0",
            "--server-url",
            "https://github.enterprise.invalid",
            "--notes-file",
            local_notes.to_str().unwrap(),
            "--github-token",
            "token",
            "--api-base-url",
            &server.url,
            "--publish-release-body",
            "--output-dir",
            ".landmark/github",
            "--technical-changelog-file",
            ".landmark/github/technical.md",
            "--evidence-file",
            ".landmark/github/evidence.json",
            "--output-file",
            "docs/github/{version}.md",
            "--output-text-file",
            "docs/github/{version}.txt",
            "--output-html-file",
            "docs/github/{version}.html",
            "--output-json",
            "docs/github/releases.json",
            "--rss-feed-file",
            "docs/github/feed.xml",
        ])
        .output()?;
    if !github.status.success() {
        return Err(String::from_utf8_lossy(&github.stderr).to_string().into());
    }
    let github_evidence_path = repo.join(".landmark/github/evidence.json");
    let github_evidence: Value = serde_json::from_str(&fs::read_to_string(&github_evidence_path)?)?;
    if github_evidence["provider"] != "github"
        || github_evidence["release_tag"] != local_evidence["release_tag"]
        || github_evidence["publication"]["release_body_updated"] != true
    {
        return Err("provider parity github run did not publish the same fixture release".into());
    }
    for path in [
        repo.join("docs/github/v1.1.0.md"),
        repo.join("docs/github/v1.1.0.txt"),
        repo.join("docs/github/v1.1.0.html"),
        repo.join("docs/github/releases.json"),
        repo.join("docs/github/feed.xml"),
    ] {
        if !path.is_file() {
            return Err(format!(
                "provider parity github run did not write {}",
                path.display()
            )
            .into());
        }
    }
    let github_feed = fs::read_to_string(repo.join("docs/github/feed.xml"))?;
    if !github_feed.contains("<link>https://github.enterprise.invalid/owner/repo</link>") {
        return Err(
            "provider parity github feed channel did not use the configured GitHub server URL"
                .into(),
        );
    }
    if !github_feed.contains("https://github.enterprise.invalid/owner/repo/releases/tag/v1.1.0") {
        return Err(
            "provider parity github feed did not use the configured GitHub server URL".into(),
        );
    }
    let state = server.state.lock().unwrap();
    let body = state.releases["v1.1.0"]["body"].as_str().unwrap_or("");
    if !body.contains("Add provider parity") {
        return Err("provider parity github release body did not use local notes".into());
    }
    Ok(json!({
        "local_evidence": local_evidence,
        "github_evidence": github_evidence,
        "release_body": body,
        "requests": state.requests,
    }))
}

pub(crate) fn scenario_release_kit_classification_uses_structured_commits(
    tmp_root: &Path,
) -> Result<Value> {
    // Reproduces the landmark v1.25.0 shape (feat/feat/fix, all scoped) that
    // silently misfired to `significance: low` in release_kit::plan before it
    // was switched from the bare-text classifier to the structured,
    // commit-type-aware one. Scoped conventional headers ("feat(scope): ...")
    // never contain the literal substring "feat:" the bare classifier looked
    // for, and one of these three real subjects contains the literal word
    // "workflows", which the bare classifier's internal-tooling heuristic
    // matches -- landing exactly on `low` for what shipped two real features.
    let repo = tmp_root.join("release-kit-classification");
    init_fixture_repo(&repo, "v1.24.0")?;
    fs::write(repo.join("fleet.txt"), "backfill-first adoption lane\n")?;
    run_ok("git", ["add", "fleet.txt"], &repo)?;
    run_ok(
        "git",
        [
            "commit",
            "-q",
            "-m",
            "feat(fleet): deliver backfill-first adoption lane",
        ],
        &repo,
    )?;
    fs::write(repo.join("kit.txt"), "release kit artifact graph\n")?;
    run_ok("git", ["add", "kit.txt"], &repo)?;
    run_ok(
        "git",
        [
            "commit",
            "-q",
            "-m",
            "feat(run): emit release kit artifact graph",
        ],
        &repo,
    )?;
    fs::write(repo.join("workflow.txt"), "attach to existing workflows\n")?;
    run_ok("git", ["add", "workflow.txt"], &repo)?;
    run_ok(
        "git",
        [
            "commit",
            "-q",
            "-m",
            "fix(fleet): attach to existing release workflows",
        ],
        &repo,
    )?;

    let result = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "local",
            "--repo-root",
            repo.to_str().unwrap(),
            "--dry-run",
        ])
        .output()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).to_string().into());
    }
    let evidence: Value = serde_json::from_slice(&result.stdout)?;
    let classification = &evidence["release_kit"]["classification"];
    if classification["importance"] == "low" {
        return Err(format!(
            "release_kit classification regressed to the bare-text misfire: {classification}"
        )
        .into());
    }
    let needs_rich_artifacts = matches!(
        classification["importance"].as_str(),
        Some("high" | "launch" | "migration" | "security")
    );
    let audiences = classification["audiences"]
        .as_array()
        .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    let has_rich_audiences =
        audiences.contains(&"release-operator") && audiences.contains(&"docs-owner");
    if needs_rich_artifacts != has_rich_audiences {
        return Err(format!(
            "release_kit_needs_rich_artifacts disagreed with planned audiences for importance={}: {audiences:?}",
            classification["importance"]
        )
        .into());
    }
    Ok(json!({
        "evidence": evidence,
        "classification": classification,
    }))
}

pub(crate) fn scenario_misty_step_landmark_social_draft(_: &Path) -> Result<Value> {
    let repo = env::current_dir()?;
    let result = Command::new(current_exe())
        .args([
            "run",
            "--provider",
            "local",
            "--repo-root",
            repo.to_str().unwrap_or("."),
            "--repository",
            "misty-step/landmark",
            "--release-tag",
            "v1.27.0",
            "--dry-run",
        ])
        .output()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).to_string().into());
    }
    let evidence: Value = serde_json::from_slice(&result.stdout)?;
    if evidence["release_tag"] != "v1.27.0" || evidence["version_decision"]["bump"] != "minor" {
        return Err("real Landmark release proof did not exercise v1.27.0 minor release".into());
    }
    release_kit_contract::assert_contract(&evidence["release_kit"], "real Landmark release kit")?;
    let social = release_kit_artifact(&evidence["release_kit"], "social-post-drafts")
        .ok_or("real Landmark release did not emit social-post-drafts")?;
    let draft = &social["draft"];
    if social["status"] != "planned"
        || social["kind"] != "social_copy"
        || draft["variants"]
            .as_array()
            .is_none_or(|variants| variants.len() != 2)
        || draft["voice_card"]
            .as_str()
            .is_none_or(|voice_card| voice_card.trim().is_empty())
        || draft["evidence_link"] != "https://github.com/misty-step/landmark/releases/tag/v1.27.0"
    {
        return Err(
            format!("real Landmark release social draft missing draft shape: {social}").into(),
        );
    }
    let variants = draft["variants"].as_array().unwrap();
    if !variants
        .iter()
        .filter_map(Value::as_str)
        .all(|variant| variant.starts_with("User-facing note:"))
    {
        return Err(format!(
            "real Landmark release social variants did not apply the voice label: {variants:?}"
        )
        .into());
    }
    if !evidence["release_kit"]["approvals"]
        .as_array()
        .is_some_and(|approvals| {
            approvals.iter().any(|approval| {
                approval["artifact_id"] == "social-post-drafts" && approval["state"] == "pending"
            })
        })
    {
        return Err("real Landmark release social draft was not pending operator review".into());
    }
    Ok(json!({
        "release_tag": evidence["release_tag"],
        "version_decision": evidence["version_decision"],
        "social": social,
    }))
}
