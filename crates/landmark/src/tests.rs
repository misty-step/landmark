use super::*;

#[test]
fn release_policy_blocks_degraded_required() {
    let path = temp_file("policy-test").unwrap();
    let args = PublicationArgs {
        synthesis_required: "true".into(),
        synthesis_strict: "false".into(),
        synth_succeeded: "true".into(),
        synth_quality: "degraded".into(),
        synth_failure_stage: "".into(),
        synth_failure_message: "".into(),
        github_output: path.clone(),
    };
    assert!(publication_policy(args).is_err());
    let outputs = parse_outputs(&path).unwrap();
    assert_eq!(outputs["can_update_release"], "false");
}

#[test]
fn release_body_replaces_existing_whats_new() {
    let body = compose_release_body(
        "## Better\n\n- New",
        "## What's New\n\nold\n\n## Technical\n\nraw",
    );
    assert!(body.contains("## Better"));
    assert!(!body.contains("old"));
    assert!(body.contains("## Technical"));
}

#[test]
fn markdown_filters_unsafe_links() {
    let html = markdown_to_html_fragment("[bad](javascript:alert(1)) [ok](https://example.com)");
    assert!(html.contains("href=\"#\""));
    assert!(html.contains("href=\"https://example.com\""));
}

#[test]
fn typed_artifact_renders_shared_outputs() {
    let artifact = ReleaseNoteArtifact::from_markdown(
        "v1.2.3",
        "## Added\n\n- See [docs](https://example.com) and [bad](javascript:alert(1))",
    );
    assert_eq!(artifact.version, "1.2.3");
    assert!(artifact.html.contains("href=\"https://example.com\""));
    assert!(artifact.html.contains("href=\"#\""));
    assert!(artifact.plaintext.contains("See docs and bad"));
    assert!(artifact.slack.contains("<https://example.com|docs>"));
    assert!(!artifact.slack.contains("javascript:"));
    assert_eq!(artifact.sections[0].title, "Added");
    assert_eq!(
        artifact.sections[0].bullets[0].links[0].href,
        "https://example.com"
    );
    assert!(artifact.json_entry()["sections"].is_array());
}

#[test]
fn next_release_tag_bumps_from_latest() {
    let latest = BackfillTag {
        tag: "v1.2.3".into(),
        version: "1.2.3".into(),
        key: (1, 2, 3),
        package: String::new(),
        prerelease: false,
    };
    assert_eq!(next_release_tag(Some(&latest), "minor"), "v1.3.0");
    assert_eq!(next_release_tag(Some(&latest), "patch"), "v1.2.4");
    assert_eq!(next_release_tag(Some(&latest), "major"), "v2.0.0");
}

#[test]
fn summary_status_includes_attempts_and_destinations() {
    let output = temp_file("summary-test").unwrap();
    let attempts = temp_file("attempts-test").unwrap();
    fs::write(
        &attempts,
        r#"[{"model":"primary","succeeded":false},{"model":"fallback","succeeded":true}]"#,
    )
    .unwrap();
    let args = SummaryArgs {
        synthesis_enabled: "true".into(),
        released: "true".into(),
        synth_succeeded: "true".into(),
        synth_quality: "valid".into(),
        update_succeeded: "true".into(),
        synth_failure_stage: "".into(),
        synth_failure_message: "".into(),
        update_failure_stage: "".into(),
        update_failure_message: "".into(),
        artifact_succeeded: "true".into(),
        artifact_failure_stage: "".into(),
        artifact_failure_message: "".into(),
        rss_enabled: "true".into(),
        rss_succeeded: "false".into(),
        rss_failure_stage: "rss_update".into(),
        rss_failure_message: "push failed".into(),
        webhook_enabled: "true".into(),
        webhook_sent: "true".into(),
        slack_enabled: "true".into(),
        slack_sent: "false".into(),
        github_output: output.clone(),
        attempts_file: attempts.to_string_lossy().into_owned(),
        context_metadata_file: ".".into(),
    };
    summary_policy(args).unwrap();
    let outputs = parse_outputs(&output).unwrap();
    assert_eq!(outputs["succeeded"], "false");
    let status: Value = serde_json::from_str(&outputs["status_json"]).unwrap();
    assert_eq!(status["model_attempts"].as_array().unwrap().len(), 2);
    assert_eq!(status["destinations"]["rss"]["enabled"], true);
    assert_eq!(status["destinations"]["rss"]["failure_stage"], "rss_update");
    assert_eq!(status["destinations"]["webhook"]["succeeded"], true);
    assert_eq!(status["destinations"]["slack"]["succeeded"], false);
}

