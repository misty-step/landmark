use super::*;

/// The one version-decision corpus shared by `landmark run`, `prepare-self-release`,
/// and (for the categories it can express) semantic-release's default angular
/// preset. Folds the former separate `decide_version_bump` and `release_bump`
/// suites together so the two entry points can never silently diverge again.
fn commit(id: &str, subject: &str, body: &str) -> ClassifiedCommit {
    classify_commit(id, subject, body)
}

#[test]
fn breaking_bang_wins_over_everything() {
    let commits = [
        commit("a", "fix(runtime): close leak", ""),
        commit("b", "feat(setup): add analyzer", ""),
        commit("c", "feat(api)!: rename output", ""),
    ];
    let decision = decide_version(&commits);
    assert_eq!(decision.bump, Some(VersionBump::Major));
    assert_eq!(decision.decisive.unwrap().id, "c");
    assert!(decision.unknown_commits.is_empty());
}

#[test]
fn breaking_change_footer_without_bang_is_also_major() {
    let commits = [commit(
        "a",
        "feat(api): rename field",
        "BREAKING CHANGE: clients must migrate field names",
    )];
    let decision = decide_version(&commits);
    assert_eq!(decision.bump, Some(VersionBump::Major));
}

#[test]
fn breaking_change_hyphen_footer_is_also_major() {
    let commits = [commit(
        "a",
        "fix(api): adjust field",
        "BREAKING-CHANGE: clients must migrate field names",
    )];
    let decision = decide_version(&commits);
    assert_eq!(decision.bump, Some(VersionBump::Major));
}

#[test]
fn feat_bumps_minor() {
    let commits = [commit("a", "feat(cli): add local run", "")];
    let decision = decide_version(&commits);
    assert_eq!(decision.bump, Some(VersionBump::Minor));
}

#[test]
fn fix_and_perf_bump_patch() {
    let fix = decide_version(&[commit("a", "fix(action): patch output", "")]);
    assert_eq!(fix.bump, Some(VersionBump::Patch));
    let perf = decide_version(&[commit("a", "perf(run): speed up render", "")]);
    assert_eq!(perf.bump, Some(VersionBump::Patch));
}

#[test]
fn reverts_bump_patch_regardless_of_format() {
    let conventional = decide_version(&[commit("a", "revert: feat(api): add x", "")]);
    assert_eq!(conventional.bump, Some(VersionBump::Patch));
    let git_default = decide_version(&[commit(
        "b",
        "Revert \"feat(api): add x\"",
        "This reverts commit abc123.",
    )]);
    assert_eq!(git_default.bump, Some(VersionBump::Patch));
}

#[test]
fn squash_merge_body_sub_lines_are_not_separately_classified() {
    // A squash-merged PR often carries other commits' subjects in the body.
    // Only the real header (the subject) may drive the bump; embedded lines
    // that look like conventional headers must not be reparsed.
    let commits = [commit(
        "a",
        "chore(deps): bump lockfile",
        "* feat: something that never happened\n* fix!: also never happened",
    )];
    let decision = decide_version(&commits);
    assert_eq!(decision.bump, None);
    assert!(decision.unknown_commits.is_empty());
}

#[test]
fn non_release_conventional_types_do_not_bump_but_are_not_unknown() {
    for subject in [
        "chore: tidy",
        "docs: update readme",
        "test: add coverage",
        "ci: tune workflow",
        "build: bump toolchain",
        "style: reformat",
        "refactor: extract helper",
    ] {
        let decision = decide_version(&[commit("a", subject, "")]);
        assert_eq!(decision.bump, None, "{subject} must not bump a release");
        assert!(
            decision.unknown_commits.is_empty(),
            "{subject} is conventional and must not be reported unknown"
        );
    }
}

#[test]
fn non_conventional_commits_are_named_unknown_and_never_silently_patch() {
    let decision = decide_version(&[
        commit("a", "Merge pull request #1 from misty-step/thing", ""),
        commit("b", "wip", ""),
    ]);
    assert_eq!(
        decision.bump, None,
        "unknown commits alone must never silently resolve to patch"
    );
    assert_eq!(decision.unknown_commits.len(), 2);
}

#[test]
fn known_signal_wins_even_alongside_unknown_commits() {
    // Unknown commits are always named, but they must never block a release
    // that other, properly classified commits already justify.
    let decision = decide_version(&[
        commit("a", "wip debugging", ""),
        commit("b", "fix(cli): correct exit code", ""),
    ]);
    assert_eq!(decision.bump, Some(VersionBump::Patch));
    assert_eq!(decision.unknown_commits.len(), 1);
    assert_eq!(decision.unknown_commits[0].id, "a");
}

#[test]
fn empty_range_has_no_bump_and_no_unknown_commits() {
    let decision = decide_version(&[]);
    assert_eq!(decision.bump, None);
    assert!(decision.unknown_commits.is_empty());
    assert!(decision.decisive.is_none());
}

#[test]
fn bootstrap_range_with_only_unconventional_history_refuses_silently_patching() {
    // A first-ever release range (no previous tag) commonly contains messy
    // pre-convention history. If NONE of it is recognizable, the decision
    // must stay `none` (and name every unknown commit) rather than guess.
    let decision = decide_version(&[
        commit("a", "Initial commit", ""),
        commit("b", "wip", ""),
        commit("c", "more wip", ""),
    ]);
    assert_eq!(decision.bump, None);
    assert_eq!(decision.unknown_commits.len(), 3);
}

#[test]
fn bootstrap_range_with_real_signal_still_bumps() {
    let decision = decide_version(&[
        commit("a", "Initial commit", ""),
        commit("b", "feat: first feature", ""),
    ]);
    assert_eq!(decision.bump, Some(VersionBump::Minor));
    assert_eq!(decision.unknown_commits.len(), 1);
}
