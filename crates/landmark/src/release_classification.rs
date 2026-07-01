use crate::*;

pub(crate) fn classify_release_context(
    technical: &str,
    sources: &[ContextSource],
) -> ReleaseClassification {
    classify_release_context_from_text(technical, sources, "rendered-text")
}

pub(crate) fn classify_release_context_with_deterministic(
    technical: &str,
    sources: &[ContextSource],
    deterministic: &DeterministicReleaseContext,
) -> ReleaseClassification {
    let relevant_commits = release_relevant_commits(technical, deterministic);
    if relevant_commits.is_empty() {
        return classify_release_context_from_text(technical, sources, "rendered-text");
    }

    let mut categories = BTreeSet::new();
    let mut reasons = Vec::new();
    let mut deterministic_signals = BTreeSet::new();
    let mut user_visible = false;
    let mut breaking = false;
    let mut security = false;
    let mut migration_heavy = false;
    let mut docs_count = 0usize;
    let mut low_value_count = 0usize;

    for commit in &relevant_commits {
        let commit_text = format!("{}\n{}", commit.subject, commit.body).to_ascii_lowercase();
        match commit.conventional_type.as_str() {
            "feat" | "fix" | "perf" => {
                user_visible = true;
                categories.insert("user-visible");
                deterministic_signals.insert(format!("conventional:{}", commit.conventional_type));
            }
            "docs" => {
                docs_count += 1;
                low_value_count += 1;
                categories.insert("docs-only");
                deterministic_signals.insert("conventional:docs".to_string());
            }
            "chore" | "ci" | "build" | "test" | "refactor" => {
                low_value_count += 1;
                categories.insert("chore-only");
                deterministic_signals.insert(format!("conventional:{}", commit.conventional_type));
            }
            _ => {}
        }
        if commit.breaking {
            breaking = true;
            categories.insert("breaking");
            deterministic_signals.insert("breaking".to_string());
        }
        if commit_text.contains("dependabot")
            || commit_text.contains("dependency")
            || commit_text.contains("dependencies")
            || commit_text.contains("cargo.lock")
            || commit_text.contains("package-lock")
        {
            low_value_count += 1;
            categories.insert("dependency-only");
            deterministic_signals.insert("dependency".to_string());
        }
        if commit_text.contains("security")
            || commit_text.contains("vulnerability")
            || commit_text.contains("cve-")
            || commit_text.contains("secret")
        {
            security = true;
            categories.insert("security");
            deterministic_signals.insert("security".to_string());
        }
        if commit_text.contains("breaking change")
            || commit_text.contains("migration")
            || commit_text.contains("migrate")
            || commit_text.contains("deprecat")
        {
            migration_heavy = true;
            categories.insert("migration-heavy");
            deterministic_signals.insert("migration".to_string());
        }
    }

    if deterministic
        .changed_files
        .iter()
        .any(|path| path.starts_with(".github/") || path.starts_with("scripts/"))
    {
        categories.insert("internal-tooling");
        reasons.push("changed-file signals include internal tooling paths".to_string());
    }
    if docs_count == relevant_commits.len()
        || deterministic
            .changed_files
            .iter()
            .all(|path| path.ends_with(".md") || path.starts_with("docs/"))
    {
        categories.insert("docs-only");
    }
    if sources
        .iter()
        .any(|source| source.name == "pull_requests" && source.included)
    {
        reasons.push("PR metadata contributed to context".to_string());
    }
    if user_visible {
        reasons.push("parsed conventional commit feature/fix/perf signals detected".to_string());
    }
    if categories.is_empty() {
        categories.insert("user-visible");
        user_visible = true;
        reasons.push("no low-value-only signals found; defaulting to user-visible".to_string());
    }

    let low_value_only =
        !user_visible && !breaking && !security && !migration_heavy && low_value_count > 0;
    let significance = if breaking || security || migration_heavy {
        "high"
    } else if low_value_only {
        "low"
    } else {
        "medium"
    }
    .to_string();

    ReleaseClassification {
        categories: categories.into_iter().map(str::to_string).collect(),
        significance,
        user_visible,
        breaking,
        security,
        migration_heavy,
        source: "structured".into(),
        model: String::new(),
        deterministic_signals: deterministic_signals.into_iter().collect(),
        disagreements: Vec::new(),
        reasons,
    }
}