#[test]
fn summary_no_release_accepts_empty_artifact_paths() {
    let output = temp_file("summary-no-release").unwrap();
    let cli = Cli::try_parse_from([
        "landmark",
        "release-policy",
        "summary",
        "--synthesis-enabled",
        "true",
        "--released",
        "false",
        "--synth-succeeded",
        "",
        "--update-succeeded",
        "",
        "--github-output",
        output.to_str().unwrap(),
        "--attempts-file",
        "",
        "--context-metadata-file",
        "",
    ])
    .unwrap();
    let Commands::ReleasePolicy(ReleasePolicyArgs {
        command: ReleasePolicyCommand::Summary(args),
    }) = cli.command
    else {
        panic!("expected release-policy summary command");
    };

    summary_policy(*args).unwrap();

    let outputs = parse_outputs(&output).unwrap();
    assert_eq!(outputs["succeeded"], "true");
    assert_eq!(outputs["failure_stage"], "");
    let status: Value = serde_json::from_str(&outputs["status_json"]).unwrap();
    assert_eq!(status["released"], false);
    assert_eq!(status["model_attempts"].as_array().unwrap().len(), 0);
    assert_eq!(status["context"], json!({}));
}

#[test]
fn setup_detects_changesets_monorepo_and_generates_matrix_workflow() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir(repo.path().join(".changeset")).unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{"name":"demo","workspaces":["packages/*"]}"#,
    )
    .unwrap();
    let diagnosis = diagnose_setup(repo.path());
    assert_eq!(diagnosis.release_tool, "changesets");
    assert!(diagnosis.monorepo);
    let recommendation = recommend_setup(&diagnosis, None);
    assert_eq!(recommendation.workflow, "changesets-monorepo");
    let workflows = setup_workflows(&diagnosis, None);
    let changesets = &workflows["changesets"].content;
    assert!(changesets.contains("fromJson(needs.release.outputs.published_packages)[0].version"));
    assert!(!changesets.contains("${{tag}}"));
    assert!(!changesets.contains("python3"));
    let workflow = &workflows["changesets-monorepo"].content;
    assert!(workflow.contains("strategy:"));
    assert!(workflow.contains("healthcheck: 'true'"));
    assert!(workflow.contains("pull-requests: write"));
    assert!(workflow.contains("NPM_TOKEN"));
}

#[test]
fn setup_detects_semantic_release_and_reports_backfill_available() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{"name":"demo","devDependencies":{"semantic-release":"^24.0.0"}}"#,
    )
    .unwrap();
    let diagnosis = diagnose_setup(repo.path());
    assert_eq!(diagnosis.release_tool, "semantic-release");
    assert_eq!(recommend_setup(&diagnosis, None).mode, "full");
    let workflow = &setup_workflows(&diagnosis, None)["semantic-release"].content;
    assert!(workflow.contains("mode: full"));
    assert!(workflow.contains("healthcheck: 'true'"));
    assert!(workflow.contains("GH_RELEASE_TOKEN"));
}

#[test]
fn fleet_plan_patches_existing_release_please_workflow() {
    let mut repo = fleet_fixture_repo(
        "misty-step/release-please-app",
        "release-please",
        ("application", "github-release"),
        (false, false),
        "unprotected-or-unavailable",
        &[],
        &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
    );
    repo.workflow_files = vec![
        fleet_workflow_file(
            ".github/workflows/release-please.yml",
            &existing_release_please_workflow(),
        )
        .expect("release-please workflow fixture"),
    ];

    let plan = plan_fleet_repository(&repo);

    assert_eq!(plan.status, "ready");
    assert_eq!(plan.workflow_patches.len(), 1);
    let patch = &plan.workflow_patches[0];
    assert_eq!(patch.path, ".github/workflows/release-please.yml");
    serde_yaml::from_str::<serde_yaml::Value>(&patch.content).unwrap();
    assert!(patch.content.contains("Existing Release"));
    assert_eq!(
        patch
            .content
            .matches("googleapis/release-please-action")
            .count(),
        1
    );
    assert!(patch.content.contains("needs: release-please"));
    assert!(patch.content.contains("mode: synthesis-only"));
    assert!(patch.content.contains("healthcheck: 'true'"));
}

#[test]
fn fleet_plan_patches_existing_changesets_workflow() {
    let mut repo = fleet_fixture_repo(
        "misty-step/changesets-app",
        "changesets",
        ("library", "github-release"),
        (false, false),
        "unprotected-or-unavailable",
        &[],
        &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
    );
    repo.workflow_files = vec![
        fleet_workflow_file(
            ".github/workflows/changesets.yml",
            &existing_changesets_workflow(),
        )
        .expect("changesets workflow fixture"),
    ];

    let plan = plan_fleet_repository(&repo);

    assert_eq!(plan.status, "ready");
    assert_eq!(plan.workflow_patches.len(), 1);
    let patch = &plan.workflow_patches[0];
    assert_eq!(patch.path, ".github/workflows/changesets.yml");
    serde_yaml::from_str::<serde_yaml::Value>(&patch.content).unwrap();
    assert!(patch.content.contains("Existing Release"));
    assert_eq!(patch.content.matches("changesets/action").count(), 1);
    assert!(patch.content.contains("needs: release"));
    assert!(patch.content.contains("mode: synthesis-only"));
    assert!(patch.content.contains("healthcheck: 'true'"));
}

