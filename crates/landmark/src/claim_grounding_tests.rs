use super::*;

fn feat_commit(subject: &str) -> ContextCommit {
    ContextCommit {
        subject: subject.to_string(),
        body: String::new(),
        short_hash: "abc1234".into(),
        conventional_type: "feat".into(),
        breaking: false,
    }
}

fn fix_commit(subject: &str) -> ContextCommit {
    ContextCommit {
        subject: subject.to_string(),
        body: String::new(),
        short_hash: "def5678".into(),
        conventional_type: "fix".into(),
        breaking: false,
    }
}

fn breaking_commit(subject: &str) -> ContextCommit {
    ContextCommit {
        subject: subject.to_string(),
        body: String::new(),
        short_hash: "9990000".into(),
        conventional_type: "feat".into(),
        breaking: true,
    }
}

#[test]
fn single_feature_commit_grounds_new_features_but_not_breaking_or_fixes() {
    // This is the canary v1.14.0 regression: one real `feat` PR, and the model
    // invented "Breaking Changes" and "Bug Fixes" sections describing events
    // that never happened. The gate must catch that fabrication even though
    // the notes are structurally perfect Markdown.
    let notes = "## Breaking Changes\n\
- Users need to update scripts that call the old entrypoint.\n\n\
## New Features\n\
- Cold agents can now prove readiness before joining a run.\n\n\
## Bug Fixes\n\
- Resolved a delay in health-report delivery.\n";
    let deterministic = DeterministicReleaseContext {
        commits: vec![feat_commit(
            "feat(agent-ops): add cold-agent readiness proof entrypoint",
        )],
        ..Default::default()
    };

    let map = build_claim_source_map(notes, &deterministic);

    assert!(!map.grounded, "fabricated sections must not be grounded");
    assert_eq!(
        map.ungrounded_sections,
        vec!["Breaking Changes".to_string(), "Bug Fixes".to_string()]
    );
    let features = map
        .sections
        .iter()
        .find(|section| section.title == "New Features")
        .expect("features section present");
    assert!(features.grounded);
    assert!(!features.matched_sources.is_empty());
}

#[test]
fn fix_commit_grounds_bug_fixes_section() {
    let notes = "## Bug Fixes\n- Dashboard no longer crashes on empty profile fields.\n";
    let deterministic = DeterministicReleaseContext {
        commits: vec![fix_commit("fix: dashboard crash on empty profile fields")],
        ..Default::default()
    };

    let map = build_claim_source_map(notes, &deterministic);

    assert!(map.grounded);
    assert!(map.ungrounded_sections.is_empty());
}

#[test]
fn breaking_commit_grounds_breaking_changes_section() {
    let notes = "## Breaking Changes\n- The deprecated /v1/auth endpoint was removed.\n";
    let deterministic = DeterministicReleaseContext {
        commits: vec![breaking_commit("feat(auth)!: remove /v1/auth endpoint")],
        ..Default::default()
    };

    let map = build_claim_source_map(notes, &deterministic);

    assert!(map.grounded);
}

#[test]
fn improvements_section_is_grounded_by_any_release_commit() {
    let notes = "## Improvements\n- The dashboard loads noticeably faster now.\n";
    let deterministic = DeterministicReleaseContext {
        commits: vec![feat_commit("feat: add lazy loading to dashboard widgets")],
        ..Default::default()
    };

    let map = build_claim_source_map(notes, &deterministic);

    assert!(map.grounded);
}

#[test]
fn empty_commit_range_grounds_nothing() {
    let notes = "## Improvements\n- Things got better somehow.\n";
    let deterministic = DeterministicReleaseContext::default();

    let map = build_claim_source_map(notes, &deterministic);

    assert!(!map.grounded, "no release commits means no evidence at all");
    assert_eq!(map.ungrounded_sections, vec!["Improvements".to_string()]);
}
