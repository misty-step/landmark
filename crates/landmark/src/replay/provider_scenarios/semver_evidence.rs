use crate::*;

pub(crate) fn scenario_semver_evidence_agrees(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("semver-evidence-agrees");
    init_rust_fixture_repo(&repo, "v1.0.0")?;
    fs::write(repo.join("src/lib.rs"), "pub fn renamed_api() {}\n")?;
    run_ok("git", ["add", "src/lib.rs"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat(api)!: rename stable API"],
        &repo,
    )?;
    run_ok("git", ["tag", "v2.0.0"], &repo)?;

    let evidence = run_with_fake_cargo_semver(tmp_root, &repo, "findings", Some("v2.0.0"))?;
    assert_decision(&evidence, "major", "major", "major", "agreed", "findings")?;
    assert_json_eq(
        &evidence,
        "/version_decision/api_evidence/target",
        "v2.0.0",
        "explicit release tag evidence target",
    )?;
    assert_release_kit_carries_decision(&evidence)?;
    Ok(json!({
        "decision": evidence["version_decision"],
        "release_kit_decision": evidence["release_kit"]["release"]["version_decision"],
    }))
}

pub(crate) fn scenario_semver_evidence_upgrades(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("semver-evidence-upgrades");
    init_rust_fixture_repo(&repo, "v1.0.0")?;
    fs::write(repo.join("src/lib.rs"), "pub fn renamed_api() {}\n")?;
    run_ok("git", ["add", "src/lib.rs"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat(api): add replacement API"],
        &repo,
    )?;

    let evidence = run_with_fake_cargo_semver(tmp_root, &repo, "findings", None)?;
    assert_decision(&evidence, "major", "minor", "major", "upgraded", "findings")?;
    assert_json_eq(
        &evidence,
        "/release_tag",
        "v2.0.0",
        "semver evidence upgrade tag",
    )?;
    assert_release_kit_carries_decision(&evidence)?;
    Ok(json!({
        "decision": evidence["version_decision"],
        "release_tag": evidence["release_tag"],
    }))
}

pub(crate) fn scenario_semver_evidence_absent(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("semver-evidence-absent");
    init_fixture_repo(&repo, "v1.0.0")?;
    fs::write(repo.join("fix.txt"), "patch\n")?;
    run_ok("git", ["add", "fix.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "fix(cli): patch output"],
        &repo,
    )?;

    let evidence = run_landmark_run(&repo, None, None)?;
    assert_decision(
        &evidence,
        "patch",
        "patch",
        "none",
        "unavailable",
        "skipped",
    )?;
    assert_json_eq(
        &evidence,
        "/version_decision/api_evidence/provider",
        "none",
        "absent evidence provider",
    )?;
    let summary = evidence["version_decision"]["api_evidence"]["summary"]
        .as_str()
        .unwrap_or_default();
    if !summary.contains("no evidence provider") {
        return Err("absent evidence scenario did not record no-provider note".into());
    }
    assert_release_kit_carries_decision(&evidence)?;
    Ok(json!({
        "decision": evidence["version_decision"],
        "release_tag": evidence["release_tag"],
    }))
}

pub(crate) fn scenario_semver_evidence_tool_failure(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("semver-evidence-tool-failure");
    init_rust_fixture_repo(&repo, "v1.0.0")?;
    fs::write(
        repo.join("src/lib.rs"),
        "pub fn stable_api() {}\npub fn patched() {}\n",
    )?;
    run_ok("git", ["add", "src/lib.rs"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "fix(api): patch helper"],
        &repo,
    )?;

    let evidence = run_with_fake_cargo_semver(tmp_root, &repo, "failed", None)?;
    assert_decision(&evidence, "patch", "patch", "none", "unverified", "failed")?;
    let failure = evidence["version_decision"]["api_evidence"]["failure_message"]
        .as_str()
        .unwrap_or_default();
    if !failure.contains("rustdoc JSON unavailable") {
        return Err("tool failure scenario did not preserve failure message".into());
    }
    assert_release_kit_carries_decision(&evidence)?;
    Ok(json!({
        "decision": evidence["version_decision"],
        "release_tag": evidence["release_tag"],
    }))
}

fn run_with_fake_cargo_semver(
    tmp_root: &Path,
    repo: &Path,
    mode: &str,
    release_tag: Option<&str>,
) -> Result<Value> {
    let fake_bin = tmp_root.join("fake-cargo-bin");
    fs::create_dir_all(&fake_bin)?;
    let fake_cargo = fake_bin.join("cargo");
    fs::write(
        &fake_cargo,
        "#!/bin/sh\nif [ \"$1\" = \"semver-checks\" ]; then\n  case \"$LANDMARK_FAKE_SEMVER\" in\n    passed)\n      echo \"SemVer check passed\"\n      exit 0\n      ;;\n    findings)\n      echo \"SemVer check failed\"\n      echo \"Breaking changes detected\"\n      echo \"Required bump: Major\"\n      exit 1\n      ;;\n    failed)\n      echo \"error: rustdoc JSON unavailable\" >&2\n      exit 2\n      ;;\n  esac\nfi\necho \"fake cargo only supports semver-checks\" >&2\nexit 64\n",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&fake_cargo, fs::Permissions::from_mode(0o755))?;
    }
    run_landmark_run(repo, Some((&fake_bin, mode)), release_tag)
}

fn run_landmark_run(
    repo: &Path,
    fake_cargo: Option<(&Path, &str)>,
    release_tag: Option<&str>,
) -> Result<Value> {
    let mut command = Command::new(current_exe());
    command.args([
        "run",
        "--provider",
        "local",
        "--repo-root",
        repo.to_str().unwrap(),
        "--repository",
        "semver-evidence",
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
    ]);
    if let Some(release_tag) = release_tag {
        command.args(["--release-tag", release_tag]);
    }
    if let Some((fake_bin, mode)) = fake_cargo {
        let path = format!(
            "{}:{}",
            fake_bin.display(),
            env::var("PATH").unwrap_or_default()
        );
        command.env("PATH", path);
        command.env("LANDMARK_FAKE_SEMVER", mode);
    }
    let result = command.output()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).to_string().into());
    }
    let evidence_path = repo.join(".landmark/run/evidence.json");
    let file_evidence: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    let stdout_evidence: Value = serde_json::from_slice(&result.stdout)?;
    if file_evidence != stdout_evidence {
        return Err("semver evidence stdout did not match written evidence".into());
    }
    Ok(file_evidence)
}

