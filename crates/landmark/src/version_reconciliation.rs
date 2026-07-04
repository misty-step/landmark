use crate::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct VersionDecisionWaiver {
    pub(crate) required: bool,
    pub(crate) status: String,
    pub(crate) kind: String,
    pub(crate) reason: String,
}

#[derive(Debug)]
pub(crate) struct ReconciledVersionDecision {
    pub(crate) bump: Option<VersionBump>,
    pub(crate) decisive: Option<ClassifiedCommit>,
    pub(crate) unknown_commits: Vec<ClassifiedCommit>,
    pub(crate) commit_bump: String,
    pub(crate) api_evidence_bump: String,
    pub(crate) reconciliation: String,
    pub(crate) decisive_signals: Vec<String>,
    pub(crate) api_evidence: VersionApiEvidence,
    pub(crate) waiver: VersionDecisionWaiver,
}

pub(crate) fn decide_version_with_api_evidence(
    commits: &[ClassifiedCommit],
    api_evidence: VersionApiEvidence,
) -> ReconciledVersionDecision {
    let commit_decision = decide_version(commits);
    let commit_bump = bump_label(commit_decision.bump).to_string();
    let api_bump = VersionBump::from_str(&api_evidence.bump);
    let api_evidence_bump = bump_label(api_bump).to_string();
    let mut final_bump = commit_decision.bump;
    let mut waiver = waiver_not_required();

    let reconciliation = match api_evidence.status.as_str() {
        "findings" => match (commit_decision.bump, api_bump) {
            (_, Some(api_bump)) if commit_decision.bump.is_none_or(|floor| api_bump > floor) => {
                final_bump = Some(api_bump);
                "upgraded"
            }
            (Some(floor), Some(api_bump)) if api_bump == floor => "agreed",
            (Some(_), Some(_)) => {
                waiver = missing_version_intent_waiver(&commit_bump, &api_evidence_bump);
                "conflict"
            }
            _ => "unverified",
        },
        "passed" if commit_decision.bump == Some(VersionBump::Major) => {
            waiver = missing_version_intent_waiver(&commit_bump, "none");
            "conflict"
        }
        "passed" => "compatible",
        "skipped" => "unavailable",
        "failed" => "unverified",
        _ => "unverified",
    }
    .to_string();

    let mut decisive_signals = Vec::new();
    if let Some(commit) = &commit_decision.decisive {
        decisive_signals.push(format!("commit:{}", commit.evidence_line()));
    }
    decisive_signals.push(format!(
        "api-evidence:{} {} {}",
        api_evidence.provider, api_evidence.status, api_evidence.summary
    ));

    ReconciledVersionDecision {
        bump: final_bump,
        decisive: commit_decision.decisive,
        unknown_commits: commit_decision.unknown_commits,
        commit_bump,
        api_evidence_bump,
        reconciliation,
        decisive_signals,
        api_evidence,
        waiver,
    }
}

fn bump_label(bump: Option<VersionBump>) -> &'static str {
    bump.map(VersionBump::as_str).unwrap_or("none")
}

fn waiver_not_required() -> VersionDecisionWaiver {
    VersionDecisionWaiver {
        required: false,
        status: "not-required".into(),
        kind: String::new(),
        reason: String::new(),
    }
}

fn missing_version_intent_waiver(commit_bump: &str, api_bump: &str) -> VersionDecisionWaiver {
    VersionDecisionWaiver {
        required: true,
        status: "missing".into(),
        kind: "version-intent-policy".into(),
        reason: format!(
            "commit-derived bump `{commit_bump}` conflicts with public API evidence `{api_bump}`; a typed version-intent waiver or human review is required"
        ),
    }
}
