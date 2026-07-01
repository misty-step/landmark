use super::*;

#[test]
fn release_context_includes_commit_bodies_and_diff_stats() {
    let repo = fixture_repo_with_landmark_125_commits();
    let args = test_synthesize_args(repo.path(), "v1.25.0");
    let config = test_synthesis_config("balanced");

    let deterministic = deterministic_release_context(&args, &config);

    assert!(
        deterministic
            .commits
            .iter()
            .any(|commit| commit.body.contains("Feature body carried into context")),
        "{:?}",
        deterministic.commits
    );
    assert!(
        deterministic
            .diff_stats
            .iter()
            .any(|stat| stat.path == "src/run.rs" && stat.additions > 0),
        "{:?}",
        deterministic.diff_stats
    );
}

#[test]
fn release_classifier_uses_structured_commits_for_semantic_release_changelog() {
    let repo = fixture_repo_with_landmark_125_commits();
    let args = test_synthesize_args(repo.path(), "v1.25.0");
    let config = test_synthesis_config("balanced");
    let deterministic = deterministic_release_context(&args, &config);
    let technical = landmark_125_semantic_release_changelog();
    let sources = vec![context_source(
        "technical_changelog",
        "changelog",
        &technical,
    )];

    let classification =
        classify_release_context_with_deterministic(&technical, &sources, &deterministic);

    assert!(
        classification.user_visible,
        "classification should not miss semantic-release Features/Bug Fixes: {:?}",
        classification
    );
    assert_eq!(classification.significance, "medium");
    assert!(
        classification
            .deterministic_signals
            .iter()
            .any(|signal| signal == "conventional:feat"),
        "{:?}",
        classification
    );
    assert!(
        classification
            .reasons
            .iter()
            .any(|reason| reason.contains("parsed conventional commit")),
        "{:?}",
        classification
    );
}

