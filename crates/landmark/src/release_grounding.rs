use crate::*;

#[derive(Clone, Debug)]
pub(crate) struct ReleaseGrounding {
    pub(crate) technical_changelog: String,
    pub(crate) deterministic: DeterministicReleaseContext,
    pub(crate) metadata: ReleaseGroundingMetadata,
}

#[derive(Clone, Debug)]
struct GroundingCandidate {
    source: &'static str,
    text: String,
    agrees_with_commits: bool,
}

pub(crate) fn resolve_release_grounding(
    args: &SynthesizeArgs,
    config: &EffectiveSynthesisConfig,
) -> Result<ReleaseGrounding> {
    let deterministic = deterministic_release_context(args, config);
    let (changelog_section, changelog_missing_section) =
        resolve_changelog_section(&args.changelog_file, &args.version)?;
    let release_body = read_optional_file(&args.release_body_file)?;
    let pull_requests = read_optional_file(&args.pr_changelog_file)?;
    let candidates = grounding_candidates(
        &deterministic,
        &changelog_section,
        &release_body,
        &pull_requests,
    );
    let source = config.changelog_source.to_ascii_lowercase();
    let mut warnings = Vec::new();
    if changelog_missing_section {
        warnings.push(format!(
            "CHANGELOG.md has no section for {}; the release commit range is authoritative",
            args.version
        ));
    }

    let selected = match source.as_str() {
        "auto" => {
            let selected = candidates
                .iter()
                .find(|candidate| candidate.agrees_with_commits)
                .cloned();
            if selected.is_none() {
                for candidate in candidates.iter().filter(|candidate| {
                    !candidate.agrees_with_commits && !deterministic.commits.is_empty()
                }) {
                    warnings.push(format!(
                        "auto {} source did not match release commits; using the git range as ground truth",
                        candidate.source
                    ));
                }
            }
            selected
        }
        "changelog" | "release-body" | "prs" => Some(
            // Explicit overrides are operator intent, so keep the requested
            // source visible even when it mismatches. The warning and commit
            // list make the conflict reviewable instead of silently guessing.
            candidates
                .iter()
                .find(|candidate| candidate.source == source)
                .cloned()
                .ok_or_else(|| required_source_error(&source, args))?,
        ),
        _ => return Err(format!("invalid changelog-source {source}").into()),
    };

    if let Some(candidate) = &selected
        && !candidate.agrees_with_commits
        && !deterministic.commits.is_empty()
    {
        warnings.push(format!(
            "selected {} source does not match release commits; prefer the commit list below",
            candidate.source
        ));
    }

    let selected_source = selected
        .as_ref()
        .map(|candidate| candidate.source.to_string())
        .unwrap_or_else(|| "git-range".into());
    let selected_source_status = selected
        .as_ref()
        .map(|candidate| {
            if candidate.agrees_with_commits {
                "matched"
            } else {
                "mismatched"
            }
        })
        .unwrap_or("commit-range")
        .to_string();
    let metadata = ReleaseGroundingMetadata {
        selected_source,
        selected_source_status,
        warnings: warnings.clone(),
        commit_count: deterministic.commits.len(),
        diff_stat_count: deterministic.diff_stats.len(),
        changelog_section: optional_source_summary(changelog_section.as_deref()),
        release_body: optional_source_summary(release_body.as_deref()),
        pull_requests: optional_source_summary(pull_requests.as_deref()),
    };
    let technical_changelog =
        render_grounded_technical_changelog(args, &deterministic, selected.as_ref(), &warnings);

    Ok(ReleaseGrounding {
        technical_changelog,
        deterministic,
        metadata,
    })
}

#[cfg(test)]
pub(crate) fn resolve_technical_changelog(
    args: &SynthesizeArgs,
    config: &EffectiveSynthesisConfig,
) -> Result<String> {
    Ok(resolve_release_grounding(args, config)?.technical_changelog)
}

fn resolve_changelog_section(path: &Path, version: &str) -> Result<(Option<String>, bool)> {
    if !path.is_file() {
        return Ok((None, false));
    }
    let text = fs::read_to_string(path)?;
    if text.trim().is_empty() {
        return Ok((None, false));
    }
    let section = extract_release_section(&text, version);
    let missing_section = section.is_none();
    Ok((section, missing_section))
}

fn grounding_candidates(
    deterministic: &DeterministicReleaseContext,
    changelog_section: &Option<String>,
    release_body: &Option<String>,
    pull_requests: &Option<String>,
) -> Vec<GroundingCandidate> {
    [
        ("changelog", changelog_section),
        ("release-body", release_body),
        ("prs", pull_requests),
    ]
    .into_iter()
    .filter_map(|(source, text)| {
        let text = text.as_ref()?;
        Some(GroundingCandidate {
            source,
            text: text.clone(),
            agrees_with_commits: source_agrees_with_commits(text, deterministic),
        })
    })
    .collect()
}

fn source_agrees_with_commits(text: &str, deterministic: &DeterministicReleaseContext) -> bool {
    deterministic.commits.is_empty() || !release_relevant_commits(text, deterministic).is_empty()
}

fn required_source_error(source: &str, args: &SynthesizeArgs) -> String {
    match source {
        "changelog" => format!(
            "CHANGELOG.md is missing, empty, or has no section for {}",
            args.version
        ),
        "release-body" => "release body source is missing or empty".into(),
        "prs" => "PR changelog source is missing or empty".into(),
        _ => format!("invalid changelog-source {source}"),
    }
}

fn optional_source_summary(text: Option<&str>) -> ContextOptionalSource {
    ContextOptionalSource {
        present: text.is_some_and(|text| !text.trim().is_empty()),
        estimated_tokens: text.map(estimate_tokens).unwrap_or(0),
    }
}

fn render_grounded_technical_changelog(
    args: &SynthesizeArgs,
    deterministic: &DeterministicReleaseContext,
    selected: Option<&GroundingCandidate>,
    warnings: &[String],
) -> String {
    let mut rendered = format!("Release-scoped ground truth for {}\n\n", args.version);
    if !warnings.is_empty() {
        rendered.push_str("Grounding warnings:\n");
        for warning in warnings {
            rendered.push_str(&format!("- {warning}\n"));
        }
        rendered.push('\n');
    }
    rendered.push_str("Release commits (authoritative):\n");
    if deterministic.commits.is_empty() {
        rendered.push_str("- No release commits found in the resolved git range.\n");
    } else {
        for commit in &deterministic.commits {
            rendered.push_str(&format!("- {}", commit.subject));
            if !commit.short_hash.trim().is_empty() {
                rendered.push_str(&format!(" ({})", commit.short_hash));
            }
            rendered.push('\n');
            for line in commit
                .body
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
            {
                rendered.push_str(&format!("  {line}\n"));
            }
        }
    }
    rendered.push('\n');
    rendered.push_str("Diff stats:\n");
    if deterministic.diff_stats.is_empty() {
        rendered.push_str("- No file-level diff stats recorded.\n");
    } else {
        for stat in deterministic.diff_stats.iter().take(20) {
            rendered.push_str(&format!(
                "- {} (+{} -{})\n",
                stat.path, stat.additions, stat.deletions
            ));
        }
    }
    if let Some(candidate) = selected {
        rendered.push('\n');
        rendered.push_str(&format!(
            "Selected technical source ({}; {}):\n\n{}\n",
            candidate.source,
            if candidate.agrees_with_commits {
                "matched release commits"
            } else {
                "does not match release commits"
            },
            candidate.text.trim()
        ));
    }
    rendered.trim().to_string()
}
