use crate::*;

const FABRICATED_NOTES: &str = "## Breaking Changes\n\
- You must update any scripts that call the old readiness entrypoint before upgrading.\n\n\
## New Features\n\
- Cold agents can now prove readiness before joining a run.\n\n\
## Bug Fixes\n\
- Resolved a delay in health-report delivery under load.\n";

/// Regression fixture for landmark-907 (the canary v1.14.0 incident): a
/// release with exactly one real `feat` PR, synthesized by a model that
/// invented a whole "Breaking Changes" section and "Bug Fixes" section
/// describing events that never happened. The old quality gate
/// (`validate_notes`) only checked Markdown shape (a `## ` heading and a
/// `- ` bullet) and scored the fabrication `valid`. This scenario fails on
/// any build where the grounding gate is missing or bypassed, and passes
/// once `synthesize` refuses to publish sections with zero matching release
/// commits.
pub(crate) fn scenario_synthesis_fabrication_gate(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("synthesis-fabrication-gate");
    fs::create_dir_all(&repo)?;
    run_ok("git", ["init", "-q"], &repo)?;
    run_ok("git", ["config", "user.name", "Landmark Replay"], &repo)?;
    run_ok(
        "git",
        ["config", "user.email", "replay@example.invalid"],
        &repo,
    )?;
    fs::write(repo.join("README.md"), "# Fixture\n")?;
    run_ok("git", ["add", "."], &repo)?;
    run_ok("git", ["commit", "-q", "-m", "chore: seed"], &repo)?;
    run_ok("git", ["tag", "v1.13.0"], &repo)?;

    // The single real change in this release: one feature PR, no fixes, no
    // breaking changes -- exactly the canary v1.14.0 shape reported on the
    // card (one line PR extract: "feat(agent-ops): add cold-agent readiness
    // proof entrypoint").
    fs::write(repo.join("agent-ops.txt"), "cold-agent readiness proof\n")?;
    run_ok("git", ["add", "agent-ops.txt"], &repo)?;
    run_ok(
        "git",
        [
            "commit",
            "-q",
            "-m",
            "feat(agent-ops): add cold-agent readiness proof entrypoint",
        ],
        &repo,
    )?;
    run_ok("git", ["tag", "v1.14.0"], &repo)?;

    let mut fake = FakeState {
        llm_status: 200,
        update_status: 200,
        ..Default::default()
    };
    // Classification call: the deterministic floor already carries the feat
    // signal, the model response just needs to parse.
    fake.llm_responses.push_back((
        200,
        json!({
            "categories": ["user-visible"],
            "significance": "medium",
            "user_visible": true,
            "breaking": false,
            "security": false,
            "migration_heavy": false,
            "reasons": ["release commit is a user-visible feature"]
        })
        .to_string(),
    ));
    // Synthesis call: the model fabricates Breaking Changes and Bug Fixes.
    fake.llm_responses
        .push_back((200, FABRICATED_NOTES.to_string()));
    let server = start_fake_server(fake)?;

    let quality_file = repo.join("quality.txt");
    let claim_map_file = repo.join("claim-map.json");
    let templates_dir = env::current_dir()?.join("templates/prompts");
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
            "Canary",
            "--version",
            "v1.14.0",
            "--changelog-file",
            "CHANGELOG.md",
            "--templates-dir",
        ])
        .arg(&templates_dir)
        .args(["--quality-file"])
        .arg(&quality_file)
        .args(["--claim-map-file"])
        .arg(&claim_map_file)
        .args(["--repo-root"])
        .arg(&repo)
        .current_dir(&repo)
        .output()?;

    if synth.status.success() {
        return Err(format!(
            "synthesize must refuse to publish fabricated sections, but it exited 0:\n{}",
            String::from_utf8_lossy(&synth.stdout)
        )
        .into());
    }

    let quality = fs::read_to_string(&quality_file).unwrap_or_default();
    let quality = quality.trim().to_string();
    if quality != "ungrounded" {
        return Err(
            format!("expected quality file to record 'ungrounded', got '{quality}'").into(),
        );
    }

    let claim_map: Value = serde_json::from_str(&fs::read_to_string(&claim_map_file)?)?;
    if claim_map["grounded"] != false {
        return Err(format!("claim map should be ungrounded: {claim_map}").into());
    }
    let ungrounded_sections: Vec<String> = claim_map["ungrounded_sections"]
        .as_array()
        .ok_or("claim map missing ungrounded_sections")?
        .iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect();
    if ungrounded_sections != vec!["Breaking Changes".to_string(), "Bug Fixes".to_string()] {
        return Err(format!(
            "expected Breaking Changes and Bug Fixes to be flagged ungrounded, got {ungrounded_sections:?}"
        )
        .into());
    }

    // The publication policy must hard-block even when synthesis is not
    // marked required -- fabrication is never an optional-quality tradeoff.
    let output = temp_file("landmark-fabrication-policy")?;
    let policy = Command::new(current_exe())
        .args([
            "release-policy",
            "publication",
            "--synthesis-required",
            "false",
            "--synthesis-strict",
            "false",
            "--synth-succeeded",
            "false",
            "--synth-quality",
            "ungrounded",
            "--github-output",
        ])
        .arg(&output)
        .output()?;
    if policy.status.success() {
        return Err("ungrounded synthesis must hard-fail the publication policy".into());
    }
    let outputs = parse_outputs(&output)?;
    if outputs.get("can_update_release").map(String::as_str) != Some("false") {
        return Err(
            format!("ungrounded synthesis must block release-body updates: {outputs:?}").into(),
        );
    }

    Ok(json!({
        "quality": quality,
        "ungrounded_sections": ungrounded_sections,
        "policy_outputs": outputs,
    }))
}
