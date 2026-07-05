use super::*;

const STALE_CHANGELOG: &str =
    "## [1.4.2]\n\n- fix: stale unrelated bugfix\n\n## [1.3.0]\n\n- feat: older feature\n";

#[test]
fn extract_release_section_returns_none_when_version_heading_is_missing() {
    // Regression: canary's CHANGELOG.md topped out at 1.4.2 while 1.6.0/1.7.0 were
    // being synthesized. The old behavior silently returned matches[0] (the 1.4.2
    // section) as if it were the 1.6.0 release, feeding the model a stale, unrelated
    // changelog it faithfully turned into confidently wrong release notes.
    let section = extract_release_section(STALE_CHANGELOG, "1.6.0");

    assert_eq!(section, None);
}

#[test]
fn extract_release_section_finds_the_matching_heading() {
    let section = extract_release_section(STALE_CHANGELOG, "1.4.2").expect("section found");

    assert!(section.contains("stale unrelated bugfix"));
    assert!(!section.contains("older feature"));
}

#[test]
fn extract_release_section_returns_none_when_text_has_no_headings_at_all() {
    let section = extract_release_section("just some prose, no version headings\n", "1.6.0");

    assert_eq!(section, None);
}

fn synthesize_args_with_repo(repo: &Path, version: &str) -> SynthesizeArgs {
    SynthesizeArgs {
        api_key: "test".into(),
        model: String::new(),
        model_policy: String::new(),
        api_url: "http://example.invalid".into(),
        fallback_models: String::new(),
        product_name: "Landmark".into(),
        product_description: String::new(),
        voice_guide: String::new(),
        audience: None,
        changelog_source: None,
        version: version.into(),
        changelog_file: repo.join("CHANGELOG.md"),
        release_body_file: repo.join("release-body.md"),
        pr_changelog_file: repo.join("pr-changelog.md"),
        prompt_template: PathBuf::from("."),
        quality_file: repo.join("quality.txt"),
        attempts_file: PathBuf::from("."),
        templates_dir: PathBuf::from("templates/prompts"),
        repo_root: repo.to_path_buf(),
        dry_run_cost: false,
        context_metadata_file: PathBuf::from("."),
        claim_map_file: PathBuf::from("."),
    }
}

fn test_synthesis_config() -> EffectiveSynthesisConfig {
    EffectiveSynthesisConfig {
        product_name: "Demo".into(),
        product_description: String::new(),
        voice_guide: String::new(),
        audience: "developer".into(),
        changelog_source: "auto".into(),
        model_policy: "balanced".into(),
        model: "primary/model".into(),
        fallback_models: String::new(),
        max_input_tokens: None,
        max_output_tokens: None,
        max_usd: None,
    }
}

#[test]
fn resolve_technical_changelog_auto_falls_back_to_release_body_when_changelog_section_missing() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(repo.path().join("CHANGELOG.md"), STALE_CHANGELOG).unwrap();
    fs::write(
        repo.path().join("release-body.md"),
        "the real 1.6.0 release body\n",
    )
    .unwrap();

    let args = synthesize_args_with_repo(repo.path(), "1.6.0");
    let mut config = test_synthesis_config();
    config.changelog_source = "auto".into();

    let technical = resolve_technical_changelog(&args, &config).expect("falls back cleanly");

    assert!(technical.contains("the real 1.6.0 release body"));
    assert!(!technical.contains("stale unrelated bugfix"));
}

#[test]
fn resolve_technical_changelog_explicit_changelog_source_fails_loudly_on_missing_section() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(repo.path().join("CHANGELOG.md"), STALE_CHANGELOG).unwrap();

    let args = synthesize_args_with_repo(repo.path(), "1.6.0");
    let mut config = test_synthesis_config();
    config.changelog_source = "changelog".into();

    let result = resolve_technical_changelog(&args, &config);

    assert!(
        result.is_err(),
        "expected a loud failure, not silent wrong-section synthesis"
    );
}

#[test]
fn resolve_technical_changelog_auto_uses_release_commits_when_changelog_is_stale() {
    let repo = fixture_release_repo("feat: add billing export");
    fs::write(repo.path().join("CHANGELOG.md"), STALE_CHANGELOG).unwrap();

    let args = synthesize_args_with_repo(repo.path(), "v1.1.0");
    let mut config = test_synthesis_config();
    config.changelog_source = "auto".into();

    let technical = resolve_technical_changelog(&args, &config).expect("grounded changelog");

    assert!(
        technical.contains("feat: add billing export"),
        "auto source should be grounded in the release commit range:\n{technical}"
    );
    assert!(
        !technical.contains("stale unrelated bugfix"),
        "stale changelog section must not silently ground synthesis:\n{technical}"
    );
}

