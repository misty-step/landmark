use crate::*;

// Pre-stable action-level behavior, exercised through the real `landmark run`
// binary against fixture repos. Proves the Rust version engine keeps a 0.x line
// below 1.0.0, seeds no-tag repos below 1.0.0, and leaves stable lines alone.
// See card landmark-016.

pub(crate) fn scenario_prestable_breaking_stays_below_one(tmp_root: &Path) -> Result<Value> {
    // Tagged v0.3.0 + a breaking change releases v0.4.0, not v1.0.0: on a 0.x
    // line the breaking boundary is the minor position.
    let repo = tmp_root.join("prestable-breaking");
    init_fixture_repo(&repo, "v0.3.0")?;
    commit_feature(&repo, "feat(api)!: rename the public output field")?;
    let evidence = run_landmark_run_local(&repo)?;
    assert_prestable_decision(&evidence, "v0.4.0", "minor", "major", "pre-stable")?;
    Ok(json!({
        "decision": evidence["version_decision"],
        "release_tag": evidence["release_tag"],
    }))
}

pub(crate) fn scenario_prestable_untagged_first_release_below_one(
    tmp_root: &Path,
) -> Result<Value> {
    // A repo with no tags at all: a breaking first change still resolves to a
    // pre-stable first release (v0.1.0) rather than semantic-release's 1.0.0.
    let repo = tmp_root.join("prestable-untagged");
    init_untagged_repo(&repo, "feat(api)!: first breaking feature")?;
    let evidence = run_landmark_run_local(&repo)?;
    assert_prestable_decision(&evidence, "v0.1.0", "minor", "major", "pre-stable")?;
    let release_tag = evidence["release_tag"].as_str().unwrap_or_default();
    if release_tag.trim_start_matches('v').starts_with("1.") {
        return Err("untagged pre-stable repo crossed to 1.x".into());
    }
    Ok(json!({
        "decision": evidence["version_decision"],
        "release_tag": evidence["release_tag"],
    }))
}

pub(crate) fn scenario_stable_line_still_majors(tmp_root: &Path) -> Result<Value> {
    // A repo already at v2.1.0 is stable: a breaking change still majors to
    // v3.0.0. Pre-stable rules must not touch a >= 1.0.0 line.
    let repo = tmp_root.join("stable-line-majors");
    init_fixture_repo(&repo, "v2.1.0")?;
    commit_feature(&repo, "feat(api)!: rename the public output field")?;
    let evidence = run_landmark_run_local(&repo)?;
    assert_prestable_decision(&evidence, "v3.0.0", "major", "major", "stable")?;
    Ok(json!({
        "decision": evidence["version_decision"],
        "release_tag": evidence["release_tag"],
    }))
}

fn commit_feature(repo: &Path, subject: &str) -> Result<()> {
    fs::write(repo.join("feature.txt"), format!("{subject}\n"))?;
    run_ok("git", ["add", "feature.txt"], repo)?;
    run_ok("git", ["commit", "-q", "-m", subject], repo)?;
    Ok(())
}

fn init_untagged_repo(path: &Path, subject: &str) -> Result<()> {
    fs::create_dir_all(path)?;
    run_ok("git", ["init", "-q"], path)?;
    run_ok("git", ["config", "user.name", "Landmark Replay"], path)?;
    run_ok(
        "git",
        ["config", "user.email", "replay@example.invalid"],
        path,
    )?;
    fs::write(path.join("README.md"), "# Untagged Fixture\n")?;
    run_ok("git", ["add", "."], path)?;
    run_ok("git", ["commit", "-q", "-m", subject], path)?;
    Ok(())
}

fn run_landmark_run_local(repo: &Path) -> Result<Value> {
    let result = Command::new(current_exe())
        .args(["run", "--provider", "local", "--repo-root"])
        .arg(repo)
        .output()?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).to_string().into());
    }
    let stdout_evidence: Value = serde_json::from_slice(&result.stdout)?;
    let evidence_path = repo.join(".landmark/run/evidence.json");
    let file_evidence: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    if stdout_evidence != file_evidence {
        return Err("pre-stable run stdout did not match written evidence packet".into());
    }
    Ok(file_evidence)
}

fn assert_prestable_decision(
    evidence: &Value,
    release_tag: &str,
    bump: &str,
    raw_bump: &str,
    stability: &str,
) -> Result<()> {
    let checks = [
        ("/release_tag", release_tag),
        ("/version_decision/bump", bump),
        ("/version_decision/raw_bump", raw_bump),
        ("/version_decision/stability", stability),
    ];
    for (pointer, expected) in checks {
        let actual = evidence
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("evidence missing string at {pointer}"))?;
        if actual != expected {
            return Err(format!("{pointer} expected `{expected}`, got `{actual}`").into());
        }
    }
    Ok(())
}
