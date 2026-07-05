use crate::*;

pub(crate) fn healthcheck(args: HealthcheckArgs) -> Result<()> {
    let payload = json!({
        "model": args.model,
        "messages": [{"role": "user", "content": "Reply with ok."}],
        "max_tokens": 8
    });
    match curl_json("POST", &args.api_url, Some(&args.api_key), Some(&payload)) {
        Ok(response) if (200..300).contains(&response.status) => {
            let value: Value = serde_json::from_str(&response.body)?;
            let content = value["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .trim();
            if content.is_empty() {
                return healthcheck_fail(args.warn_only, "LLM healthcheck returned empty content");
            }
            Ok(())
        }
        Ok(response) => healthcheck_fail(
            args.warn_only,
            &format!("LLM healthcheck failed with HTTP {}", response.status),
        ),
        Err(error) => healthcheck_fail(
            args.warn_only,
            &format!("LLM healthcheck request failed: {error}"),
        ),
    }
}

pub(crate) fn healthcheck_fail(warn_only: bool, message: &str) -> Result<()> {
    if warn_only {
        eprintln!("::warning::{message}");
        Ok(())
    } else {
        Err(message.to_string().into())
    }
}

pub(crate) fn preflight_tags() -> Result<()> {
    let tags = run_ok("git", ["tag", "--list", "v*"], Path::new("."))?;
    let orphaned: Vec<_> = tags
        .lines()
        .filter(|tag| semver_from_tag(tag).is_some())
        .filter(|tag| {
            let status = Command::new("git")
                .args(["rev-list", "-n", "1", tag])
                .status()
                .map(|status| status.success())
                .unwrap_or(false);
            !status
        })
        .collect();
    if orphaned.is_empty() {
        Ok(())
    } else {
        Err(format!("orphaned release tags: {}", orphaned.join(", ")).into())
    }
}

pub(crate) fn fetch_release_body(args: FetchReleaseBodyArgs) -> Result<()> {
    let provider = GitHubProvider::required(&args.api_base_url, &args.github_token);
    let value = provider.release_by_tag(&args.repository, &args.release_tag)?;
    ensure_parent(&args.output_file)?;
    fs::write(
        args.output_file,
        value
            .as_ref()
            .and_then(|release| release["body"].as_str())
            .unwrap_or(""),
    )?;
    Ok(())
}

pub(crate) fn extract_prs(args: ExtractPrsArgs) -> Result<()> {
    let provider = GitHubProvider::required(&args.api_base_url, &args.github_token);
    let (previous_tag, target_tag) = context_git_range(&args.repo_root, &args.release_tag);
    let since = if previous_tag.is_empty() {
        None
    } else {
        git_commit_date(&args.repo_root, &previous_tag)
    };
    let until = git_commit_date(&args.repo_root, &target_tag);
    let prs = provider.closed_pull_requests(&args.repository, since)?;
    let scoped = filter_prs_by_range(&prs, since, until);
    let mut rendered = String::new();
    for pr in &scoped {
        let number = pr["number"].as_i64().unwrap_or_default();
        let title = pr["title"].as_str().unwrap_or("Untitled");
        let user = pr["user"]["login"].as_str().unwrap_or("unknown");
        rendered.push_str(&format!("- {title} (#{number}) by @{user}\n"));
    }
    if rendered.is_empty() {
        rendered.push_str(&format!("Release {}\n", args.release_tag));
    }
    ensure_parent(&args.output_file)?;
    fs::write(args.output_file, rendered)?;
    Ok(())
}

pub(crate) fn synthesize(args: SynthesizeArgs) -> Result<()> {
    let config = resolve_synthesis_config(&args)?;
    let grounding = resolve_release_grounding(&args, &config)?;
    let prompt = render_prompt(&args, &config, &grounding.technical_changelog)?;
    let context = if args.dry_run_cost {
        synthesis_context_packet(&args, &config, &grounding, &prompt)
    } else {
        synthesis_context_packet_with_model(&args, &config, &grounding, &prompt)
    };
    write_json_if_requested(&args.context_metadata_file, &context)?;
    if args.dry_run_cost {
        println!("{}", serde_json::to_string_pretty(&context)?);
        return Ok(());
    }
    if context.cost.skip {
        write_json_if_requested(
            &args.attempts_file,
            &vec![json!({
                "model": context.cost.model,
                "succeeded": false,
                "quality": "skipped",
                "message": context.cost.skip_reason,
                "cost": context.cost.clone(),
                "classification": context.classification.clone(),
                "decision": context.decision.clone(),
            })],
        )?;
        ensure_parent(&args.quality_file)?;
        fs::write(&args.quality_file, "skipped")?;
        return Ok(());
    }
    validate_nonblank(&args.api_key, "api-key")?;
    validate_nonblank(&context.cost.model, "model")?;
    let mut models = vec![context.cost.model.clone()];
    models.extend(
        config
            .fallback_models
            .split(',')
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .map(str::to_string),
    );
    let mut last_error = String::new();
    let mut attempts = Vec::new();
    let mut last_ungrounded: Option<ClaimSourceMap> = None;
    for model in models {
        match request_synthesis(&args.api_url, &args.api_key, &model, &prompt) {
            Ok(notes) if !notes.trim().is_empty() => {
                let claim_map = build_claim_source_map(&notes, &grounding.deterministic);
                let quality = if !validate_notes(&notes) {
                    "degraded"
                } else if !claim_map.grounded {
                    "ungrounded"
                } else {
                    "valid"
                };
                attempts.push(json!({
                    "model": model,
                    "succeeded": true,
                    "quality": quality,
                    "message": "",
                    "cost": context.cost.clone(),
                    "classification": context.classification.clone(),
                    "decision": context.decision.clone(),
                    "claim_map": claim_map,
                }));
                if quality == "ungrounded" {
                    // Structurally valid Markdown that names a change with zero
                    // supporting release commits is exactly the canary v1.14.0
                    // failure mode (invented Breaking Changes / Bug Fixes for a
                    // single real feat PR). Never accept it — try the next
                    // fallback model instead of publishing fiction.
                    last_error = format!(
                        "model {model} fabricated release-note sections with no grounded source evidence: {}",
                        claim_map.ungrounded_sections.join(", ")
                    );
                    last_ungrounded = Some(claim_map);
                    continue;
                }
                let notes = notes_with_classification_notice(&notes, &context.classification);
                write_json_if_requested(&args.attempts_file, &attempts)?;
                write_json_if_requested(&args.claim_map_file, &claim_map)?;
                ensure_parent(&args.quality_file)?;
                fs::write(&args.quality_file, quality)?;
                println!("{}", notes.trim());
                return Ok(());
            }
            Ok(_) => {
                last_error = format!("model {model} returned empty content");
                attempts.push(json!({
                    "model": model,
                    "succeeded": false,
                    "quality": "failed",
                    "message": last_error,
                    "cost": context.cost.clone(),
                    "classification": context.classification.clone(),
                    "decision": context.decision.clone(),
                }));
            }
            Err(error) => {
                last_error = format!("model {model} failed: {error}");
                attempts.push(json!({
                    "model": model,
                    "succeeded": false,
                    "quality": "failed",
                    "message": last_error,
                    "cost": context.cost.clone(),
                    "classification": context.classification.clone(),
                    "decision": context.decision.clone(),
                }));
            }
        }
    }
    write_json_if_requested(&args.attempts_file, &attempts)?;
    if let Some(claim_map) = last_ungrounded {
        // Every model that returned content fabricated at least one section.
        // Record the grounding verdict for post-mortem evidence, but still
        // refuse: an explicit failure, never a silently "valid" fabrication.
        write_json_if_requested(&args.claim_map_file, &claim_map)?;
        ensure_parent(&args.quality_file)?;
        fs::write(&args.quality_file, "ungrounded")?;
    }
    Err(last_error.into())
}

pub(crate) fn resolve_synthesis_config(args: &SynthesizeArgs) -> Result<EffectiveSynthesisConfig> {
    let manifest = load_manifest(&args.repo_root)?.unwrap_or_default();
    let product_name = nonblank_or(
        &args.product_name,
        manifest.product.name.as_deref(),
        "product-name",
    )?;
    let product_description = nonblank_or_default(
        &args.product_description,
        manifest.product.description.as_deref(),
    );
    let voice_guide = nonblank_or_default(&args.voice_guide, manifest.voice.as_deref());
    let audience = optional_or_default(
        args.audience.as_deref(),
        manifest.audience.as_deref(),
        "general",
    );
    let changelog_source = optional_or_default(
        args.changelog_source.as_deref(),
        manifest.changelog.source.as_deref(),
        "auto",
    );
    let model_policy = optional_or_default(
        Some(args.model_policy.as_str()),
        manifest.model.policy.as_deref(),
        "balanced",
    );
    let model = trimmed_option(&args.model)
        .or_else(|| manifest.model.primary.as_deref().and_then(trimmed_option))
        .or_else(|| policy_default_model(Some(&model_policy)))
        .unwrap_or_default();
    let fallback_models = if !args.fallback_models.trim().is_empty() {
        args.fallback_models.trim().to_string()
    } else {
        manifest.model.fallbacks.join(",")
    };
    Ok(EffectiveSynthesisConfig {
        product_name,
        product_description,
        voice_guide,
        audience,
        changelog_source,
        model_policy,
        model,
        fallback_models,
        max_input_tokens: manifest.budget.max_input_tokens,
        max_output_tokens: manifest.budget.max_output_tokens,
        max_usd: manifest.budget.max_usd,
    })
}

pub(crate) fn nonblank_or(value: &str, manifest: Option<&str>, name: &str) -> Result<String> {
    if let Some(value) = trimmed_option(value) {
        return Ok(value);
    }
    if let Some(value) = manifest.and_then(trimmed_option) {
        return Ok(value);
    }
    Err(format!("{name} must not be blank").into())
}

pub(crate) fn nonblank_or_default(value: &str, manifest: Option<&str>) -> String {
    trimmed_option(value)
        .or_else(|| manifest.and_then(trimmed_option))
        .unwrap_or_default()
}

pub(crate) fn optional_or_default(
    value: Option<&str>,
    manifest: Option<&str>,
    default: &str,
) -> String {
    value
        .and_then(trimmed_option)
        .or_else(|| manifest.and_then(trimmed_option))
        .unwrap_or_else(|| default.to_string())
}

pub(crate) fn policy_default_model(policy: Option<&str>) -> Option<String> {
    let tier = match policy.and_then(trimmed_option).as_deref() {
        Some("off") => "off",
        Some("cheap") => "cheap",
        Some("rich") => "rich",
        _ => "balanced",
    };
    Some(default_model_for_tier(tier).into())
}

/// Single source of truth for model-tier pins. `cheap_model()`, `rich_model()`,
/// `policy_default_model()`, and `release_classification_models()` all read
/// their defaults from here instead of hardcoding literals independently —
/// that independent hardcoding is exactly how `openai/gpt-4o-mini` and
/// `anthropic/claude-sonnet-4` went stale without anyone noticing. When a
/// pin needs to move, update it once, here, and bump the review date.
/// See backlog.d/013-refresh-model-defaults-and-fix-config-override-bug.md.
pub(crate) fn default_model_for_tier(tier: &str) -> &'static str {
    match tier {
        "off" => "off",
        // model pin reviewed: 2026-07
        "cheap" => "anthropic/claude-haiku-4.5",
        // model pin reviewed: 2026-07
        "rich" | "balanced" => "anthropic/claude-sonnet-5",
        // model pin reviewed: 2026-07
        "classification" => "deepseek/deepseek-v4-flash",
        // model pin reviewed: 2026-07
        "classification-fallback" => "anthropic/claude-haiku-4.5",
        // model pin reviewed: 2026-07
        _ => "anthropic/claude-sonnet-5",
    }
}