#[test]
fn fleet_plan_blocks_existing_semantic_release_workflow() {
    let plan = plan_fleet_repository(&fleet_existing_semantic_release_workflow_fixture());

    assert_eq!(plan.status, "blocked");
    assert_eq!(plan.integration_mode, "blocked");
    assert!(plan.workflow_patches.is_empty());
    assert!(
        plan.skip_reason
            .contains("existing semantic-release workflow")
    );
}

#[test]
fn fleet_plan_blocks_secret_like_workflow_bodies() {
    let mut repo = fleet_fixture_repo(
        "misty-step/release-please-app",
        "release-please",
        ("application", "github-release"),
        (false, false),
        "unprotected-or-unavailable",
        &[],
        &["GH_RELEASE_TOKEN", "OPENROUTER_API_KEY"],
    );
    let mut workflow = existing_release_please_workflow();
    workflow.push_str("\n# ghp_1234567890abcdef\n");
    repo.workflow_files = vec![
        fleet_workflow_file(".github/workflows/release.yml", &workflow)
            .expect("release-please workflow fixture"),
    ];

    let plan = plan_fleet_repository(&repo);

    assert_eq!(plan.status, "blocked");
    assert!(plan.workflow_patches.is_empty());
    assert!(plan.skip_reason.contains("secret-like literals"));
    assert!(repo.workflow_files[0].content.is_empty());
    assert!(repo.workflow_files[0].content_redacted);
}

#[test]
fn fleet_plan_readies_backfill_first_no_release_package_repositories() {
    for (name, kind, packages) in [
        (
            "phrazzld/no-release-ts-app",
            "application",
            vec!["package.json"],
        ),
        (
            "misty-step/no-release-rust-crate",
            "library",
            vec!["Cargo.toml"],
        ),
        ("phrazzld/no-release-go-app", "application", vec!["go.mod"]),
        (
            "misty-step/no-release-python-lib",
            "library",
            vec!["pyproject.toml"],
        ),
        (
            "phrazzld/no-release-multipackage",
            "library",
            vec!["Cargo.toml", "package.json", "packages/api/package.json"],
        ),
    ] {
        let repo = fleet_fixture_repo_with_packages(
            name,
            "no-release-tool",
            (kind, "none"),
            (false, false),
            "unprotected-or-unavailable",
            &packages,
            &[],
        );

        let plan = plan_fleet_repository(&repo);

        assert_eq!(plan.status, "ready");
        assert_eq!(plan.recommended_mode, "backfill-first");
        assert_eq!(plan.integration_mode, "backfill-first");
        assert!(plan.required_secrets.is_empty());
        assert!(plan.missing_secrets.is_empty());
        assert_eq!(plan.initial_version_recommendation, "0.1.0");
        assert!(!plan.initial_tag_recommendation.is_empty());
        if name == "phrazzld/no-release-multipackage" {
            assert_eq!(
                plan.initial_tag_recommendation,
                "no-release-multipackage@0.1.0"
            );
        } else {
            assert_eq!(plan.initial_tag_recommendation, "v0.1.0");
        }
        assert!(
            plan.artifact_paths
                .iter()
                .any(|path| path == "docs/releases/{version}.md")
        );
        assert!(
            plan.historical_preview_command
                .contains("--mode artifacts-only --dry-run")
        );
        assert!(
            plan.historical_preview_command
                .contains(&plan.initial_tag_recommendation)
        );
        assert!(plan.rollback_guidance.contains("close the PR"));
        assert!(
            plan.rollback_guidance
                .contains("previewed local artifact files")
        );
        assert!(
            plan.migration_notes
                .iter()
                .any(|note| note.contains("operator-approved initial tag"))
        );
        assert_eq!(
            plan.manifest.release.profile.as_deref(),
            Some("synthesis-only")
        );
    }
}

#[test]
fn fleet_plan_keeps_no_package_no_release_repositories_skipped() {
    let repo = fleet_fixture_repo_with_packages(
        "misty-step/docs-site",
        "no-release-tool",
        ("non-release", "none"),
        (false, false),
        "unprotected-or-unavailable",
        &[],
        &[],
    );

    let plan = plan_fleet_repository(&repo);

    assert_eq!(plan.status, "skipped");
    assert_eq!(plan.recommended_mode, "skipped");
    assert!(plan.initial_version_recommendation.is_empty());
    assert!(plan.initial_tag_recommendation.is_empty());
    assert!(plan.artifact_paths.is_empty());
    assert!(plan.historical_preview_command.is_empty());
}

