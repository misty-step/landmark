use crate::*;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub(crate) enum VersionBump {
    Patch,
    Minor,
    Major,
}

impl VersionBump {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            VersionBump::Patch => "patch",
            VersionBump::Minor => "minor",
            VersionBump::Major => "major",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "patch" => Some(VersionBump::Patch),
            "minor" => Some(VersionBump::Minor),
            "major" => Some(VersionBump::Major),
            _ => None,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub(crate) enum CommitCategory {
    Breaking,
    Feature,
    Fix,
    NonRelease,
    Unknown,
}

#[derive(Clone, Debug)]
pub(crate) struct ClassifiedCommit {
    pub(crate) id: String,
    pub(crate) subject: String,
    /// Raw conventional-commit type ("feat", "fix", "perf", "revert", "chore", ...),
    /// or empty when the subject does not parse as a conventional-commit header.
    /// Kept alongside the coarser `category` so changelog rendering can still
    /// distinguish, say, `fix` from `perf` even though both bump `patch`.
    pub(crate) kind: String,
    pub(crate) scope: String,
    pub(crate) description: String,
    pub(crate) breaking: bool,
    pub(crate) category: CommitCategory,
}

impl ClassifiedCommit {
    pub(crate) fn evidence_line(&self) -> String {
        format!("{} {}", self.id, self.subject)
    }
}

/// Parses a conventional-commit header (`type(scope)!: description`). Shared by
/// every version-decision call site so `type`/`scope`/breaking-bang extraction
/// never drifts between them.
pub(crate) fn parse_conventional_header(subject: &str) -> Option<(String, String, bool, String)> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^([A-Za-z]+)(?:\(([^)]+)\))?(!)?: (.+)$").unwrap());
    let caps = re.captures(subject.trim())?;
    Some((
        caps.get(1)?.as_str().to_ascii_lowercase(),
        caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string(),
        caps.get(3).is_some(),
        caps.get(4)?.as_str().to_string(),
    ))
}

pub(crate) fn has_breaking_footer(body: &str) -> bool {
    body.lines().any(|line| {
        let line = line.trim();
        line.starts_with("BREAKING CHANGE:") || line.starts_with("BREAKING-CHANGE:")
    })
}

pub(crate) fn is_revert_commit(subject: &str, kind: Option<&str>, body: &str) -> bool {
    subject.trim().starts_with("Revert \"")
        || kind == Some("revert")
        || body
            .lines()
            .any(|line| line.trim().starts_with("This reverts commit"))
}

/// One shared classifier for release-relevant commit intent. Every Landmark
/// entry point (`landmark run`, `prepare-self-release`, and the GitHub Action's
/// semantic-release compatibility path) must derive its version bump from this
/// same categorization so the three never silently disagree.
pub(crate) fn classify_commit(id: &str, subject: &str, body: &str) -> ClassifiedCommit {
    let header = parse_conventional_header(subject);
    let kind = header.as_ref().map(|(kind, ..)| kind.as_str());
    let scope = header
        .as_ref()
        .map(|(_, scope, ..)| scope.clone())
        .unwrap_or_default();
    let description = header
        .as_ref()
        .map(|(_, _, _, description)| description.clone())
        .unwrap_or_else(|| subject.to_string());
    let breaking =
        header.as_ref().is_some_and(|(_, _, breaking, _)| *breaking) || has_breaking_footer(body);
    let is_revert = is_revert_commit(subject, kind, body);

    let category = if breaking {
        CommitCategory::Breaking
    } else if is_revert {
        CommitCategory::Fix
    } else {
        match kind {
            Some("feat") => CommitCategory::Feature,
            Some("fix") | Some("perf") => CommitCategory::Fix,
            Some(_) => CommitCategory::NonRelease,
            None => CommitCategory::Unknown,
        }
    };
    let kind = if is_revert {
        "revert".to_string()
    } else {
        kind.unwrap_or_default().to_string()
    };

    ClassifiedCommit {
        id: id.to_string(),
        subject: subject.to_string(),
        kind,
        scope,
        description,
        breaking,
        category,
    }
}

#[derive(Debug)]
pub(crate) struct VersionDecision {
    pub(crate) bump: Option<VersionBump>,
    pub(crate) decisive: Option<ClassifiedCommit>,
    pub(crate) unknown_commits: Vec<ClassifiedCommit>,
}

/// Reduces classified commits to a single bump decision. Unknown (non-
/// conventional) commits never silently become `patch`: they are recorded in
/// `unknown_commits` for the caller to surface, and they only ever suppress a
/// bump that no other commit already justified — matching semantic-release's
/// own behavior of treating an unrecognized commit as no signal rather than a
/// release trigger.
pub(crate) fn decide_version(commits: &[ClassifiedCommit]) -> VersionDecision {
    let mut bump: Option<VersionBump> = None;
    let mut decisive: Option<ClassifiedCommit> = None;
    let mut unknown_commits = Vec::new();

    for commit in commits {
        let candidate = match commit.category {
            CommitCategory::Breaking => Some(VersionBump::Major),
            CommitCategory::Feature => Some(VersionBump::Minor),
            CommitCategory::Fix => Some(VersionBump::Patch),
            CommitCategory::NonRelease => None,
            CommitCategory::Unknown => {
                unknown_commits.push(commit.clone());
                None
            }
        };
        if let Some(candidate) = candidate
            && bump.is_none_or(|current| candidate > current)
        {
            bump = Some(candidate);
            decisive = Some(commit.clone());
        }
    }

    VersionDecision {
        bump,
        decisive,
        unknown_commits,
    }
}

/// Cargo-style pre-stable versioning. While the current version is below 1.0.0
/// the 0.x line treats a *minor* bump as its breaking boundary, so a repo never
/// auto-crosses into 1.0.0: Major demotes to Minor (0.x -> 0.(x+1)), Minor
/// demotes to Patch, Patch is unchanged. At or above 1.0.0 the bump is returned
/// unchanged. Promotion to stable is a manual `v1.0.0` tag; once the current
/// version is >= 1.0.0 auto-detection stops demoting. Callers pass "0.0.0" for a
/// repo with no releases yet so a breaking first change still stays below 1.0.0.
/// See card landmark-016.
pub(crate) fn apply_stability(bump: VersionBump, current_version: &str) -> VersionBump {
    if is_pre_stable(current_version) {
        match bump {
            VersionBump::Major => VersionBump::Minor,
            VersionBump::Minor => VersionBump::Patch,
            VersionBump::Patch => VersionBump::Patch,
        }
    } else {
        bump
    }
}

/// A repo is pre-stable when its current version parses as semver below 1.0.0.
/// Unparseable/exotic versions resolve to stable (identity), matching the
/// action-level `auto` fallback for tag formats we don't recognize.
pub(crate) fn is_pre_stable(current_version: &str) -> bool {
    semver_from_tag(current_version)
        .map(|((major, _, _), _)| major == 0)
        .unwrap_or(false)
}