#[test]
fn model_classifier_uses_commit_diff_context_and_preserves_floor() {
    let repo = fixture_repo_with_landmark_125_commits();
    let args = test_synthesize_args(repo.path(), "v1.25.0");
    let config = test_synthesis_config("balanced");
    let deterministic = deterministic_release_context(&args, &config);
    let technical = landmark_125_semantic_release_changelog();
    let sources = vec![context_source(
        "technical_changelog",
        "changelog",
        &technical,
    )];
    let server = start_fake_server(FakeState {
        llm_status: 200,
        llm_notes: json!({
            "categories": ["chore-only"],
            "significance": "low",
            "user_visible": false,
            "breaking": false,
            "security": false,
            "migration_heavy": false,
            "reasons": ["model treated the release as internal maintenance"]
        })
        .to_string(),
        update_status: 200,
        ..Default::default()
    })
    .unwrap();

    let classification = classify_release_context_with_model(
        &technical,
        &sources,
        &deterministic,
        &format!("{}/chat/completions", server.url),
        "test-key",
        &["test/model".into()],
    );

    assert_eq!(classification.source, "model");
    assert_eq!(classification.model, "test/model");
    assert!(classification.user_visible, "{classification:?}");
    assert_eq!(classification.significance, "medium");
    assert!(
        classification
            .disagreements
            .iter()
            .any(|disagreement| disagreement.contains("deterministic floor")),
        "{classification:?}"
    );
    assert!(
        classification
            .deterministic_signals
            .iter()
            .any(|signal| signal == "conventional:feat"),
        "{classification:?}"
    );

    let requests = server.state.lock().unwrap().requests.clone();
    let payload = request_payload(&requests, 0).unwrap();
    let classifier_context: Value =
        serde_json::from_str(payload["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(payload["model"], "test/model");
    assert!(
        classifier_context["deterministic"]["commits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|commit| commit["subject"] == "feat(run): emit release kit artifact graph"),
        "{classifier_context:#?}"
    );
    assert!(
        classifier_context["deterministic"]["diff_stats"]
            .as_array()
            .unwrap()
            .iter()
            .any(|stat| stat["path"] == "src/run.rs"),
        "{classifier_context:#?}"
    );
}

#[test]
fn dry_run_context_packet_does_not_call_model_classifier() {
    let repo = fixture_repo_with_landmark_125_commits();
    let mut args = test_synthesize_args(repo.path(), "v1.25.0");
    let config = test_synthesis_config("balanced");
    let technical = landmark_125_semantic_release_changelog();
    let server = start_fake_server(FakeState {
        llm_status: 200,
        llm_notes: json!({
            "categories": ["user-visible"],
            "significance": "medium",
            "user_visible": true,
            "breaking": false,
            "security": false,
            "migration_heavy": false,
            "reasons": ["would classify if called"]
        })
        .to_string(),
        update_status: 200,
        ..Default::default()
    })
    .unwrap();
    args.api_url = format!("{}/chat/completions", server.url);
    args.dry_run_cost = true;

    let context = synthesis_context_packet_with_model(&args, &config, &technical, "prompt");

    assert_eq!(context.classification.source, "structured");
    assert!(server.state.lock().unwrap().requests.is_empty());
}

#[test]
fn release_notes_include_classification_notice_for_disagreements() {
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

    assert!(rendered.contains("Landmark classification notice"));
    assert!(rendered.contains("conventional:feat"));
    assert!(rendered.contains("deterministic floor found user-visible"));
}

#[test]
fn classifier_keeps_workflow_manifest_cli_substrings_low_for_chore() {
    let deterministic = deterministic_context(
        vec![context_commit(
            "chore(ci): refresh workflow",
            "Touches manifest defaults and CLI examples.",
        )],
        vec![
            ".github/workflows/release.yml".into(),
            ".landmark.yml".into(),
        ],
    );
    let technical = "### Chores\n\n* refresh workflow for CLI manifest setup\n".to_string();
    let sources = vec![context_source(
        "technical_changelog",
        "changelog",
        &technical,
    )];

    let classification =
        classify_release_context_with_deterministic(&technical, &sources, &deterministic);

    assert!(!classification.user_visible, "{classification:?}");
    assert_eq!(classification.significance, "low");
    assert!(
        classification
            .deterministic_signals
            .iter()
            .any(|signal| signal == "conventional:chore"),
        "{classification:?}"
    );
    assert!(
        classification
            .categories
            .iter()
            .any(|category| category == "internal-tooling"),
        "{classification:?}"
    );
}

#[test]
fn classifier_recovers_conventional_floor_from_squash_body() {
    let deterministic = deterministic_context(
        vec![context_commit(
            "Merge pull request #42 from feature/import",
            "- feat(cli): add import wizard\n- fix(parser): handle CSV rows",
        )],
        vec!["src/import.rs".into(), "src/parser.rs".into()],
    );
    let technical =
        "### Features\n\n* add import wizard\n\n### Bug Fixes\n\n* handle CSV rows\n".to_string();
    let sources = vec![context_source(
        "technical_changelog",
        "changelog",
        &technical,
    )];

    let classification =
        classify_release_context_with_deterministic(&technical, &sources, &deterministic);

    assert!(classification.user_visible, "{classification:?}");
    assert_eq!(classification.significance, "medium");
    assert!(
        classification
            .deterministic_signals
            .iter()
            .any(|signal| signal == "conventional:feat"),
        "{classification:?}"
    );
    assert!(
        classification
            .deterministic_signals
            .iter()
            .any(|signal| signal == "conventional:fix"),
        "{classification:?}"
    );
}

#[test]
fn classifier_treats_perf_as_user_visible_floor_signal() {
    let deterministic = deterministic_context(
        vec![context_commit(
            "perf(api): cache release lookup",
            "Speeds up repeated release scans.",
        )],
        vec!["src/cache.rs".into()],
    );
    let technical = "### Performance\n\n* cache release lookup\n".to_string();
    let sources = vec![context_source(
        "technical_changelog",
        "changelog",
        &technical,
    )];

    let classification =
        classify_release_context_with_deterministic(&technical, &sources, &deterministic);

    assert!(classification.user_visible, "{classification:?}");
    assert!(
        classification
            .deterministic_signals
            .iter()
            .any(|signal| signal == "conventional:perf"),
        "{classification:?}"
    );
}

#[test]
fn classifier_handles_empty_context_without_panicking() {
    let classification = classify_release_context_with_deterministic(
        "",
        &[],
        &DeterministicReleaseContext::default(),
    );

    assert!(classification.user_visible, "{classification:?}");
    assert_eq!(classification.significance, "medium");
}

#[test]
fn classifier_defaults_non_conventional_commit_to_user_visible() {
    let deterministic = deterministic_context(
        vec![context_commit(
            "Add guided import wizard",
            "User-facing import flow without conventional commit syntax.",
        )],
        vec!["src/import.rs".into()],
    );
    let technical = "### Changes\n\n* Add guided import wizard\n".to_string();
    let sources = vec![context_source(
        "technical_changelog",
        "changelog",
        &technical,
    )];

    let classification =
        classify_release_context_with_deterministic(&technical, &sources, &deterministic);

    assert!(classification.user_visible, "{classification:?}");
    assert_eq!(classification.significance, "medium");
    assert!(classification.deterministic_signals.is_empty());
}

#[test]
fn classifier_handles_revert_subject_as_visible_release_event() {
    let deterministic = deterministic_context(
        vec![context_commit(
            "Revert \"feat(cli): add guided import wizard\"",
            "This reverts commit abc1234.",
        )],
        vec!["src/import.rs".into()],
    );
    let technical = "### Reverts\n\n* Revert \"feat(cli): add guided import wizard\"\n".to_string();
    let sources = vec![context_source(
        "technical_changelog",
        "changelog",
        &technical,
    )];

    let classification =
        classify_release_context_with_deterministic(&technical, &sources, &deterministic);

    assert!(classification.user_visible, "{classification:?}");
    assert_eq!(classification.significance, "medium");
}

#[test]
fn classifier_covers_rename_and_bootstrap_ranges_without_keyword_misfire() {
    let deterministic = deterministic_context(
        vec![
            context_commit(
                "refactor: rename release-kit package",
                "Internal crate rename without public operator action.",
            ),
            context_commit(
                "chore: bootstrap release workflow",
                "Initial workflow bootstrap for maintainers.",
            ),
        ],
        vec![
            "crates/landmark/src/release_kit.rs".into(),
            ".github/workflows/release.yml".into(),
        ],
    );
    let technical =
        "### Maintenance\n\n* rename release-kit package\n* bootstrap release workflow\n"
            .to_string();
    let sources = vec![context_source(
        "technical_changelog",
        "changelog",
        &technical,
    )];

    let classification =
        classify_release_context_with_deterministic(&technical, &sources, &deterministic);

    assert!(!classification.user_visible, "{classification:?}");
    assert_eq!(classification.significance, "low");
    assert!(
        classification
            .deterministic_signals
            .iter()
            .any(|signal| signal == "conventional:refactor"),
        "{classification:?}"
    );
    assert!(
        classification
            .deterministic_signals
            .iter()
            .any(|signal| signal == "conventional:chore"),
        "{classification:?}"
    );
}

fn deterministic_context(
    commits: Vec<ContextCommit>,
    changed_files: Vec<String>,
) -> DeterministicReleaseContext {
    DeterministicReleaseContext {
        commits,
        changed_files,
        ..Default::default()
    }
}

fn context_commit(subject: &str, body: &str) -> ContextCommit {
    ContextCommit {
        subject: subject.into(),
        body: body.into(),
        short_hash: "abc1234".into(),
        conventional_type: conventional_commit_type(subject).unwrap_or("").into(),
        breaking: body.contains("BREAKING CHANGE:"),
    }
}

fn test_synthesis_config(model_policy: &str) -> EffectiveSynthesisConfig {
    EffectiveSynthesisConfig {
        product_name: "Demo".into(),
        product_description: "Demo release automation.".into(),
        voice_guide: String::new(),
        audience: "developer".into(),
        changelog_source: "auto".into(),
        model_policy: model_policy.into(),
        model: "primary/model".into(),
        fallback_models: String::new(),
        max_input_tokens: None,
        max_output_tokens: None,
        max_usd: None,
    }
}

fn test_synthesize_args(repo: &Path, version: &str) -> SynthesizeArgs {
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
        release_body_file: repo.join("release.md"),
        pr_changelog_file: PathBuf::from("."),
        prompt_template: PathBuf::from("."),
        quality_file: repo.join("quality.txt"),
        attempts_file: PathBuf::from("."),
        templates_dir: PathBuf::from("templates/prompts"),
        repo_root: repo.to_path_buf(),
        dry_run_cost: false,
        context_metadata_file: PathBuf::from("."),
    }
}

fn fixture_repo_with_landmark_125_commits() -> tempfile::TempDir {
    let repo = tempfile::tempdir().unwrap();
    run_ok("git", ["init", "-q"], repo.path()).unwrap();
    run_ok("git", ["config", "user.name", "Landmark Test"], repo.path()).unwrap();
    run_ok(
        "git",
        ["config", "user.email", "landmark@example.invalid"],
        repo.path(),
    )
    .unwrap();
    fs::write(repo.path().join("README.md"), "# Landmark\n").unwrap();
    run_ok("git", ["add", "README.md"], repo.path()).unwrap();
    run_ok("git", ["commit", "-q", "-m", "chore: seed"], repo.path()).unwrap();
    run_ok("git", ["tag", "v1.24.0"], repo.path()).unwrap();

    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(repo.path().join("src/fleet.rs"), "pub fn fleet() {}\n").unwrap();
    run_ok("git", ["add", "src/fleet.rs"], repo.path()).unwrap();
    run_ok(
        "git",
        [
            "commit",
            "-q",
            "-m",
            "feat(fleet): deliver backfill-first adoption lane",
            "-m",
            "Feature body carried into context.",
        ],
        repo.path(),
    )
    .unwrap();

    fs::write(repo.path().join("src/run.rs"), "pub fn run() {}\n").unwrap();
    run_ok("git", ["add", "src/run.rs"], repo.path()).unwrap();
    run_ok(
        "git",
        [
            "commit",
            "-q",
            "-m",
            "feat(run): emit release kit artifact graph",
        ],
        repo.path(),
    )
    .unwrap();

    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join(".github/workflows/release.yml"),
        "name: Release\n",
    )
    .unwrap();
    run_ok("git", ["add", ".github/workflows/release.yml"], repo.path()).unwrap();
    run_ok(
        "git",
        [
            "commit",
            "-q",
            "-m",
            "fix(fleet): attach to existing release workflows",
        ],
        repo.path(),
    )
    .unwrap();
    repo
}

fn landmark_125_semantic_release_changelog() -> String {
    "# [1.25.0](https://github.com/misty-step/landmark/compare/v1.24.0...v1.25.0) (2026-06-25)\n\n### Features\n\n* **fleet:** deliver backfill-first adoption lane\n* **run:** emit release kit artifact graph\n\n### Bug Fixes\n\n* **fleet:** attach to existing release workflows\n".into()
}