#[test]
fn fleet_backfill_guidance_helpers_handle_empty_and_tag_variants() {
    let mut repo = fleet_fixture_repo_with_packages(
        "misty-step/library",
        "no-release-tool",
        ("library", "none"),
        (false, false),
        "unprotected-or-unavailable",
        &["Cargo.toml"],
        &[],
    );

    assert!(fleet_initial_version("skipped", "skipped").is_empty());
    assert!(fleet_initial_tag(&repo, "").is_empty());
    assert!(fleet_historical_preview_command("").is_empty());

    repo.tag_format = "{version}".into();
    assert_eq!(fleet_initial_tag(&repo, "0.1.0"), "0.1.0");
    repo.tag_format = "package@{version}".into();
    assert_eq!(fleet_initial_tag(&repo, "0.1.0"), "library@0.1.0");
    repo.tag_format = "custom".into();
    assert_eq!(
        fleet_initial_tag(&repo, "0.1.0"),
        "0.1.0 (custom tag format requires operator approval)"
    );
}

#[test]
fn org_secret_visibility_all_counts_for_fleet_secret_metadata() {
    let metadata = json!({
        "secrets": [
            {"name": "GH_RELEASE_TOKEN", "visibility": "all"},
            {"name": "OPENROUTER_API_KEY", "visibility": "selected", "selected_repositories": [
                {"full_name": "misty-step/landmark", "name": "landmark"}
            ]},
            {"name": "UNRELATED", "visibility": "private"}
        ]
    });

    let names = org_secret_names_for_repo(&metadata, "misty-step/landmark", "landmark");
    assert!(names.contains("GH_RELEASE_TOKEN"));
    assert!(names.contains("OPENROUTER_API_KEY"));
    assert!(!names.contains("UNRELATED"));

    let other = org_secret_names_for_repo(&metadata, "misty-step/other", "other");
    assert!(other.contains("GH_RELEASE_TOKEN"));
    assert!(!other.contains("OPENROUTER_API_KEY"));
}

#[test]
fn fleet_detects_landmark_and_legacy_landmark_release_workflow_content() {
    for action_ref in ["misty-step/landmark@v1", "misty-step/landmark@v1"] {
        let workflow_texts = vec![(
            "release.yml".to_string(),
            format!("steps:\n  - uses: {action_ref}\n"),
        )];

        assert!(workflow_invokes_landmark(&workflow_texts[0].1));
        assert_eq!(
            fleet_release_tool(
                &[],
                &["release.yml".into()],
                &workflow_texts,
                &["v1.2.3".into()]
            ),
            "manual-tag"
        );
    }
}

#[test]
fn fleet_classifiers_distinguish_rollout_kinds_and_surfaces() {
    assert_eq!(
        classify_fleet_repository_kind("release-docs", &[], &[]),
        "non-release"
    );
    assert_eq!(
        classify_fleet_repository_kind("terraform-infra", &["go.mod".into()], &[]),
        "infrastructure"
    );
    assert_eq!(
        classify_fleet_repository_kind("search-experiment", &["Cargo.toml".into()], &[]),
        "experiment"
    );
    assert_eq!(
        classify_fleet_repository_kind("widget-crate", &["Cargo.toml".into()], &[]),
        "library"
    );
    assert_eq!(
        classify_fleet_repository_kind("billing-app", &["package.json".into()], &[]),
        "application"
    );
    assert_eq!(
        classify_fleet_release_surface(
            "changesets",
            &[],
            &[("release.yml".into(), "npm publish".into())],
        ),
        "package-registry"
    );
    assert_eq!(
        classify_fleet_release_surface("semantic-release", &[], &[]),
        "github-release+semantic-release"
    );
    assert_eq!(
        classify_fleet_release_surface("no-release-tool", &[], &[]),
        "none"
    );
}

#[test]
fn init_manifest_infers_product_context_from_repo_metadata() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{"name":"@mistystep/atlas","description":"Release operations for app fleets."}"#,
    )
    .unwrap();
    fs::write(
        repo.path().join("README.md"),
        "# Atlas\n\nLandmark-managed release automation.\n",
    )
    .unwrap();

    let manifest = infer_manifest(repo.path());
    assert_eq!(manifest.product.name.as_deref(), Some("Atlas"));
    assert_eq!(
        manifest.product.description.as_deref(),
        Some("Release operations for app fleets.")
    );
    assert_eq!(manifest.audience.as_deref(), Some("developer"));
    assert_eq!(manifest.changelog.source.as_deref(), Some("auto"));

    let rendered = render_manifest_yaml(&manifest).unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_str(&rendered).unwrap();
    assert_eq!(parsed["product"]["name"], "Atlas");
    assert_eq!(parsed["model"]["policy"], "balanced");
}

