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

pub(crate) fn classify_release_context_with_model(
    technical: &str,
    sources: &[ContextSource],
    deterministic: &DeterministicReleaseContext,
    api_url: &str,
    api_key: &str,
    models: &[String],
) -> ReleaseClassification {
    let deterministic_classification =
        classify_release_context_with_deterministic(technical, sources, deterministic);
    if api_key.trim().is_empty() || models.is_empty() {
        return deterministic_classification;
    }

    let mut errors = Vec::new();
    for model in models {
        match request_release_classification(
            api_url,
            api_key,
            model,
            technical,
            sources,
            deterministic,
        ) {
            Ok(classification) => {
                return apply_deterministic_floor(classification, &deterministic_classification);
            }
            Err(error) => errors.push(format!("{model}: {}", sanitize_text(&error.to_string()))),
        }
    }

    let mut classification = deterministic_classification;
    classification.source = "structured-fallback".into();
    classification.reasons.push(format!(
        "model classification unavailable; deterministic floor used: {}",
        errors.join("; ")
    ));
    classification
}

pub(crate) fn release_classification_models(
    config: &EffectiveSynthesisConfig,
    api_url: &str,
) -> Vec<String> {
    if config.model_policy.trim().eq_ignore_ascii_case("off") {
        return Vec::new();
    }
    let mut models = Vec::new();
    let primary = config.model.trim();
    let openrouter = api_url.to_ascii_lowercase().contains("openrouter.ai");
    if openrouter && !config.model_policy.trim().eq_ignore_ascii_case("cheap") {
        push_unique_model(&mut models, "openai/gpt-4o-mini");
    } else if !primary.is_empty() && primary != "off" {
        push_unique_model(&mut models, primary);
    } else {
        push_unique_model(&mut models, "openai/gpt-4o-mini");
    }
    for model in config
        .fallback_models
        .split(',')
        .map(str::trim)
        .filter(|model| !model.is_empty())
    {
        push_unique_model(&mut models, model);
    }
    if openrouter {
        push_unique_model(&mut models, "openai/gpt-4o-mini");
    }
    models
}

pub(crate) fn push_unique_model(models: &mut Vec<String>, model: &str) {
    if !models.iter().any(|existing| existing == model) {
        models.push(model.to_string());
    }
}

pub(crate) fn request_release_classification(
    api_url: &str,
    api_key: &str,
    model: &str,
    technical: &str,
    sources: &[ContextSource],
    deterministic: &DeterministicReleaseContext,
) -> Result<ReleaseClassification> {
    let input = json!({
        "task": "classify_release_importance",
        "rendered_changelog_context": technical,
        "context_sources": sources,
        "deterministic": deterministic,
        "output_schema": {
            "type": "object",
            "required": [
                "categories",
                "significance",
                "user_visible",
                "breaking",
                "security",
                "migration_heavy",
                "reasons"
            ],
            "properties": {
                "categories": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": [
                            "user-visible",
                            "docs-only",
                            "chore-only",
                            "dependency-only",
                            "internal-tooling",
                            "breaking",
                            "security",
                            "migration-heavy"
                        ]
                    }
                },
                "significance": { "type": "string", "enum": ["low", "medium", "high"] },
                "user_visible": { "type": "boolean" },
                "breaking": { "type": "boolean" },
                "security": { "type": "boolean" },
                "migration_heavy": { "type": "boolean" },
                "reasons": { "type": "array", "items": { "type": "string" } }
            }
        }
    });
    let payload = json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You classify software releases. Return only one strict JSON object matching the requested schema. Conventional commit feat, fix, perf, and breaking signals are floor evidence, not final prose."
            },
            {
                "role": "user",
                "content": serde_json::to_string_pretty(&input)?
            }
        ],
        "temperature": 0,
        "max_tokens": 700
    });
    let response = curl_json("POST", api_url, Some(api_key), Some(&payload))?;
    if !(200..300).contains(&response.status) {
        return Err(format!("HTTP {}", response.status).into());
    }
    let value: Value = serde_json::from_str(&response.body)?;
    let content = value["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("provider response did not include choices[0].message.content")?;
    parse_model_release_classification(content, model)
}

