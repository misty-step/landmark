//! Contract tests for model_policy: chain selection, provenance isolation,
//! and cost-rate honesty. These defend the ticket's observable contracts —
//! derived chains never leak into classification, escalation honors the
//! selected primary, and budget math matches pinned rates.

use crate::*;

fn test_config(model_policy: &str, model: &str) -> EffectiveSynthesisConfig {
    EffectiveSynthesisConfig {
        product_name: "Demo".into(),
        product_description: "Demo release automation.".into(),
        voice_guide: String::new(),
        audience: "developer".into(),
        changelog_source: "auto".into(),
        model_policy: model_policy.into(),
        model: model.into(),
        model_explicit: false,
        fallback_models: String::new(),
        max_input_tokens: None,
        max_output_tokens: None,
        max_usd: None,
    }
}

fn high_significance() -> ReleaseClassification {
    ReleaseClassification {
        categories: vec!["user-visible".into()],
        significance: "high".into(),
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
fn effective_fallback_models_follow_selected_primary_and_tier_order() {
    let config = test_config("balanced", "deepseek/deepseek-v4-flash-0731");
    assert_eq!(
        effective_fallback_models(&config, "deepseek/deepseek-v4-flash-0731"),
        vec![
            "google/gemini-3.7-flash".to_string(),
            "deepseek/deepseek-v4-pro-0813".to_string()
        ]
    );
    assert_eq!(
        effective_fallback_models(&config, "deepseek/deepseek-v4-pro-0813"),
        vec![
            "deepseek/deepseek-v4-flash-0731".to_string(),
            "google/gemini-3.7-flash".to_string()
        ],
        "after high-significance escalation the chain must drop the escalated primary, not config.model"
    );
    assert!(effective_fallback_models(&config, "off").is_empty());
    let mut configured = test_config("balanced", "primary/model");
    configured.fallback_models = "custom/a, custom/b".into();
    assert_eq!(
        effective_fallback_models(&configured, "primary/model"),
        vec!["custom/a".to_string(), "custom/b".to_string()]
    );
}

#[test]
fn classification_roster_excludes_derived_synthesis_chain() {
    let repo = tempfile::tempdir().unwrap();
    let args = SynthesizeArgs {
        api_key: "test".into(),
        model: String::new(),
        model_policy: String::new(),
        api_url: "http://example.invalid".into(),
        fallback_models: String::new(),
        product_name: "Demo".into(),
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
        claim_map_file: PathBuf::from("."),
    };
    let config = resolve_synthesis_config(&args).unwrap();
    assert_eq!(config.model, "deepseek/deepseek-v4-flash-0731");
    assert!(
        config.fallback_models.is_empty(),
        "derived synthesis chain must not masquerade as configured fallbacks"
    );
    let roster = release_classification_models(&config);
    assert_eq!(
        roster,
        vec![
            "deepseek/deepseek-v4-flash-0731".to_string(),
            "google/gemini-3.7-flash".to_string()
        ],
        "classifier stays on classification tiers; the rich synthesis pin must not leak"
    );
}

#[test]
fn balanced_high_significance_escalates_to_rich_tier_unless_primary_explicit() {
    let mut config = test_config("balanced", "deepseek/deepseek-v4-flash-0731");
    let (tier, model, skip, _) = selected_model_plan(&config, &high_significance());
    assert!(!skip);
    assert_eq!(tier, "rich");
    assert_eq!(model, "deepseek/deepseek-v4-pro-0813");

    config.model = "custom/model".into();
    config.model_explicit = true;
    let (_, escalated, _, _) = selected_model_plan(&config, &high_significance());
    assert_eq!(escalated, "custom/model");
}

#[test]
fn policy_default_model_normalizes_policy_case() {
    assert_eq!(policy_default_model(Some("OFF")).as_deref(), Some("off"));
    assert_eq!(
        policy_default_model(Some("Rich")).as_deref(),
        Some("deepseek/deepseek-v4-pro-0813")
    );
    let mut config = test_config("OFF", "off");
    config.fallback_models = String::new();
    let (tier, _, skip, _) = selected_model_plan(&config, &high_significance());
    assert!(skip);
    assert_eq!(tier, "off");
}

#[test]
fn cost_estimates_use_current_commodity_pins() {
    let flash = estimate_model_cost_usd("cheap", 12_000, 1_200);
    assert!(
        (flash - (12_000.0 / 1_000_000.0 * 0.08 + 1_200.0 / 1_000_000.0 * 0.18)).abs() < 1e-12,
        "{flash}"
    );
    assert_eq!(estimate_model_cost_usd("balanced", 12_000, 1_200), flash);
    let rich = estimate_model_cost_usd("rich", 12_000, 1_200);
    assert!(rich > flash * 10.0, "rich {rich} must exceed cheap {flash}");
}

#[test]
fn manifest_schema_policy_pattern_matches_runtime_case_contract() {
    let schema_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/landmark-manifest.v1.schema.json"
    );
    let schema: Value = serde_json::from_str(&fs::read_to_string(schema_path).unwrap()).unwrap();
    let pattern = schema["properties"]["model"]["properties"]["policy"]["pattern"]
        .as_str()
        .expect("manifest schema policy must declare a case-insensitive pattern");
    let policy_pattern = Regex::new(pattern).unwrap();
    for accepted in [
        "cheap", "balanced", "rich", "off", "OFF", "Rich", "BALANCED",
    ] {
        assert!(
            policy_pattern.is_match(accepted),
            "schema must accept {accepted}: runtime accepts any case"
        );
    }
    for rejected in ["", "bogus", "off ", "cheapp"] {
        assert!(
            !policy_pattern.is_match(rejected),
            "schema must reject {rejected}"
        );
    }
}

#[test]
fn manifest_policy_case_is_accepted_and_normalized_end_to_end() {
    for (manifest_policy, expected_model) in
        [("OFF", "off"), ("Rich", "deepseek/deepseek-v4-pro-0813")]
    {
        let repo = tempfile::tempdir().unwrap();
        fs::write(
            repo.path().join(".landmark.yml"),
            format!("product:\n  name: Demo\nmodel:\n  policy: {manifest_policy}\n"),
        )
        .unwrap();
        let args = SynthesizeArgs {
            api_key: "test".into(),
            model: String::new(),
            model_policy: String::new(),
            api_url: "http://example.invalid".into(),
            fallback_models: String::new(),
            product_name: "Demo".into(),
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
            claim_map_file: PathBuf::from("."),
        };
        let config = resolve_synthesis_config(&args).unwrap();
        assert_eq!(
            config.model, expected_model,
            "manifest policy {manifest_policy} must resolve through tier pins"
        );
        if expected_model == "off" {
            let (tier, _, skip, _) = selected_model_plan(&config, &high_significance());
            assert!(skip);
            assert_eq!(tier, "off");
        }
    }
}