#[test]
fn setup_projects_manifest_defaults_into_generated_workflows() {
    let diagnosis = SetupDiagnosis {
        release_tool: "semantic-release".into(),
        default_branch: "master".into(),
        tag_format: "v{version}".into(),
        conventional_commits: "ready".into(),
        monorepo: false,
        packages: vec!["landmark".into()],
        signals: Vec::new(),
    };
    let manifest = LandmarkManifest {
        product: ProductManifest {
            name: Some("Landmark".into()),
            description: Some("Release notes and changelog automation.".into()),
        },
        audience: Some("enterprise".into()),
        voice: Some("plainspoken, specific, operator-facing".into()),
        changelog: ChangelogManifest {
            source: Some("release-body".into()),
        },
        artifacts: ArtifactManifest {
            markdown: Some("docs/releases/{version}.md".into()),
            plaintext: None,
            html: Some("docs/releases/{version}.html".into()),
            json: None,
            rss: Some("docs/releases/feed.xml".into()),
        },
        release: ReleaseManifest {
            profile: Some("full".into()),
        },
        model: ModelManifest {
            policy: Some("cheap".into()),
            primary: Some("openai/gpt-4o-mini".into()),
            fallbacks: vec!["google/gemini-2.5-flash".into()],
        },
        budget: BudgetManifest {
            max_input_tokens: Some(8000),
            max_output_tokens: Some(900),
            max_usd: Some(0.05),
        },
    };

    let workflows = setup_workflows(&diagnosis, Some(&manifest));
    let workflow = &workflows["semantic-release"].content;
    assert!(workflow.contains("product-description: Release notes and changelog automation."));
    assert!(workflow.contains("audience: enterprise"));
    assert!(workflow.contains("voice-guide: plainspoken, specific, operator-facing"));
    assert!(workflow.contains("changelog-source: release-body"));
    assert!(workflow.contains("notes-output-file: docs/releases/{version}.md"));
    assert!(workflow.contains("notes-output-html-file: docs/releases/{version}.html"));
    assert!(workflow.contains("rss-feed-file: docs/releases/feed.xml"));
    assert!(workflow.contains("llm-model: openai/gpt-4o-mini"));
    assert!(workflow.contains("llm-fallback-models: google/gemini-2.5-flash"));
    let release_please = &workflows["release-please"].content;
    assert_eq!(release_please.matches("changelog-source:").count(), 1);
    assert!(release_please.contains("changelog-source: release-body"));

    let mut synthesis_only_manifest = manifest.clone();
    synthesis_only_manifest.release.profile = Some("synthesis-only".into());
    let recommendation = recommend_setup(&diagnosis, Some(&synthesis_only_manifest));
    assert_eq!(recommendation.mode, "synthesis-only");
    assert!(
        recommendation
            .rationale
            .contains(&"manifest release profile: synthesis-only".into())
    );
}

#[test]
fn synthesis_manifest_defaults_keep_explicit_cli_precedence() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join(".landmark.yml"),
        r#"product:
  name: Manifest Product
  description: Manifest description
audience: enterprise
voice: Manifest voice
changelog:
  source: release-body
model:
  policy: cheap
  primary: manifest/model
  fallbacks:
    - manifest/fallback
"#,
    )
    .unwrap();
    let mut args = SynthesizeArgs {
        api_key: "test".into(),
        model: String::new(),
        model_policy: String::new(),
        api_url: "http://example.invalid".into(),
        fallback_models: String::new(),
        product_name: String::new(),
        product_description: String::new(),
        voice_guide: String::new(),
        audience: None,
        changelog_source: None,
        version: "v1.2.3".into(),
        changelog_file: repo.path().join("CHANGELOG.md"),
        release_body_file: repo.path().join("release.md"),
        pr_changelog_file: PathBuf::from("."),
        prompt_template: PathBuf::from("."),
        quality_file: repo.path().join("quality.txt"),
        attempts_file: PathBuf::from("."),
        templates_dir: PathBuf::from("templates/prompts"),
        repo_root: repo.path().to_path_buf(),
        dry_run_cost: false,
        context_metadata_file: PathBuf::from("."),
    };
    let defaults = resolve_synthesis_config(&args).unwrap();
    assert_eq!(defaults.product_name, "Manifest Product");
    assert_eq!(defaults.product_description, "Manifest description");
    assert_eq!(defaults.voice_guide, "Manifest voice");
    assert_eq!(defaults.audience, "enterprise");
    assert_eq!(defaults.changelog_source, "release-body");
    assert_eq!(defaults.model, "manifest/model");
    assert_eq!(defaults.fallback_models, "manifest/fallback");

    args.audience = Some("developer".into());
    args.changelog_source = Some("prs".into());
    args.product_description = "Explicit description".into();
    args.model = "explicit/model".into();
    let explicit = resolve_synthesis_config(&args).unwrap();
    assert_eq!(explicit.product_description, "Explicit description");
    assert_eq!(explicit.audience, "developer");
    assert_eq!(explicit.changelog_source, "prs");
    assert_eq!(explicit.model, "explicit/model");

    args.model = String::new();
    args.audience = None;
    args.changelog_source = None;
    let mut manifest: LandmarkManifest =
        serde_yaml::from_str(&fs::read_to_string(repo.path().join(".landmark.yml")).unwrap())
            .unwrap();
    manifest.model.primary = None;
    manifest.model.policy = Some("rich".into());
    fs::write(
        repo.path().join(".landmark.yml"),
        render_manifest_yaml(&manifest).unwrap(),
    )
    .unwrap();
    let policy_default = resolve_synthesis_config(&args).unwrap();
    assert_eq!(policy_default.model, "anthropic/claude-sonnet-5");
}

