use super::release_kit::{
    release_kit_audiences, release_kit_importance, release_kit_needs_rich_artifacts,
    release_kit_social_draft_eligible, social_voice_label,
};
use super::*;

fn baseline_classification() -> ReleaseClassification {
    ReleaseClassification {
        categories: vec!["user-visible".into()],
        significance: "medium".into(),
        user_visible: true,
        breaking: false,
        security: false,
        migration_heavy: false,
        source: "structured".into(),
        model: String::new(),
        deterministic_signals: Vec::new(),
        disagreements: Vec::new(),
        reasons: Vec::new(),
    }
}

fn baseline_decision() -> RunVersionDecision {
    RunVersionDecision {
        latest_tag: "v1.0.0".into(),
        bump: "minor".into(),
        commit_bump: "minor".into(),
        api_evidence_bump: "none".into(),
        reconciliation: "unavailable".into(),
        commit_count: 1,
        conventional_commit_count: 1,
        range: "v1.0.0..HEAD".into(),
        decisive_commit: None,
        decisive_signals: Vec::new(),
        unknown_commits: Vec::new(),
        api_evidence: no_version_api_evidence("test fixture"),
        waiver: VersionDecisionWaiver {
            required: false,
            status: "not-required".into(),
            kind: String::new(),
            reason: String::new(),
        },
    }
}

#[test]
fn importance_is_security_when_classification_flags_security() {
    let mut classification = baseline_classification();
    classification.security = true;
    // Security wins even alongside signals that would otherwise say migration/high.
    classification.breaking = true;
    classification.significance = "high".into();
    let importance = release_kit_importance(&classification, &baseline_decision());
    assert_eq!(importance, "security");
}

#[test]
fn importance_is_migration_for_major_bump_breaking_or_migration_heavy() {
    let decision_major = RunVersionDecision {
        bump: "major".into(),
        ..baseline_decision()
    };
    assert_eq!(
        release_kit_importance(&baseline_classification(), &decision_major),
        "migration"
    );

    let mut breaking_classification = baseline_classification();
    breaking_classification.breaking = true;
    assert_eq!(
        release_kit_importance(&breaking_classification, &baseline_decision()),
        "migration"
    );

    let mut migration_heavy_classification = baseline_classification();
    migration_heavy_classification.migration_heavy = true;
    assert_eq!(
        release_kit_importance(&migration_heavy_classification, &baseline_decision()),
        "migration"
    );
}

#[test]
fn importance_is_high_for_high_significance() {
    let mut classification = baseline_classification();
    classification.significance = "high".into();
    assert_eq!(
        release_kit_importance(&classification, &baseline_decision()),
        "high"
    );
}

#[test]
fn importance_is_launch_for_bootstrap_release_with_a_real_bump() {
    let decision = RunVersionDecision {
        latest_tag: String::new(),
        bump: "minor".into(),
        ..baseline_decision()
    };
    assert_eq!(
        release_kit_importance(&baseline_classification(), &decision),
        "launch"
    );
}

#[test]
fn importance_is_not_launch_when_bootstrap_has_no_bump() {
    let decision = RunVersionDecision {
        latest_tag: String::new(),
        bump: "none".into(),
        ..baseline_decision()
    };
    // No latest tag but also no bump: falls through to the medium default,
    // not launch -- "launch" specifically means a bootstrap release that
    // actually has release-worthy signal.
    assert_eq!(
        release_kit_importance(&baseline_classification(), &decision),
        "medium"
    );
}

#[test]
fn importance_is_low_for_low_significance() {
    let mut classification = baseline_classification();
    classification.significance = "low".into();
    assert_eq!(
        release_kit_importance(&classification, &baseline_decision()),
        "low"
    );
}

#[test]
fn importance_defaults_to_medium() {
    assert_eq!(
        release_kit_importance(&baseline_classification(), &baseline_decision()),
        "medium"
    );
}

