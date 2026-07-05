use crate::*;

/// Machine-checkable record of which synthesized sections trace back to a
/// deterministic release signal (a real commit) and which do not. This is
/// the artifact the synthesis quality gate inspects instead of trusting
/// structure alone: a section can have perfect Markdown shape (`## ` heading,
/// `- ` bullets) and still be fiction, which is exactly what shipped for
/// canary v1.14.0 (invented Breaking Changes + Bug Fixes sections for a
/// release with a single `feat` commit).
#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct ClaimSourceMap {
    pub(crate) grounded: bool,
    pub(crate) ungrounded_sections: Vec<String>,
    pub(crate) sections: Vec<ClaimSectionMapping>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ClaimSectionMapping {
    pub(crate) title: String,
    pub(crate) kind: String,
    pub(crate) bullet_count: usize,
    pub(crate) matched_sources: Vec<String>,
    pub(crate) grounded: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClaimKind {
    Breaking,
    Feature,
    Fix,
    Improvement,
    Other,
}

impl ClaimKind {
    fn from_title(title: &str) -> Self {
        match title.trim().to_ascii_lowercase().as_str() {
            "breaking changes" => ClaimKind::Breaking,
            "new features" | "features" => ClaimKind::Feature,
            "bug fixes" | "fixes" => ClaimKind::Fix,
            "improvements" => ClaimKind::Improvement,
            _ => ClaimKind::Other,
        }
    }

    fn label(self) -> &'static str {
        match self {
            ClaimKind::Breaking => "breaking",
            ClaimKind::Feature => "feature",
            ClaimKind::Fix => "fix",
            ClaimKind::Improvement => "improvement",
            ClaimKind::Other => "other",
        }
    }

    /// Breaking Changes and Bug Fixes name a specific, falsifiable kind of
    /// change, so only a commit carrying that exact deterministic signal
    /// entails the claim. Improvements and unrecognized headings are the
    /// catch-all buckets every prompt template can produce; any release
    /// commit grounds them, but an empty release commit range still grounds
    /// nothing.
    fn matches(self, commit: &ContextCommit) -> bool {
        match self {
            ClaimKind::Breaking => commit.breaking,
            ClaimKind::Feature => commit.conventional_type == "feat",
            ClaimKind::Fix => commit.conventional_type == "fix",
            ClaimKind::Improvement | ClaimKind::Other => true,
        }
    }
}

pub(crate) fn build_claim_source_map(
    notes: &str,
    deterministic: &DeterministicReleaseContext,
) -> ClaimSourceMap {
    let sections: Vec<ClaimSectionMapping> = parse_note_sections(notes)
        .iter()
        .map(|section| section_mapping(section, &deterministic.commits))
        .collect();
    let ungrounded_sections = sections
        .iter()
        .filter(|section| !section.grounded)
        .map(|section| section.title.clone())
        .collect();
    ClaimSourceMap {
        grounded: sections.iter().all(|section| section.grounded),
        ungrounded_sections,
        sections,
    }
}

fn section_mapping(section: &NoteSection, commits: &[ContextCommit]) -> ClaimSectionMapping {
    let kind = ClaimKind::from_title(&section.title);
    let matched_sources: Vec<String> = commits
        .iter()
        .filter(|commit| kind.matches(commit))
        .map(commit_source_id)
        .collect();
    ClaimSectionMapping {
        title: section.title.clone(),
        kind: kind.label().to_string(),
        bullet_count: section.bullets.len(),
        grounded: !matched_sources.is_empty(),
        matched_sources,
    }
}

fn commit_source_id(commit: &ContextCommit) -> String {
    if commit.short_hash.trim().is_empty() {
        commit.subject.clone()
    } else {
        commit.short_hash.clone()
    }
}