#[test]
fn release_classifier_defaults_unknown_context_to_user_visible_medium() {
    let classification = classify_release_context_from_text(
        "## [1.2.3]\n\n- improve output\n",
        &[],
        "rendered-text",
    );

    assert!(classification.user_visible);
    assert_eq!(classification.significance, "medium");
    assert!(
        classification
            .categories
            .iter()
            .any(|category| category == "user-visible")
    );
}

#[test]
fn synthesis_budget_preserves_existing_policy_skip_reason() {
    let config = test_synthesis_config("off", Some(1), Some(0.0));
    let classification = test_release_classification("medium");
    let cost = estimate_synthesis_cost(&config, "a long enough prompt", &classification, &[]);

    assert!(cost.skip);
    assert_eq!(cost.skip_reason, "model.policy=off disables LLM synthesis");
}

#[test]
fn synthesis_budget_counts_rendered_prompt_once() {
    let prompt = "abcd".repeat(100);
    let max_input_tokens = estimate_tokens(&prompt);
    let config = test_synthesis_config("balanced", Some(max_input_tokens), None);
    let classification = test_release_classification("medium");
    let prompt_source = ContextSource {
        name: "prompt_template".into(),
        kind: "prompt".into(),
        bytes: prompt.len(),
        estimated_tokens: max_input_tokens,
        included: true,
    };

    let cost = estimate_synthesis_cost(&config, &prompt, &classification, &[prompt_source]);

    assert!(!cost.skip, "{:?}", cost);
    assert_eq!(cost.input_tokens, max_input_tokens);
}

#[test]
fn manifest_validation_rejects_multiline_action_scalars() {
    let manifest = LandmarkManifest {
        product: ProductManifest {
            name: Some("Demo".into()),
            description: Some("first line\nsecond line".into()),
        },
        audience: Some("developer".into()),
        voice: Some("clear".into()),
        changelog: ChangelogManifest {
            source: Some("auto".into()),
        },
        artifacts: ArtifactManifest::default(),
        release: ReleaseManifest::default(),
        model: ModelManifest::default(),
        budget: BudgetManifest::default(),
    };
    let errors = validate_manifest(&manifest);
    assert!(errors.iter().any(|error| error.contains("single-line")));
}

#[test]
fn manifest_validation_rejects_unsupported_policy_and_profile() {
    let manifest = LandmarkManifest {
        product: ProductManifest {
            name: Some("Demo".into()),
            description: Some("Demo app".into()),
        },
        audience: Some("developer".into()),
        voice: Some("clear".into()),
        changelog: ChangelogManifest {
            source: Some("auto".into()),
        },
        artifacts: ArtifactManifest::default(),
        release: ReleaseManifest {
            profile: Some("banana".into()),
        },
        model: ModelManifest {
            policy: Some("banana".into()),
            primary: None,
            fallbacks: Vec::new(),
        },
        budget: BudgetManifest::default(),
    };
    let errors = validate_manifest(&manifest);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("release.profile must be full or synthesis-only"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("model.policy must be cheap, balanced, rich, or off"))
    );
}

#[test]
fn manifest_shape_rejects_unknown_keys() {
    let raw: serde_yaml::Value = serde_yaml::from_str(
        "product:\n  name: Demo\n  description: Demo app\n  tagline: nope\nrelease:\n  profile: synthesis-only\nsurprise: true\n",
    )
    .unwrap();
    let errors = validate_manifest_yaml_shape(&raw);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("manifest contains unknown key `surprise`"))
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("manifest.product contains unknown key `tagline`"))
    );
}

#[test]
fn failure_classifier_emits_stable_codes_and_redacts_tokens() {
    let auth = classify_failure("--publish-release-body requires --github-token");
    assert_eq!(auth.code, "provider_auth");
    assert_eq!(auth.stage, "provider");
    assert!(!auth.retryable);

    let changelog = classify_failure("manifest changelog.source must be auto");
    assert_eq!(changelog.code, "invalid_changelog_source");
    assert_eq!(changelog.stage, "configuration");

    let redacted = redact_context("request failed with ghp_123456789abcdef and sk-123456789abcdef");
    assert!(!redacted.contains("ghp_123456789abcdef"));
    assert!(!redacted.contains("sk-123456789abcdef"));
    assert!(redacted.contains("[REDACTED]"));
}