#[test]
fn needs_rich_artifacts_matches_exactly_high_launch_migration_security() {
    for importance in ["high", "launch", "migration", "security"] {
        assert!(
            release_kit_needs_rich_artifacts(importance),
            "{importance} should need rich artifacts"
        );
    }
    for importance in ["medium", "low", "unknown", ""] {
        assert!(
            !release_kit_needs_rich_artifacts(importance),
            "{importance} should not need rich artifacts"
        );
    }
}

#[test]
fn audiences_always_include_primary_and_developer_operator() {
    let audiences = release_kit_audiences("enterprise", "low");
    assert!(audiences.contains(&"enterprise".to_string()));
    assert!(audiences.contains(&"developer-operator".to_string()));
    assert_eq!(audiences.len(), 2);
}

#[test]
fn audiences_add_release_operator_and_docs_owner_only_when_rich_artifacts_needed() {
    for importance in ["high", "launch", "migration", "security"] {
        let audiences = release_kit_audiences("general", importance);
        assert!(
            audiences.contains(&"release-operator".to_string()),
            "{importance} should add release-operator"
        );
        assert!(
            audiences.contains(&"docs-owner".to_string()),
            "{importance} should add docs-owner"
        );
    }
    for importance in ["medium", "low"] {
        let audiences = release_kit_audiences("general", importance);
        assert!(
            !audiences.contains(&"release-operator".to_string()),
            "{importance} should not add release-operator"
        );
        assert!(
            !audiences.contains(&"docs-owner".to_string()),
            "{importance} should not add docs-owner"
        );
    }
}

#[test]
fn social_draft_gate_excludes_chore_and_patch_noise() {
    let mut chore = baseline_classification();
    chore.user_visible = false;
    chore.significance = "low".into();
    chore.categories = vec!["chore-only".into()];
    chore.deterministic_signals = vec!["conventional:chore".into()];
    let patch_decision = RunVersionDecision {
        bump: "patch".into(),
        commit_bump: "patch".into(),
        ..baseline_decision()
    };
    assert!(!release_kit_social_draft_eligible(
        &chore,
        &patch_decision,
        "## Technical\n\n- chore(ci): refresh workflow"
    ));

    let mut fix = baseline_classification();
    fix.deterministic_signals = vec!["conventional:fix".into()];
    assert!(!release_kit_social_draft_eligible(
        &fix,
        &patch_decision,
        "## Technical\n\n- fix(cli): handle empty config"
    ));

    let mut perf = baseline_classification();
    perf.deterministic_signals = vec!["conventional:perf".into()];
    assert!(!release_kit_social_draft_eligible(
        &perf,
        &patch_decision,
        "## Technical\n\n- perf(api): reduce feed latency\n\nCrucible receipt: https://example.invalid/receipts/perf"
    ));
}

#[test]
fn social_draft_gate_allows_capabilities_and_breaking_changes() {
    let mut capability = baseline_classification();
    capability.deterministic_signals = vec!["conventional:feat".into()];
    assert!(release_kit_social_draft_eligible(
        &capability,
        &baseline_decision(),
        "## Technical\n\n- feat(cli): add release preview"
    ));

    let mut breaking = baseline_classification();
    breaking.breaking = true;
    assert!(release_kit_social_draft_eligible(
        &breaking,
        &RunVersionDecision {
            bump: "major".into(),
            commit_bump: "major".into(),
            ..baseline_decision()
        },
        "## Technical\n\n- feat(api)!: rename release field"
    ));
}

#[test]
fn social_voice_card_changes_draft_label() {
    assert_eq!(
        social_voice_label("operator-facing, concrete, low-hype"),
        "Operator note"
    );
    assert_eq!(
        social_voice_label("developer and maintainer focused"),
        "Maintainer note"
    );
    assert_eq!(social_voice_label("public user-facing"), "User-facing note");
    assert_eq!(social_voice_label("plain and quiet"), "Release note");
}