pub(crate) fn parse_model_release_classification(
    content: &str,
    model: &str,
) -> Result<ReleaseClassification> {
    let value: Value = serde_json::from_str(extract_json_object(content)?)?;
    let significance = string_field(&value, "significance")?;
    if !matches!(significance.as_str(), "low" | "medium" | "high") {
        return Err(format!(
            "classification significance must be low, medium, or high; got {significance}"
        )
        .into());
    }
    Ok(ReleaseClassification {
        categories: string_array_field(&value, "categories")?,
        significance,
        user_visible: bool_field(&value, "user_visible")?,
        breaking: bool_field(&value, "breaking")?,
        security: bool_field(&value, "security")?,
        migration_heavy: bool_field(&value, "migration_heavy")?,
        source: "model".into(),
        model: model.into(),
        deterministic_signals: Vec::new(),
        disagreements: Vec::new(),
        reasons: string_array_field(&value, "reasons")?,
    })
}

pub(crate) fn extract_json_object(content: &str) -> Result<&str> {
    let trimmed = content.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Ok(trimmed);
    }
    let start = trimmed
        .find('{')
        .ok_or("classification response did not contain a JSON object")?;
    let end = trimmed
        .rfind('}')
        .ok_or("classification response did not contain a complete JSON object")?;
    if end < start {
        return Err("classification response JSON object was malformed".into());
    }
    Ok(&trimmed[start..=end])
}

pub(crate) fn string_field(value: &Value, field: &str) -> Result<String> {
    value[field]
        .as_str()
        .map(sanitize_text)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("classification field {field} must be a non-empty string").into())
}

pub(crate) fn bool_field(value: &Value, field: &str) -> Result<bool> {
    value[field]
        .as_bool()
        .ok_or_else(|| format!("classification field {field} must be a boolean").into())
}

pub(crate) fn string_array_field(value: &Value, field: &str) -> Result<Vec<String>> {
    let values = value[field]
        .as_array()
        .ok_or_else(|| format!("classification field {field} must be an array"))?;
    let mut out = Vec::new();
    for item in values {
        let value = item
            .as_str()
            .map(sanitize_text)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("classification field {field} must contain only strings"))?;
        out.push(value);
    }
    Ok(out)
}

pub(crate) fn apply_deterministic_floor(
    mut classification: ReleaseClassification,
    deterministic: &ReleaseClassification,
) -> ReleaseClassification {
    for signal in &deterministic.deterministic_signals {
        if !classification
            .deterministic_signals
            .iter()
            .any(|existing| existing == signal)
        {
            classification.deterministic_signals.push(signal.clone());
        }
    }

    let user_visible_floor = deterministic.deterministic_signals.iter().any(|signal| {
        matches!(
            signal.as_str(),
            "conventional:feat" | "conventional:fix" | "conventional:perf" | "breaking"
        )
    });
    if user_visible_floor && !classification.user_visible {
        classification.user_visible = true;
        push_unique_category(&mut classification.categories, "user-visible");
        classification
            .disagreements
            .push("deterministic floor found user-visible commit signals but model did not".into());
    }
    for (field, category, value) in [
        ("breaking", "breaking", deterministic.breaking),
        ("security", "security", deterministic.security),
        (
            "migration-heavy",
            "migration-heavy",
            deterministic.migration_heavy,
        ),
    ] {
        if value
            && !classification
                .categories
                .iter()
                .any(|existing| existing == category)
        {
            push_unique_category(&mut classification.categories, category);
        }
        if field == "breaking" && value && !classification.breaking {
            classification.breaking = true;
            classification
                .disagreements
                .push("deterministic floor found breaking signals but model did not".into());
        }
        if field == "security" && value && !classification.security {
            classification.security = true;
            classification
                .disagreements
                .push("deterministic floor found security signals but model did not".into());
        }
        if field == "migration-heavy" && value && !classification.migration_heavy {
            classification.migration_heavy = true;
            classification
                .disagreements
                .push("deterministic floor found migration signals but model did not".into());
        }
    }
    if (user_visible_floor || classification.breaking || classification.security)
        && classification.significance == "low"
    {
        classification.significance = if classification.breaking || classification.security {
            "high".into()
        } else {
            "medium".into()
        };
        classification
            .disagreements
            .push("deterministic floor prevented low-significance skip".into());
    }
    classification
}

pub(crate) fn push_unique_category(categories: &mut Vec<String>, category: &str) {
    if !categories.iter().any(|existing| existing == category) {
        categories.push(category.to_string());
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
