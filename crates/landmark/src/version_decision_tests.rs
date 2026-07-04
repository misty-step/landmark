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

// The Threshold incident (backlog 002): a repo/product rename range computed
// `patch` under the old engine because it defaulted anything unrecognized to
// patch. These pin the fixed behavior so it can't regress silently.

#[test]
fn conventional_rename_commit_does_not_bump() {
    // `refactor:` is a recognized conventional type with no release intent.
    // A rename range described this way must resolve to no bump, not patch.
    let decision = decide_version(&[commit("a", "refactor: rename Foo to Bar", "")]);
    assert_eq!(decision.bump, None);
    assert!(decision.unknown_commits.is_empty());
}

#[test]
fn freeform_rename_commit_is_named_unknown_not_silently_patched() {
    // No conventional-commit prefix at all -- the exact shape of a pre-
    // convention rename/rebrand commit. Must be named unknown, never patch.
    let decision = decide_version(&[commit("a", "Rename Foo to Bar across the repo", "")]);
    assert_eq!(decision.bump, None);
    assert_eq!(decision.unknown_commits.len(), 1);
    assert_eq!(decision.unknown_commits[0].id, "a");
}

#[test]
fn rename_marked_breaking_bumps_major() {
    // When a rename genuinely is product-breaking and is marked as such, the
    // engine must still recognize it -- the fix is refusing to *guess*, not
    // refusing to *recognize an explicit signal*.
    let decision = decide_version(&[commit("a", "feat(api)!: rename public output field", "")]);
    assert_eq!(decision.bump, Some(VersionBump::Major));
}

#[test]
fn rename_commit_alongside_real_signal_never_blocks_the_release() {
    let decision = decide_version(&[
        commit("a", "Rename internal module", ""),
        commit("b", "fix(cli): correct exit code", ""),
    ]);
    assert_eq!(decision.bump, Some(VersionBump::Patch));
    assert_eq!(decision.decisive.unwrap().id, "b");
    assert_eq!(decision.unknown_commits.len(), 1);
    assert_eq!(decision.unknown_commits[0].id, "a");
}

fn api_evidence(status: &str, bump: &str) -> VersionApiEvidence {
    VersionApiEvidence {
        provider: "cargo-semver-checks".into(),
        status: status.into(),
        bump: bump.into(),
        baseline: "v1.0.0".into(),
        target: "HEAD".into(),
        command: "cargo semver-checks --baseline-rev v1.0.0".into(),
        exit_code: 0,
        summary: format!("fixture {status}"),
        findings: Vec::new(),
        failure_message: String::new(),
    }
}

#[test]
fn api_evidence_can_upgrade_commit_floor_but_never_downgrade_it() {
    let commits = [commit("a", "feat(api): add helper", "")];
    let reconciled = decide_version_with_api_evidence(&commits, api_evidence("findings", "major"));
    assert_eq!(reconciled.bump, Some(VersionBump::Major));
    assert_eq!(reconciled.commit_bump, "minor");
    assert_eq!(reconciled.api_evidence_bump, "major");
    assert_eq!(reconciled.reconciliation, "upgraded");
    assert!(
        reconciled
            .decisive_signals
            .iter()
            .any(|signal| signal.contains("api-evidence:cargo-semver-checks"))
    );

    let floor = decide_version_with_api_evidence(
        &[commit("b", "feat(api)!: rename helper", "")],
        api_evidence("passed", "none"),
    );
    assert_eq!(floor.bump, Some(VersionBump::Major));
    assert_eq!(floor.reconciliation, "conflict");
    assert_eq!(floor.waiver.status, "missing");
    assert!(floor.waiver.required);
}

#[test]
fn absent_or_failed_api_evidence_keeps_the_floor_loudly() {
    let commits = [commit("a", "fix(cli): patch output", "")];
    let absent =
        decide_version_with_api_evidence(&commits, no_version_api_evidence("no Cargo.toml"));
    assert_eq!(absent.bump, Some(VersionBump::Patch));
    assert_eq!(absent.reconciliation, "unavailable");
    assert_eq!(absent.api_evidence.status, "skipped");
    assert!(
        absent
            .decisive_signals
            .iter()
            .any(|signal| signal.contains("api-evidence:none skipped"))
    );

    let mut failed = api_evidence("failed", "none");
    failed.failure_message = "cargo semver-checks exited 2".into();
    let failed = decide_version_with_api_evidence(&commits, failed);
    assert_eq!(failed.bump, Some(VersionBump::Patch));
    assert_eq!(failed.reconciliation, "unverified");
    assert_eq!(failed.waiver.status, "not-required");
    assert!(
        failed
            .decisive_signals
            .iter()
            .any(|signal| signal.contains("api-evidence:cargo-semver-checks failed"))
    );
}