pub(crate) fn read_optional_file(path: &Path) -> Result<Option<String>> {
    if path.as_os_str().is_empty() || !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    if text.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

pub(crate) fn extract_release_section(text: &str, version: &str) -> Option<String> {
    let normalized =
        normalize_version(version).unwrap_or_else(|_| version.trim_start_matches('v').to_string());
    let heading = Regex::new(r"(?m)^##\s+\[?v?([0-9]+\.[0-9]+\.[0-9][^\]\s]*)\]?.*$").unwrap();
    let matches: Vec<_> = heading.find_iter(text).collect();
    for (index, mat) in matches.iter().enumerate() {
        let line = text[mat.start()..mat.end()].to_string();
        if line.contains(&normalized) || line.contains(version) {
            let end = matches
                .get(index + 1)
                .map(|next| next.start())
                .unwrap_or(text.len());
            return Some(text[mat.start()..end].trim().to_string());
        }
    }
    // No heading matches this release. Silently returning the first (most recent)
    // section here used to hand the model a stale, unrelated changelog as ground
    // truth for the release actually being synthesized — return None instead and
    // let the caller fail loudly or fall back to a different source.
    None
}

pub(crate) fn render_prompt(
    args: &SynthesizeArgs,
    config: &EffectiveSynthesisConfig,
    technical: &str,
) -> Result<String> {
    let template = if args.prompt_template.is_file() {
        fs::read_to_string(&args.prompt_template)?
    } else {
        let filename = match config.audience.as_str() {
            "developer" | "end-user" | "enterprise" | "general" => {
                format!("{}.md", config.audience)
            }
            _ => return Err(format!("invalid audience {}", config.audience).into()),
        };
        let path = args.templates_dir.join(filename);
        if path.is_file() {
            fs::read_to_string(path)?
        } else {
            fs::read_to_string("templates/synthesis-prompt.md")?
        }
    };
    let product_context = if config.product_description.trim().is_empty() {
        String::new()
    } else {
        format!("Product context: {}\n", config.product_description.trim())
    };
    let voice_guide = if config.voice_guide.trim().is_empty() {
        String::new()
    } else {
        format!("Voice guide: {}\n", config.voice_guide.trim())
    };
    Ok(template
        .replace("{{PRODUCT_NAME}}", &config.product_name)
        .replace("{{VERSION}}", &args.version)
        .replace("{{TECHNICAL_CHANGELOG}}", technical)
        .replace("{{PRODUCT_CONTEXT}}", &product_context)
        .replace("{{VOICE_GUIDE}}", &voice_guide)
        .replace("{{BULLET_TARGET}}", "4")
        .replace("{{BREAKING_CHANGES}}", &render_breaking_changes(technical)))
}

pub(crate) fn synthesis_context_packet(
    args: &SynthesizeArgs,
    config: &EffectiveSynthesisConfig,
    grounding: &ReleaseGrounding,
    prompt: &str,
) -> SynthesisContextPacket {
    let sources = synthesis_context_sources(args, config, grounding, prompt);
    let classification = classify_release_context_with_deterministic(
        &grounding.technical_changelog,
        &sources,
        &grounding.deterministic,
    );
    synthesis_context_packet_from_classification(
        args,
        config,
        prompt,
        sources,
        grounding,
        classification,
    )
}

pub(crate) fn synthesis_context_packet_with_model(
    args: &SynthesizeArgs,
    config: &EffectiveSynthesisConfig,
    grounding: &ReleaseGrounding,
    prompt: &str,
) -> SynthesisContextPacket {
    if args.dry_run_cost {
        return synthesis_context_packet(args, config, grounding, prompt);
    }
    let sources = synthesis_context_sources(args, config, grounding, prompt);
    let models = release_classification_models(config);
    let classification = classify_release_context_with_model(
        &grounding.technical_changelog,
        &sources,
        &grounding.deterministic,
        &args.api_url,
        &args.api_key,
        &models,
    );
    synthesis_context_packet_from_classification(
        args,
        config,
        prompt,
        sources,
        grounding,
        classification,
    )
}

pub(crate) fn synthesis_context_packet_from_classification(
    args: &SynthesizeArgs,
    config: &EffectiveSynthesisConfig,
    prompt: &str,
    sources: Vec<ContextSource>,
    grounding: &ReleaseGrounding,
    classification: ReleaseClassification,
) -> SynthesisContextPacket {
    let cost = estimate_synthesis_cost(config, prompt, &classification, &sources);
    let decision = synthesis_decision(config, &cost, &classification);
    SynthesisContextPacket {
        product: ContextProduct {
            name: config.product_name.clone(),
            audience: config.audience.clone(),
            description: config.product_description.clone(),
        },
        release: ContextRelease {
            version: args.version.clone(),
            changelog_source: config.changelog_source.clone(),
            model_policy: config.model_policy.clone(),
        },
        grounding: grounding.metadata.clone(),
        deterministic: grounding.deterministic.clone(),
        sources,
        classification,
        cost,
        decision,
    }
}

pub(crate) fn deterministic_release_context(
    args: &SynthesizeArgs,
    config: &EffectiveSynthesisConfig,
) -> DeterministicReleaseContext {
    let repo_root = &args.repo_root;
    DeterministicReleaseContext {
        commits: context_commits(repo_root, &args.version),
        tags: context_tags(repo_root),
        changed_files: context_changed_files(repo_root, &args.version),
        diff_stats: context_diff_stats(repo_root, &args.version),
        manifest: ContextManifestSummary {
            present: repo_root.join(".landmark.yml").is_file(),
            product_name: config.product_name.clone(),
            audience: config.audience.clone(),
            model_policy: config.model_policy.clone(),
        },
        docs: context_documents(repo_root),
        package: context_package(repo_root),
        prior_releases: context_prior_releases(repo_root),
        pr_metadata: context_optional_source(&args.pr_changelog_file),
        release_body: context_optional_source(&args.release_body_file),
        artifacts: ContextArtifactAudiences {
            internal_technical_changelog: "landmark.internal-technical-changelog.v1".into(),
            public_release_notes: format!("landmark.public-release-notes.v1:{}", config.audience),
        },
    }
}

pub(crate) fn context_commits(repo_root: &Path, version: &str) -> Vec<ContextCommit> {
    let (previous, target) = context_git_range(repo_root, version);
    local_release_commits(repo_root, &previous, &target)
        .unwrap_or_default()
        .into_iter()
        .take(30)
        .map(|commit| ContextCommit {
            conventional_type: conventional_commit_type(&commit.subject)
                .unwrap_or("")
                .to_string(),
            breaking: is_breaking_commit(&commit),
            subject: commit.subject,
            body: commit.body,
            short_hash: commit.short_hash,
        })
        .collect()
}

pub(crate) fn context_diff_range(repo_root: &Path, version: &str) -> String {
    let (previous, target) = context_git_range(repo_root, version);
    if previous.trim().is_empty() {
        format!("{}..{target}", empty_git_tree())
    } else {
        format!("{previous}..{target}")
    }
}

pub(crate) fn context_changed_files(repo_root: &Path, version: &str) -> Vec<String> {
    let range = context_diff_range(repo_root, version);
    run_ok("git", ["diff", "--name-only", range.as_str()], repo_root)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(100)
        .map(str::to_string)
        .collect()
}

pub(crate) fn context_diff_stats(repo_root: &Path, version: &str) -> Vec<ContextDiffStat> {
    let range = context_diff_range(repo_root, version);
    run_ok("git", ["diff", "--numstat", range.as_str()], repo_root)
        .unwrap_or_default()
        .lines()
        .filter_map(parse_numstat_line)
        .take(100)
        .collect()
}

pub(crate) fn parse_numstat_line(line: &str) -> Option<ContextDiffStat> {
    let mut parts = line.splitn(3, '\t');
    let additions = parts.next()?.trim();
    let deletions = parts.next()?.trim();
    let path = parts.next()?.trim();
    if path.is_empty() {
        return None;
    }
    let binary = additions == "-" || deletions == "-";
    Some(ContextDiffStat {
        path: path.to_string(),
        additions: additions.parse().unwrap_or(0),
        deletions: deletions.parse().unwrap_or(0),
        binary,
    })
}

pub(crate) fn context_git_range(repo_root: &Path, version: &str) -> (String, String) {
    let tags = backfill_tags(repo_root).unwrap_or_default();
    let normalized = version.trim();
    let target = if tags.iter().any(|tag| tag.tag == normalized) {
        normalized.to_string()
    } else {
        "HEAD".into()
    };
    let previous = tags
        .iter()
        .find(|tag| tag.tag == normalized)
        .and_then(|tag| previous_backfill_tag(&tags, tag))
        .or_else(|| {
            tags.iter()
                .rfind(|tag| !tag.prerelease)
                .map(|tag| tag.tag.clone())
        })
        .filter(|tag| tag != &target)
        .unwrap_or_default();
    (previous, target)
}

pub(crate) fn context_tags(repo_root: &Path) -> Vec<String> {
    backfill_tags(repo_root)
        .unwrap_or_default()
        .into_iter()
        .rev()
        .take(10)
        .map(|tag| tag.tag)
        .collect()
}

pub(crate) fn context_documents(repo_root: &Path) -> Vec<ContextDocument> {
    ["README.md", "docs/README.md"]
        .iter()
        .filter_map(|path| {
            let full = repo_root.join(path);
            let text = fs::read_to_string(&full).ok()?;
            let title = text
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("")
                .trim()
                .trim_start_matches('#')
                .trim()
                .to_string();
            Some(ContextDocument {
                path: (*path).into(),
                title,
                estimated_tokens: estimate_tokens(&text),
            })
        })
        .collect()
}

pub(crate) fn context_package(repo_root: &Path) -> Option<ContextPackage> {
    if let Some(package) = read_package_json(repo_root) {
        return Some(ContextPackage {
            manager: "npm".into(),
            name: package["name"].as_str().unwrap_or("").to_string(),
            description: package["description"].as_str().unwrap_or("").to_string(),
        });
    }
    let cargo = fs::read_to_string(repo_root.join("Cargo.toml")).ok()?;
    let name = Regex::new(r#"(?m)^name\s*=\s*"([^"]+)""#)
        .ok()?
        .captures(&cargo)
        .and_then(|caps| caps.get(1))
        .map(|value| value.as_str().to_string())
        .unwrap_or_default();
    Some(ContextPackage {
        manager: "cargo".into(),
        name,
        description: String::new(),
    })
}

pub(crate) fn context_prior_releases(repo_root: &Path) -> Vec<String> {
    let changelog = fs::read_to_string(repo_root.join("CHANGELOG.md")).unwrap_or_default();
    Regex::new(r"(?m)^##\s+(.+)$")
        .unwrap()
        .captures_iter(&changelog)
        .filter_map(|caps| caps.get(1).map(|value| value.as_str().trim().to_string()))
        .take(5)
        .collect()
}

pub(crate) fn context_optional_source(path: &Path) -> ContextOptionalSource {
    let text = read_optional_file(path).ok().flatten().unwrap_or_default();
    ContextOptionalSource {
        present: !text.trim().is_empty(),
        estimated_tokens: if text.trim().is_empty() {
            0
        } else {
            estimate_tokens(&text)
        },
    }
}

pub(crate) fn synthesis_decision(
    config: &EffectiveSynthesisConfig,
    cost: &CostEstimate,
    classification: &ReleaseClassification,
) -> SynthesisDecision {
    let (action, reason, llm_required) = if cost.skip {
        ("skipped", cost.skip_reason.clone(), false)
    } else if config.model_policy.trim().eq_ignore_ascii_case("balanced")
        && cost.model_tier == "rich"
        && classification.significance == "high"
    {
        (
            "escalated",
            "high-significance release uses rich model tier".into(),
            true,
        )
    } else {
        (
            "used",
            format!(
                "{} policy uses {} model tier",
                config.model_policy, cost.model_tier
            ),
            true,
        )
    };
    SynthesisDecision {
        action: action.into(),
        reason,
        llm_required,
        model_tier: cost.model_tier.clone(),
    }
}

pub(crate) fn empty_git_tree() -> &'static str {
    "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
}

pub(crate) fn synthesis_context_sources(
    args: &SynthesizeArgs,
    config: &EffectiveSynthesisConfig,
    grounding: &ReleaseGrounding,
    prompt: &str,
) -> Vec<ContextSource> {
    let mut sources = vec![
        context_source("prompt_template", "prompt", prompt),
        context_source(
            "technical_changelog",
            &grounding.metadata.selected_source,
            &grounding.technical_changelog,
        ),
    ];
    if !config.product_description.trim().is_empty() {
        sources.push(context_source(
            "product_manifest",
            "manifest",
            &config.product_description,
        ));
    }
    if !config.voice_guide.trim().is_empty() {
        sources.push(context_source(
            "voice_guide",
            "manifest",
            &config.voice_guide,
        ));
    }
    if let Ok(Some(body)) = read_optional_file(&args.release_body_file) {
        sources.push(context_source_with_included(
            "release_body",
            "release-body",
            &body,
            grounding.metadata.selected_source == "release-body",
        ));
    }
    if let Ok(Some(prs)) = read_optional_file(&args.pr_changelog_file) {
        sources.push(context_source_with_included(
            "pull_requests",
            "prs",
            &prs,
            grounding.metadata.selected_source == "prs",
        ));
    }
    sources
}

pub(crate) fn context_source(name: &str, kind: &str, text: &str) -> ContextSource {
    context_source_with_included(name, kind, text, !text.trim().is_empty())
}

pub(crate) fn context_source_with_included(
    name: &str,
    kind: &str,
    text: &str,
    included: bool,
) -> ContextSource {
    ContextSource {
        name: name.to_string(),
        kind: kind.to_string(),
        bytes: text.len(),
        estimated_tokens: estimate_tokens(text),
        included: included && !text.trim().is_empty(),
    }
}

pub(crate) fn estimate_tokens(text: &str) -> u64 {
    let chars = text.chars().count() as u64;
    chars.div_ceil(4).max(1)
}

pub(crate) fn estimate_synthesis_cost(
    config: &EffectiveSynthesisConfig,
    prompt: &str,
    classification: &ReleaseClassification,
    _sources: &[ContextSource],
) -> CostEstimate {
    let policy = config.model_policy.trim().to_ascii_lowercase();
    let (model_tier, model, mut skip, mut skip_reason) =
        selected_model_plan(config, classification);
    let input_tokens = estimate_tokens(prompt);
    let output_tokens = config
        .max_output_tokens
        .unwrap_or(match model_tier.as_str() {
            "cheap" => 700,
            "rich" => 1400,
            _ => 1000,
        });
    if !skip
        && let Some(max_input) = config.max_input_tokens
        && input_tokens > max_input
    {
        skip = true;
        skip_reason =
            format!("estimated input tokens {input_tokens} exceed manifest budget {max_input}");
    }
    let estimated_usd = estimate_model_cost_usd(&model_tier, input_tokens, output_tokens);
    if !skip
        && let Some(max_usd) = config.max_usd
        && estimated_usd > max_usd
    {
        skip = true;
        skip_reason = format!(
            "estimated synthesis cost ${estimated_usd:.4} exceeds manifest budget ${max_usd:.4}"
        );
    }
    if policy == "off" {
        skip = true;
        skip_reason = "model.policy=off disables LLM synthesis".into();
    }
    CostEstimate {
        input_tokens,
        output_tokens,
        model_tier,
        model,
        estimated_usd,
        skip,
        skip_reason,
    }
}

pub(crate) fn selected_model_plan(
    config: &EffectiveSynthesisConfig,
    classification: &ReleaseClassification,
) -> (String, String, bool, String) {
    match config.model_policy.trim().to_ascii_lowercase().as_str() {
        "off" => (
            "off".into(),
            "off".into(),
            true,
            "model.policy=off disables LLM synthesis".into(),
        ),
        "cheap" => ("cheap".into(), cheap_model(config), false, String::new()),
        "rich" => ("rich".into(), rich_model(config), false, String::new()),
        _ if classification.significance == "low" => (
            "off".into(),
            "off".into(),
            true,
            "low-significance docs/chore/dependency release skipped by balanced policy".into(),
        ),
        _ if classification.significance == "high" => {
            ("rich".into(), rich_model(config), false, String::new())
        }
        _ => (
            "balanced".into(),
            config.model.clone(),
            false,
            String::new(),
        ),
    }
}

pub(crate) fn cheap_model(config: &EffectiveSynthesisConfig) -> String {
    if config.model != "off" && !config.model.trim().is_empty() {
        config.model.clone()
    } else {
        default_model_for_tier("cheap").into()
    }
}

pub(crate) fn rich_model(config: &EffectiveSynthesisConfig) -> String {
    if config.model != "off" && !config.model.trim().is_empty() {
        config.model.clone()
    } else {
        default_model_for_tier("rich").into()
    }
}

pub(crate) fn estimate_model_cost_usd(tier: &str, input_tokens: u64, output_tokens: u64) -> f64 {
    let (input_per_million, output_per_million) = match tier {
        "cheap" => (0.15, 0.60),
        "rich" => (3.00, 15.00),
        "off" => (0.0, 0.0),
        _ => (1.00, 5.00),
    };
    ((input_tokens as f64 / 1_000_000.0) * input_per_million)
        + ((output_tokens as f64 / 1_000_000.0) * output_per_million)
}

pub(crate) fn render_breaking_changes(technical: &str) -> String {
    let mut changes = BTreeSet::new();
    let breaking_commit = Regex::new(r"^[a-z]+(\([^)]*\))?!:").unwrap();
    for line in technical.lines() {
        let trimmed = line.trim().trim_start_matches("- ").trim();
        if trimmed.to_ascii_lowercase().contains("breaking change")
            || breaking_commit.is_match(trimmed)
        {
            changes.insert(trimmed.to_string());
        }
    }
    if changes.is_empty() {
        String::new()
    } else {
        let mut rendered = String::from("Breaking changes:\n");
        for change in changes {
            rendered.push_str(&format!("- {change}\n"));
        }
        rendered
    }
}

pub(crate) fn request_synthesis(
    api_url: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
) -> Result<String> {
    let payload = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "You write concise user-facing release notes."},
            {"role": "user", "content": prompt}
        ]
    });
    let response = curl_json("POST", api_url, Some(api_key), Some(&payload))?;
    if !(200..300).contains(&response.status) {
        return Err(format!("HTTP {}", response.status).into());
    }
    let value: Value = serde_json::from_str(&response.body)?;
    let content = value["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("provider response did not include choices[0].message.content")?;
    Ok(content.to_string())
}