/// classify_failure is a first-match if/else-if chain over substring checks,
/// so branch order matters: an added or reordered branch could silently
/// steal matches from another branch without any test failing unless every
/// branch has a message that hits it and only it. Each message below is
/// chosen to avoid every earlier branch's trigger words.
#[test]
fn failure_classifier_covers_remaining_branches() {
    let provider_outage = classify_failure("HTTP 429 too many requests from provider");
    assert_eq!(provider_outage.code, "provider_outage");
    assert_eq!(provider_outage.stage, "provider");
    assert!(provider_outage.retryable);

    let budget_skip = classify_failure("synthesis skipped: budget exceeded for this release");
    assert_eq!(budget_skip.code, "budget_skip");
    assert_eq!(budget_skip.stage, "synthesis");
    assert!(!budget_skip.retryable);

    let synthesis_degradation =
        classify_failure("synthesis quality degraded; using unvalidated output");
    assert_eq!(synthesis_degradation.code, "synthesis_degradation");
    assert_eq!(synthesis_degradation.stage, "synthesis");
    assert!(!synthesis_degradation.retryable);

    let publication_mutation_failure =
        classify_failure("landmark could not update the release body; release remains published");
    assert_eq!(
        publication_mutation_failure.code,
        "publication_mutation_failure"
    );
    assert_eq!(publication_mutation_failure.stage, "publication");
    assert!(publication_mutation_failure.retryable);

    let feed_failure = classify_failure("could not update the rss release feed");
    assert_eq!(feed_failure.code, "feed_failure");
    assert_eq!(feed_failure.stage, "artifact");
    assert!(!feed_failure.retryable);

    let artifact_write_failure =
        classify_failure("failed to write technical changelog output: permission denied");
    assert_eq!(artifact_write_failure.code, "artifact_write_failure");
    assert_eq!(artifact_write_failure.stage, "artifact");
    assert!(!artifact_write_failure.retryable);

    let invalid_input = classify_failure(
        "unsupported provider 'foo'; this build supports provider=local or provider=github",
    );
    assert_eq!(invalid_input.code, "invalid_input");
    assert_eq!(invalid_input.stage, "configuration");
    assert!(!invalid_input.retryable);

    let command_failed = classify_failure("unexpected git subprocess exit status 128");
    assert_eq!(command_failed.code, "command_failed");
    assert_eq!(command_failed.stage, "runtime");
    assert!(!command_failed.retryable);
}

fn test_synthesis_config(
    model_policy: &str,
    max_input_tokens: Option<u64>,
    max_usd: Option<f64>,
) -> EffectiveSynthesisConfig {
    EffectiveSynthesisConfig {
        product_name: "Demo".into(),
        product_description: "Demo release automation.".into(),
        voice_guide: String::new(),
        audience: "developer".into(),
        changelog_source: "auto".into(),
        model_policy: model_policy.into(),
        model: "primary/model".into(),
        fallback_models: String::new(),
        max_input_tokens,
        max_output_tokens: None,
        max_usd,
    }
}

fn test_release_classification(significance: &str) -> ReleaseClassification {
    ReleaseClassification {
        categories: vec!["user-visible".into()],
        significance: significance.into(),
        user_visible: true,
        breaking: false,
        security: false,
        migration_heavy: false,
        source: "test".into(),
        model: String::new(),
        deterministic_signals: Vec::new(),
        disagreements: Vec::new(),
        reasons: Vec::new(),
    }
}

#[test]
fn setup_generated_workflows_are_yaml() {
    let diagnosis = SetupDiagnosis {
        release_tool: "manual-tag".into(),
        default_branch: "main".into(),
        tag_format: "v{version}".into(),
        conventional_commits: "ready".into(),
        monorepo: true,
        packages: vec!["pkg-a".into(), "pkg-b".into()],
        signals: Vec::new(),
    };
    for candidate in setup_workflows(&diagnosis, None).values() {
        let parsed: serde_yaml::Value = serde_yaml::from_str(&candidate.content).unwrap();
        assert!(parsed["jobs"].is_mapping(), "{}", candidate.path);
    }
    let manual = &setup_workflows(&diagnosis, None)["manual-tag"].content;
    assert!(manual.contains("release:\n    types: [published]"));
    assert!(!manual.contains("push:\n    tags:"));
    assert!(manual.contains("${{ github.event.release.tag_name }}"));
    assert!(manual.contains("${{ secrets.GH_RELEASE_TOKEN }}"));
}

#[test]
fn bump_version_applies_each_bump_kind() {
    assert_eq!(bump_version("1.2.3", VersionBump::Major).unwrap(), "2.0.0");
    assert_eq!(bump_version("1.2.3", VersionBump::Minor).unwrap(), "1.3.0");
    assert_eq!(bump_version("1.2.3", VersionBump::Patch).unwrap(), "1.2.4");
}