fn assert_decision(
    evidence: &Value,
    bump: &str,
    commit_bump: &str,
    api_bump: &str,
    reconciliation: &str,
    api_status: &str,
) -> Result<()> {
    assert_json_eq(evidence, "/version_decision/bump", bump, "decision bump")?;
    assert_json_eq(
        evidence,
        "/version_decision/commit_bump",
        commit_bump,
        "commit bump",
    )?;
    assert_json_eq(
        evidence,
        "/version_decision/api_evidence_bump",
        api_bump,
        "api evidence bump",
    )?;
    assert_json_eq(
        evidence,
        "/version_decision/reconciliation",
        reconciliation,
        "reconciliation",
    )?;
    assert_json_eq(
        evidence,
        "/version_decision/api_evidence/status",
        api_status,
        "api evidence status",
    )?;
    let signals = evidence["version_decision"]["decisive_signals"]
        .as_array()
        .ok_or("decision missing decisive_signals")?;
    if !signals.iter().any(|signal| {
        signal
            .as_str()
            .unwrap_or_default()
            .contains("api-evidence:")
    }) {
        return Err("decision did not name API evidence as a signal".into());
    }
    Ok(())
}

fn assert_release_kit_carries_decision(evidence: &Value) -> Result<()> {
    release_kit_contract::assert_contract(&evidence["release_kit"], "semver evidence release kit")?;
    if evidence["release_kit"]["release"]["version_decision"] != evidence["version_decision"] {
        return Err("release kit did not carry exact run version decision".into());
    }
    Ok(())
}

fn assert_json_eq(value: &Value, pointer: &str, expected: &str, label: &str) -> Result<()> {
    let actual = value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{label} missing JSON string at {pointer}"))?;
    if actual != expected {
        return Err(format!("{label} expected `{expected}`, got `{actual}`").into());
    }
    Ok(())
}