pub(crate) fn validate_notes(notes: &str) -> bool {
    notes
        .lines()
        .any(|line| line.trim_start().starts_with("## "))
        && notes
            .lines()
            .any(|line| line.trim_start().starts_with("- "))
}

pub(crate) fn notes_with_classification_notice(
    notes: &str,
    classification: &ReleaseClassification,
) -> String {
    if classification.disagreements.is_empty() {
        return notes.trim().to_string();
    }
    let signals = if classification.deterministic_signals.is_empty() {
        "none".to_string()
    } else {
        classification.deterministic_signals.join(", ")
    };
    format!(
        "{}\n\n<details>\n<summary>Release classification notes</summary>\n\nLandmark classification notice: deterministic release signals ({signals}) disagreed with model classification; synthesis proceeded. {}\n\n</details>",
        notes.trim(),
        classification.disagreements.join("; ")
    )
}

pub(crate) fn release_policy(args: ReleasePolicyArgs) -> Result<()> {
    match args.command {
        ReleasePolicyCommand::Publication(args) => publication_policy(args),
        ReleasePolicyCommand::Summary(args) => summary_policy(*args),
    }
}

pub(crate) fn publication_policy(args: PublicationArgs) -> Result<()> {
    let required = parse_bool(&args.synthesis_required) || parse_bool(&args.synthesis_strict);
    let succeeded = parse_bool(&args.synth_succeeded);
    let quality = normalize_quality(&args.synth_quality);
    let mut can_update_release = succeeded;
    let mut can_publish_artifacts = succeeded;
    let mut failure_stage = args.synth_failure_stage.clone();
    let mut failure_message = args.synth_failure_message.clone();
    let mut exit_failure = false;
    let policy_succeeded = if quality == "skipped" || failure_stage == "skipped" {
        can_update_release = false;
        can_publish_artifacts = false;
        true
    } else {
        succeeded
    };
    if quality == "ungrounded" {
        // Fabricated sections are never publishable, independent of the
        // synthesis-required input. That flag governs whether an operator
        // wants failed/degraded synthesis to block the pipeline; it was never
        // meant to make shipping invented release notes optional.
        can_update_release = false;
        can_publish_artifacts = false;
        failure_stage = "grounding".to_string();
        failure_message =
            "Synthesis fabricated release-note sections with no grounded source evidence."
                .to_string();
        exit_failure = true;
    } else if policy_succeeded && quality == "degraded" && required {
        can_update_release = false;
        can_publish_artifacts = false;
        failure_stage = "validation".to_string();
        failure_message = "Synthesis quality is degraded and synthesis is required.".to_string();
        exit_failure = true;
    } else if !policy_succeeded && required {
        can_update_release = false;
        can_publish_artifacts = false;
        if failure_stage.is_empty() {
            failure_stage = "synthesis".to_string();
        }
        if failure_message.is_empty() {
            failure_message = "Synthesis failed and synthesis is required.".to_string();
        }
        exit_failure = true;
    }
    write_outputs(
        &args.github_output,
        &[
            ("succeeded", policy_succeeded.to_string()),
            ("quality", quality),
            ("can_update_release", can_update_release.to_string()),
            ("can_publish_artifacts", can_publish_artifacts.to_string()),
            ("failure_stage", sanitize_text(&failure_stage)),
            ("failure_message", sanitize_text(&failure_message)),
        ],
    )?;
    if exit_failure {
        Err("synthesis publication policy failed".into())
    } else {
        Ok(())
    }
}