#[test]
fn self_release_commits_skip_release_commits_and_keep_the_rest() {
    let repo = tempfile::tempdir().unwrap();
    let path = repo.path();
    run_ok("git", ["init", "-q"], path).unwrap();
    run_ok("git", ["config", "user.name", "Test"], path).unwrap();
    run_ok(
        "git",
        ["config", "user.email", "test@example.invalid"],
        path,
    )
    .unwrap();
    fs::write(path.join("f"), "1").unwrap();
    run_ok("git", ["add", "."], path).unwrap();
    run_ok("git", ["commit", "-q", "-m", "chore: seed"], path).unwrap();
    run_ok("git", ["tag", "v1.0.0"], path).unwrap();
    fs::write(path.join("f"), "2").unwrap();
    run_ok("git", ["add", "."], path).unwrap();
    run_ok("git", ["commit", "-q", "-m", "feat(x): add thing"], path).unwrap();
    run_ok(
        "git",
        [
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "chore(release): 1.1.0",
        ],
        path,
    )
    .unwrap();
    let commits = self_release_commits(path, "v1.0.0").unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].subject, "feat(x): add thing");
    let changelog_commits = release_worthy_commits(&commits);
    assert_eq!(changelog_commits.len(), 1);
    assert_eq!(changelog_commits[0].category, "features");
}

#[test]
fn changelog_section_extracts_only_requested_version() {
    let repo = tempfile::tempdir().unwrap();
    let path = repo.path().join("CHANGELOG.md");
    fs::write(
        &path,
        "# [1.2.0](compare) (2026-06-12)\n\n### Features\n\n* new\n\n## [1.1.0](compare) (2026-06-11)\n\n### Bug Fixes\n\n* old\n",
    )
    .unwrap();
    let section = changelog_section(&path, "1.2.0").unwrap();
    assert!(section.contains("* new"));
    assert!(!section.contains("* old"));
}

#[test]
fn cargo_lock_version_update_targets_landmark_package_only() {
    let repo = tempfile::tempdir().unwrap();
    let path = repo.path().join("Cargo.lock");
    fs::write(
        &path,
        "[[package]]\nname = \"dep\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"landmark\"\nversion = \"1.2.3\"\n",
    )
    .unwrap();
    update_lock_package_version(&path, "landmark", "1.3.0").unwrap();
    let text = fs::read_to_string(path).unwrap();
    assert!(text.contains("name = \"dep\"\nversion = \"0.1.0\""));
    assert!(text.contains("name = \"landmark\"\nversion = \"1.3.0\""));
}

#[test]
fn version_sync_allows_explicit_release_candidate() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("crates/landmark")).unwrap();
    run_ok("git", ["init", "-q"], repo.path()).unwrap();
    run_ok("git", ["config", "user.name", "Landmark Test"], repo.path()).unwrap();
    run_ok(
        "git",
        ["config", "user.email", "landmark@example.invalid"],
        repo.path(),
    )
    .unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{"name":"landmark","version":"1.18.0"}"#,
    )
    .unwrap();
    fs::write(
        repo.path().join("crates/landmark/Cargo.toml"),
        "[package]\nname = \"landmark\"\nversion = \"1.18.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("CHANGELOG.md"),
        "# [1.18.0](compare) (2026-06-12)\n\n### Features\n\n* release candidate\n",
    )
    .unwrap();
    run_ok("git", ["add", "."], repo.path()).unwrap();
    run_ok(
        "git",
        ["commit", "-q", "-m", "chore: candidate"],
        repo.path(),
    )
    .unwrap();
    run_ok("git", ["tag", "v1.17.2"], repo.path()).unwrap();
    let strict = CheckVersionArgs {
        reference: "HEAD".into(),
        repo_root: repo.path().to_path_buf(),
        allow_release_candidate: false,
    };
    assert!(check_version_sync(strict).is_err());
    let candidate = CheckVersionArgs {
        reference: "HEAD".into(),
        repo_root: repo.path().to_path_buf(),
        allow_release_candidate: true,
    };
    assert!(check_version_sync(candidate).is_ok());

    fs::write(
        repo.path().join("CHANGELOG.md"),
        "# [1.18.0](compare) (2026-06-12)\n\n### Features\n\n",
    )
    .unwrap();
    let missing_entry = CheckVersionArgs {
        reference: "HEAD".into(),
        repo_root: repo.path().to_path_buf(),
        allow_release_candidate: true,
    };
    assert!(check_version_sync(missing_entry).is_err());

    fs::write(
        repo.path().join("CHANGELOG.md"),
        "# [1.18.0](compare) (2026-06-12)\n\n### Features\n\n* release candidate\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("crates/landmark/Cargo.toml"),
        "[package]\nname = \"landmark\"\nversion = \"1.17.9\"\nedition = \"2024\"\n",
    )
    .unwrap();
    let mismatched_metadata = CheckVersionArgs {
        reference: "HEAD".into(),
        repo_root: repo.path().to_path_buf(),
        allow_release_candidate: true,
    };
    assert!(check_version_sync(mismatched_metadata).is_err());
}

#[test]
fn floating_tag_skips_prerelease() {
    assert_eq!(parse_major_tag("v1.2.3").as_deref(), Some("v1"));
    assert_eq!(parse_major_tag("v1.2.3-beta.1"), None);
}