#[test]
fn resolve_technical_changelog_auto_rejects_pr_text_that_does_not_match_release_commits() {
    let repo = fixture_release_repo("fix: recover canary check-ins");
    fs::write(
        repo.path().join("pr-changelog.md"),
        "- ancient unrelated dashboard rewrite (#41) by @octocat\n",
    )
    .unwrap();

    let args = synthesize_args_with_repo(repo.path(), "v1.1.0");
    let mut config = test_synthesis_config();
    config.changelog_source = "auto".into();

    let technical = resolve_technical_changelog(&args, &config).expect("grounded changelog");

    assert!(
        technical.contains("fix: recover canary check-ins"),
        "auto source should keep the release-scoped commit:\n{technical}"
    );
    assert!(
        !technical.contains("ancient unrelated dashboard rewrite"),
        "out-of-range PR text must not become synthesis ground truth:\n{technical}"
    );
}

#[test]
fn resolve_technical_changelog_explicit_source_keeps_mismatch_visible() {
    let repo = fixture_release_repo("fix: ship the actual release");
    fs::write(
        repo.path().join("CHANGELOG.md"),
        "## [1.1.0]\n\n- feat: stale unrelated dashboard rewrite\n",
    )
    .unwrap();

    let args = synthesize_args_with_repo(repo.path(), "v1.1.0");
    let mut config = test_synthesis_config();
    config.changelog_source = "changelog".into();

    let technical = resolve_technical_changelog(&args, &config).expect("explicit source retained");

    assert!(technical.contains("fix: ship the actual release"));
    assert!(technical.contains("feat: stale unrelated dashboard rewrite"));
    assert!(
        technical.contains("selected changelog source does not match release commits"),
        "explicit mismatch should be visible, not silently trusted:\n{technical}"
    );
    assert!(
        technical.contains("Selected technical source (changelog; does not match release commits)"),
        "selected source status should be rendered for model and operator review:\n{technical}"
    );
}

#[test]
fn classification_notice_is_collapsed_out_of_the_visible_release_body() {
    let notes = "## Improvements\n\n- Added a safer classifier.\n";
    let classification = ReleaseClassification {
        categories: vec!["user-visible".into()],
        significance: "medium".into(),
        user_visible: true,
        breaking: false,
        security: false,
        migration_heavy: false,
        source: "model".into(),
        model: "test/model".into(),
        deterministic_signals: vec!["conventional:feat".into()],
        disagreements: vec![
            "deterministic floor found user-visible commit signals but model did not".into(),
        ],
        reasons: Vec::new(),
    };

    let rendered = notes_with_classification_notice(notes, &classification);

    assert!(rendered.contains("<details>"));
    assert!(rendered.contains("</details>"));
    assert!(rendered.contains("Landmark classification notice"));
    // The published body up top should read as plain release notes, not debug output.
    assert!(rendered.starts_with("## Improvements"));
}

fn fixture_release_repo(release_subject: &str) -> tempfile::TempDir {
    let repo = tempfile::tempdir().unwrap();
    run_ok("git", ["init", "-q"], repo.path()).unwrap();
    run_ok("git", ["config", "user.name", "Landmark Test"], repo.path()).unwrap();
    run_ok(
        "git",
        ["config", "user.email", "landmark@example.invalid"],
        repo.path(),
    )
    .unwrap();
    fs::write(repo.path().join("README.md"), "# Fixture\n").unwrap();
    run_ok("git", ["add", "README.md"], repo.path()).unwrap();
    run_ok("git", ["commit", "-q", "-m", "chore: seed"], repo.path()).unwrap();
    run_ok("git", ["tag", "v1.0.0"], repo.path()).unwrap();

    fs::write(repo.path().join("release.txt"), release_subject).unwrap();
    run_ok("git", ["add", "release.txt"], repo.path()).unwrap();
    run_ok("git", ["commit", "-q", "-m", release_subject], repo.path()).unwrap();
    run_ok("git", ["tag", "v1.1.0"], repo.path()).unwrap();
    repo
}