pub(crate) fn summary_policy(args: SummaryArgs) -> Result<()> {
    let synthesis_enabled = parse_bool(&args.synthesis_enabled);
    let released = parse_bool(&args.released);
    let synth_succeeded = parse_bool(&args.synth_succeeded);
    let update_succeeded = parse_bool(&args.update_succeeded);
    let artifact_succeeded = parse_bool(&args.artifact_succeeded);
    let rss_enabled = parse_bool(&args.rss_enabled);
    let rss_succeeded = parse_bool(&args.rss_succeeded);
    let webhook_enabled = parse_bool(&args.webhook_enabled);
    let webhook_sent = parse_bool(&args.webhook_sent);
    let slack_enabled = parse_bool(&args.slack_enabled);
    let slack_sent = parse_bool(&args.slack_sent);
    let quality = normalize_quality(&args.synth_quality);
    let synthesis_skipped = quality == "skipped" || args.synth_failure_stage == "skipped";
    let (succeeded, failure_stage, failure_message) = if !synthesis_enabled || !released {
        (true, "", "")
    } else if synthesis_skipped {
        (
            true,
            args.synth_failure_stage.as_str(),
            args.synth_failure_message.as_str(),
        )
    } else if !synth_succeeded {
        (
            false,
            args.synth_failure_stage.as_str(),
            args.synth_failure_message.as_str(),
        )
    } else if !update_succeeded {
        (
            false,
            args.update_failure_stage.as_str(),
            args.update_failure_message.as_str(),
        )
    } else if !artifact_succeeded {
        (
            false,
            args.artifact_failure_stage.as_str(),
            args.artifact_failure_message.as_str(),
        )
    } else if rss_enabled && !rss_succeeded {
        (
            false,
            args.rss_failure_stage.as_str(),
            args.rss_failure_message.as_str(),
        )
    } else {
        (true, "", "")
    };
    let mut destinations = BTreeMap::new();
    destinations.insert(
        "release_body".to_string(),
        DestinationStatus {
            enabled: synthesis_enabled && released && synth_succeeded && !synthesis_skipped,
            succeeded: update_succeeded,
            failure_stage: sanitize_text(&args.update_failure_stage),
            failure_message: sanitize_text(&args.update_failure_message),
        },
    );
    destinations.insert(
        "artifacts".to_string(),
        DestinationStatus {
            enabled: synthesis_enabled
                && released
                && synth_succeeded
                && update_succeeded
                && !synthesis_skipped,
            succeeded: artifact_succeeded,
            failure_stage: sanitize_text(&args.artifact_failure_stage),
            failure_message: sanitize_text(&args.artifact_failure_message),
        },
    );
    destinations.insert(
        "rss".to_string(),
        DestinationStatus {
            enabled: rss_enabled,
            succeeded: rss_succeeded,
            failure_stage: sanitize_text(&args.rss_failure_stage),
            failure_message: sanitize_text(&args.rss_failure_message),
        },
    );
    destinations.insert(
        "webhook".to_string(),
        DestinationStatus {
            enabled: webhook_enabled,
            succeeded: webhook_sent,
            failure_stage: String::new(),
            failure_message: String::new(),
        },
    );
    destinations.insert(
        "slack".to_string(),
        DestinationStatus {
            enabled: slack_enabled,
            succeeded: slack_sent,
            failure_stage: String::new(),
            failure_message: String::new(),
        },
    );
    let status = SynthesisStatus {
        synthesis_enabled,
        released,
        succeeded,
        quality: quality.clone(),
        failure_stage: sanitize_text(failure_stage),
        failure_message: sanitize_text(failure_message),
        model_attempts: read_json_array_if_requested(Path::new(&args.attempts_file))?,
        context: read_json_value_if_requested(Path::new(&args.context_metadata_file))?,
        destinations,
    };
    write_outputs(
        &args.github_output,
        &[
            ("succeeded", succeeded.to_string()),
            ("quality", quality),
            ("failure_stage", sanitize_text(failure_stage)),
            ("failure_message", sanitize_text(failure_message)),
            ("status_json", serde_json::to_string(&status)?),
        ],
    )
}

pub(crate) fn normalize_quality(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "valid" => "valid".to_string(),
        "degraded" => "degraded".to_string(),
        "ungrounded" => "ungrounded".to_string(),
        "skipped" => "skipped".to_string(),
        "failed" => "failed".to_string(),
        _ => "failed".to_string(),
    }
}
