use crate::*;

/// Regression for the canary v1.6.0/v1.7.1 incident: canary's `release.yml`
/// (full mode, triggered by CI completing on master) creates the release and
/// synthesizes notes for it; that release creation fires a `release:
/// published` webhook that triggers canary's separate `landmark-release.yml`
/// (synthesis-only mode), which synthesizes notes for the *same* tag again.
/// Two `update-release` calls land on one release, each with independently
/// generated (differently worded) notes for the same underlying changes. The
/// second call must replace the first run's synthesized section, not stack
/// on top of it — otherwise the release body ends up with the same fix
/// described twice.
pub(crate) fn scenario_release_body_idempotent_across_reruns(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("release-body-idempotent-across-reruns");
    init_fixture_repo(&repo, "v1.7.1")?;

    let mut fake = FakeState {
        update_status: 200,
        ..Default::default()
    };
    fake.releases.insert(
        "v1.7.1".to_string(),
        json!({
            "id": 1,
            "tag_name": "v1.7.1",
            "body": "## [1.7.1](https://github.com/misty-step/canary/compare/v1.7.0...v1.7.1) (2026-07-02)\n\n\n### Bug Fixes\n\n* **witness:** stop overdue pressure from blocking recovery",
            "html_url": "https://example.invalid/releases/v1.7.1",
        }),
    );
    let server = start_fake_server(fake)?;

    let run_update = |notes: &str| -> Result<()> {
        let notes_file = repo.join("notes.md");
        fs::write(&notes_file, notes)?;
        let update = Command::new(current_exe())
            .args([
                "update-release",
                "--github-token",
                "token",
                "--repository",
                "owner/repo",
                "--tag",
                "v1.7.1",
                "--notes-file",
            ])
            .arg(&notes_file)
            .args(["--api-base-url", &server.url])
            .current_dir(&repo)
            .output()?;
        if !update.status.success() {
            return Err(String::from_utf8_lossy(&update.stderr).to_string().into());
        }
        Ok(())
    };

    // Full-mode run's synthesis: notes carry their own `## Bug Fixes` subheading.
    run_update(
        "## Bug Fixes\n\n* Fixed canary-watchman agent recovery deadlock where the \
         watchman's own overdue pressure prevented it from completing recovery \
         check-ins, potentially causing permanent monitoring gaps during \
         high-pressure scenarios.\n\n> Landmark classification notice: deterministic \
         release signals disagreed with model classification; synthesis proceeded.",
    )?;
    // Synthesis-only run fired by the `release: published` webhook from the run
    // above: same underlying fix, independently reworded by the model.
    run_update(
        "## Bug Fixes\n\n* Fixed canary-watchman recovery deadlock where the \
         watchman's own overdue pressure prevented it from completing recovery \
         check-ins, breaking automatic recovery workflows for agent-operated \
         reliability scenarios.\n\n> Landmark classification notice: deterministic \
         release signals disagreed with model classification; synthesis proceeded.",
    )?;

    let final_body = server.state.lock().unwrap().releases["v1.7.1"]["body"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let top_level_sections = final_body.matches("\n## Bug Fixes\n").count();
    if top_level_sections != 1 {
        return Err(format!(
            "expected a single top-level Bug Fixes section after two synthesis \
             runs, found {top_level_sections}:\n{final_body}"
        )
        .into());
    }
    if final_body.contains("permanent monitoring gaps") {
        return Err(format!(
            "first run's notes leaked into the body after the second run replaced them:\n{final_body}"
        )
        .into());
    }
    if !final_body.contains("breaking automatic recovery workflows") {
        return Err(
            format!("expected the second run's notes in the final body:\n{final_body}").into(),
        );
    }
    if !final_body.contains("### Bug Fixes") {
        return Err(format!(
            "expected semantic-release's own changelog footer to survive both synthesis runs:\n{final_body}"
        )
        .into());
    }

    Ok(json!({
        "final_body": final_body,
        "checked": [
            "two synthesis runs on one release converge on the latest notes",
            "semantic-release footer preserved",
        ],
    }))
}