pub(crate) fn release_relevant_commits<'a>(
    technical: &str,
    deterministic: &'a DeterministicReleaseContext,
) -> Vec<&'a ContextCommit> {
    if deterministic.commits.is_empty() || technical.trim().is_empty() {
        return deterministic.commits.iter().collect();
    }
    let lower = technical.to_ascii_lowercase();
    deterministic
        .commits
        .iter()
        .filter(|commit| commit_matches_release_text(commit, &lower))
        .collect()
}

pub(crate) fn commit_matches_release_text(commit: &ContextCommit, lower_technical: &str) -> bool {
    let subject = commit.subject.to_ascii_lowercase();
    lower_technical.contains(&subject)
        || commit_summary(&subject)
            .is_some_and(|summary| !summary.is_empty() && lower_technical.contains(summary))
}

pub(crate) fn commit_summary(subject: &str) -> Option<&str> {
    subject.split_once(':').map(|(_, summary)| summary.trim())
}

pub(crate) fn classify_release_context_from_text(
    technical: &str,
    sources: &[ContextSource],
    source: &str,
) -> ReleaseClassification {
    let lower = technical.to_ascii_lowercase();
    let mut categories = BTreeSet::new();
    let mut reasons = Vec::new();
    let docs = lower.contains("docs:")
        || lower.contains("documentation")
        || lower.contains("readme")
        || lower.contains(".md");
    let chore = lower.contains("chore:")
        || lower.contains("ci:")
        || lower.contains("build:")
        || lower.contains("test:")
        || lower.contains("refactor:");
    let dependencies = lower.contains("dependabot")
        || lower.contains("dependency")
        || lower.contains("dependencies")
        || lower.contains("package-lock")
        || lower.contains("cargo.lock");
    let internal = lower.contains("workflow")
        || lower.contains(".github/")
        || lower.contains("script")
        || lower.contains("harness")
        || lower.contains("replay");
    let mut user_visible = lower.contains("feat:")
        || lower.contains("fix:")
        || lower.contains("user")
        || lower.contains("public")
        || lower.contains("cli")
        || lower.contains("action input")
        || lower.contains("release notes");
    let breaking = lower.contains("breaking change")
        || Regex::new(r"(?m)^[*-]?\s*[a-z]+(\([^)]*\))?!:")
            .unwrap()
            .is_match(technical);
    let security = lower.contains("security")
        || lower.contains("vulnerability")
        || lower.contains("cve-")
        || lower.contains("secret");
    let migration_heavy = lower.contains("migration")
        || lower.contains("migrate")
        || lower.contains("deprecat")
        || lower.contains("manifest")
        || lower.contains("configuration");

    if docs {
        categories.insert("docs-only");
        reasons.push("documentation signals detected".to_string());
    }
    if chore {
        categories.insert("chore-only");
        reasons.push("chore/build/test/refactor signals detected".to_string());
    }
    if dependencies {
        categories.insert("dependency-only");
        reasons.push("dependency update signals detected".to_string());
    }
    if internal {
        categories.insert("internal-tooling");
        reasons.push("internal tooling or workflow signals detected".to_string());
    }
    if user_visible {
        categories.insert("user-visible");
        reasons.push("feature, fix, CLI, or public-surface signals detected".to_string());
    }
    if breaking {
        categories.insert("breaking");
        reasons.push("breaking-change signals detected".to_string());
    }
    if security {
        categories.insert("security");
        reasons.push("security-sensitive signals detected".to_string());
    }
    if migration_heavy {
        categories.insert("migration-heavy");
        reasons.push("migration or configuration signals detected".to_string());
    }
    if sources
        .iter()
        .any(|source| source.name == "pull_requests" && source.included)
    {
        reasons.push("PR metadata contributed to context".to_string());
    }
    if categories.is_empty() {
        categories.insert("user-visible");
        user_visible = true;
        reasons.push("no low-value-only signals found; defaulting to user-visible".to_string());
    }

    let low_value_only = !user_visible && !breaking && !security && !migration_heavy;
    let significance = if breaking || security || migration_heavy {
        "high"
    } else if low_value_only {
        "low"
    } else {
        "medium"
    }
    .to_string();

    ReleaseClassification {
        categories: categories.into_iter().map(str::to_string).collect(),
        significance,
        user_visible,
        breaking,
        security,
        migration_heavy,
        source: source.into(),
        model: String::new(),
        deterministic_signals: Vec::new(),
        disagreements: Vec::new(),
        reasons,
    }
}
