use crate::*;
pub(crate) fn backfill(args: BackfillArgs) -> Result<()> {
    let mode = args.mode.trim();
    if mode != "artifacts-only" && mode != "release-body" {
        return Err("backfill --mode must be artifacts-only or release-body".into());
    }
    if args.rss_max_entries == 0 {
        return Err("--rss-max-entries must be positive".into());
    }
    let repository = if args.repository.trim().is_empty() {
        env::var("GITHUB_REPOSITORY").unwrap_or_default()
    } else {
        args.repository.trim().to_string()
    };
    if !repository.is_empty() {
        validate_repo(&repository)?;
    }
    if mode == "release-body" && !args.dry_run && !args.confirm_release_body {
        return Err(
            "backfill --mode release-body requires --dry-run or --confirm-release-body".into(),
        );
    }

    let all_tags = backfill_tags(&args.repo_root)?;
    let since = args.since.trim().to_string();
    let since_index = if since.is_empty() {
        None
    } else {
        all_tags.iter().position(|tag| tag.tag == since)
    };
    let mut skipped_tags = Vec::new();
    if !since.is_empty() && since_index.is_none() {
        skipped_tags.push(BackfillSkipRecord {
            tag: since.clone(),
            reason: "since tag not found".into(),
        });
    }

    let candidate_tags: Vec<_> = if since.is_empty() {
        all_tags.clone()
    } else if let Some(index) = since_index {
        all_tags.iter().skip(index + 1).cloned().collect()
    } else {
        Vec::new()
    };
    let mut selected_tags = Vec::new();
    let mut remaining_tags = Vec::new();
    for tag in candidate_tags {
        if args.max_tags > 0 && selected_tags.len() >= args.max_tags {
            remaining_tags.push(tag.tag);
        } else {
            selected_tags.push(tag);
        }
    }

    let mut processed_tags = Vec::new();
    let mut artifacts = Vec::new();
    let mut release_body_updates = Vec::new();
    let mut feed_items = if mode == "artifacts-only" && !args.dry_run {
        parse_existing_feed_items(
            &fs::read_to_string(args.repo_root.join(&args.rss_feed_file)).unwrap_or_default(),
        )
    } else {
        Vec::new()
    };
    let mut total_prompt_tokens = 0usize;
    let token = trimmed_option(&args.github_token);

    for tag in selected_tags {
        if tag.prerelease {
            skipped_tags.push(BackfillSkipRecord {
                tag: tag.tag,
                reason: "prerelease tags are skipped by default".into(),
            });
            continue;
        }
        let release =
            backfill_release_lookup(&args.api_base_url, &repository, &tag.tag, token.as_deref())?;
        if release.body.contains("## What's New") {
            skipped_tags.push(BackfillSkipRecord {
                tag: tag.tag,
                reason: "release body already contains Landmark notes".into(),
            });
            continue;
        }
        let source = backfill_source(&args.repo_root, &tag, release.body.as_str(), &all_tags)?;
        if mode == "release-body" && source.source == "github_release" {
            skipped_tags.push(BackfillSkipRecord {
                tag: tag.tag,
                reason: "existing GitHub Release body is the source; refusing to duplicate it in release-body mode".into(),
            });
            continue;
        }
        if source.duplicate_changelog {
            skipped_tags.push(BackfillSkipRecord {
                tag: tag.tag,
                reason: "duplicate changelog sections make release mapping ambiguous".into(),
            });
            continue;
        }
        let prompt_tokens = estimate_prompt_tokens(&source.notes);
        total_prompt_tokens += prompt_tokens;
        let record = BackfillTagRecord {
            tag: tag.tag.clone(),
            version: tag.version.clone(),
            package: tag.package.clone(),
            source: source.source.clone(),
            release_status: release.status.clone(),
            notes_sha256: sha256_hex(source.notes.as_bytes()),
            estimated_prompt_tokens: prompt_tokens,
        };

        if mode == "artifacts-only" {
            if args.dry_run {
                artifacts.push(backfill_plan_artifacts(&args, &tag));
            } else {
                artifacts.push(backfill_write_artifacts(
                    &args,
                    &repository,
                    &tag,
                    &source.notes,
                    &mut feed_items,
                )?);
            }
        } else if let Some(id) = release.id {
            let updated_body = compose_release_body(&source.notes, &release.body);
            let preview_sha256 = sha256_hex(updated_body.as_bytes());
            if !args.dry_run {
                backfill_update_release_body(&args, &repository, id, &updated_body)?;
            }
            release_body_updates.push(BackfillReleaseBodyUpdate {
                tag: tag.tag.clone(),
                release_id: id,
                dry_run: args.dry_run,
                updated: !args.dry_run,
                preview_sha256,
            });
        } else {
            skipped_tags.push(BackfillSkipRecord {
                tag: tag.tag.clone(),
                reason: format!(
                    "release-body mode requires an existing GitHub Release ({})",
                    release.status
                ),
            });
            continue;
        }

        processed_tags.push(record);
    }

    if mode == "artifacts-only" && !args.dry_run {
        backfill_write_feed(&args, &repository, feed_items)?;
    }

    let manifest = BackfillManifest {
        generated_at: Utc::now().to_rfc3339(),
        mode: mode.into(),
        dry_run: args.dry_run,
        repo_root: args.repo_root.display().to_string(),
        repository,
        since,
        processed_tags,
        skipped_tags,
        remaining_tags,
        estimated_cost: BackfillCostEstimate {
            llm_calls: 0,
            estimated_prompt_tokens: total_prompt_tokens,
            estimated_usd: 0.0,
            policy:
                "artifact backfill does not call the LLM; use the manifest to batch later synthesis"
                    .into(),
        },
        artifacts,
        release_body_updates,
    };
    println!("{}", serde_json::to_string_pretty(&manifest)?);
    if !args.dry_run {
        let resume_path = args.repo_root.join(&args.resume_file);
        ensure_parent(&resume_path)?;
        fs::write(resume_path, serde_json::to_string_pretty(&manifest)? + "\n")?;
    }
    Ok(())
}

pub(crate) fn backfill_tags(repo_root: &Path) -> Result<Vec<BackfillTag>> {
    let mut tags = git_tags(repo_root)?
        .into_iter()
        .filter_map(|tag| backfill_parse_tag(&tag))
        .collect::<Vec<_>>();
    tags.sort_by(|left, right| {
        left.key
            .cmp(&right.key)
            .then_with(|| left.package.cmp(&right.package))
            .then_with(|| left.tag.cmp(&right.tag))
    });
    Ok(tags)
}

pub(crate) fn backfill_parse_tag(tag: &str) -> Option<BackfillTag> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"^(?:(?P<package>[A-Za-z0-9_.-]+)@)?v?(?P<major>[0-9]+)\.(?P<minor>[0-9]+)\.(?P<patch>[0-9]+)(?P<pre>-[A-Za-z0-9][A-Za-z0-9.-]*)?$",
        )
        .unwrap()
    });
    let caps = re.captures(tag.trim())?;
    let major = caps.name("major")?.as_str().parse().ok()?;
    let minor = caps.name("minor")?.as_str().parse().ok()?;
    let patch = caps.name("patch")?.as_str().parse().ok()?;
    let package = caps
        .name("package")
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();
    Some(BackfillTag {
        tag: tag.trim().to_string(),
        version: format!("{major}.{minor}.{patch}"),
        key: (major, minor, patch),
        package,
        prerelease: caps.name("pre").is_some(),
    })
}

pub(crate) fn backfill_release_lookup(
    api_base_url: &str,
    repository: &str,
    tag: &str,
    github_token: Option<&str>,
) -> Result<BackfillReleaseLookup> {
    if repository.is_empty() {
        return Ok(BackfillReleaseLookup {
            status: "unavailable: repository not configured".into(),
            id: None,
            body: String::new(),
        });
    }
    if github_token.is_none() {
        return Ok(BackfillReleaseLookup {
            status: "unavailable: github token not configured".into(),
            id: None,
            body: String::new(),
        });
    }
    let provider = GitHubProvider::new(api_base_url, github_token);
    match provider.release_by_tag(repository, tag) {
        Ok(Some(value)) => Ok(BackfillReleaseLookup {
            status: "found".into(),
            id: value["id"].as_i64(),
            body: value["body"].as_str().unwrap_or("").to_string(),
        }),
        Ok(None) => Ok(BackfillReleaseLookup {
            status: "missing".into(),
            id: None,
            body: String::new(),
        }),
        Err(error) => Ok(BackfillReleaseLookup {
            status: format!("unavailable: {error}"),
            id: None,
            body: String::new(),
        }),
    }
}

pub(crate) fn backfill_source(
    repo_root: &Path,
    tag: &BackfillTag,
    release_body: &str,
    all_tags: &[BackfillTag],
) -> Result<BackfillSource> {
    if !release_body.trim().is_empty() {
        return Ok(BackfillSource {
            source: "github_release".into(),
            notes: release_body.trim().to_string(),
            duplicate_changelog: false,
        });
    }
    let changelog_path = repo_root.join("CHANGELOG.md");
    let changelog = changelog_sections(&changelog_path, &tag.version)?;
    if !changelog.sections.is_empty() {
        return Ok(BackfillSource {
            source: "changelog".into(),
            notes: changelog.sections[0].clone(),
            duplicate_changelog: changelog.duplicate,
        });
    }
    let git_notes = backfill_git_range_notes(repo_root, tag, all_tags)?;
    if !git_notes.trim().is_empty() {
        return Ok(BackfillSource {
            source: "git_range".into(),
            notes: git_notes,
            duplicate_changelog: false,
        });
    }
    let manifest = load_manifest(repo_root)?.unwrap_or_else(|| infer_manifest(repo_root));
    let product_name = manifest.product.name.as_deref().unwrap_or("the repository");
    Ok(BackfillSource {
        source: "manifest_context".into(),
        notes: format!(
            "## Historical Release {}\n\n- Historical notes were unavailable in GitHub Releases, CHANGELOG.md, and the tag range.\n- Product context: {}.\n",
            tag.tag, product_name
        ),
        duplicate_changelog: false,
    })
}

pub(crate) struct ChangelogSections {
    pub(crate) sections: Vec<String>,
    pub(crate) duplicate: bool,
}

pub(crate) fn changelog_sections(path: &Path, version: &str) -> Result<ChangelogSections> {
    if !path.is_file() {
        return Ok(ChangelogSections {
            sections: Vec::new(),
            duplicate: false,
        });
    }
    let text = fs::read_to_string(path)?;
    let marker = format!("[{version}]");
    let bare_marker = format!(" {version}");
    let mut sections = Vec::new();
    let mut current = Vec::new();
    let mut started = false;
    for line in text.lines() {
        let heading = line.starts_with('#');
        if heading
            && (line.contains(&marker)
                || line.trim_end() == format!("## {version}")
                || line.contains(&bare_marker))
        {
            if started && !current.is_empty() {
                sections.push(current.join("\n").trim().to_string());
                current.clear();
            }
            started = true;
            current.push(line.to_string());
            continue;
        }
        if started && heading && (line.starts_with("# ") || line.starts_with("## ")) {
            sections.push(current.join("\n").trim().to_string());
            current.clear();
            started = false;
            continue;
        }
        if started {
            current.push(line.to_string());
        }
    }
    if started && !current.is_empty() {
        sections.push(current.join("\n").trim().to_string());
    }
    sections.retain(|section| !section.trim().is_empty());
    Ok(ChangelogSections {
        duplicate: sections.len() > 1,
        sections,
    })
}

pub(crate) fn backfill_git_range_notes(
    repo_root: &Path,
    tag: &BackfillTag,
    all_tags: &[BackfillTag],
) -> Result<String> {
    let previous = previous_backfill_tag(all_tags, tag);
    let range = previous
        .map(|prev| format!("{prev}..{}", tag.tag))
        .unwrap_or_else(|| tag.tag.clone());
    let log = run_ok(
        "git",
        ["log", "--reverse", "--format=%s (%h)", range.as_str()],
        repo_root,
    )?;
    if log.trim().is_empty() {
        return Ok(String::new());
    }
    let mut notes = format!("## Historical Release {}\n\n", tag.tag);
    for line in log.lines().filter(|line| !line.trim().is_empty()) {
        notes.push_str("- ");
        notes.push_str(line.trim());
        notes.push('\n');
    }
    Ok(notes)
}

pub(crate) fn previous_backfill_tag(
    all_tags: &[BackfillTag],
    current: &BackfillTag,
) -> Option<String> {
    let mut previous = None;
    for tag in all_tags {
        if tag.package == current.package && tag.key < current.key && !tag.prerelease {
            previous = Some(tag.tag.clone());
        }
    }
    previous
}

pub(crate) fn backfill_write_artifacts(
    args: &BackfillArgs,
    repository: &str,
    tag: &BackfillTag,
    notes: &str,
    feed_items: &mut Vec<FeedItem>,
) -> Result<BackfillArtifactRecord> {
    let artifact = ReleaseNoteArtifact::from_markdown(&tag.tag, notes);
    let manifest =
        load_manifest(&args.repo_root)?.unwrap_or_else(|| infer_manifest(&args.repo_root));
    let audience = manifest
        .audience
        .as_deref()
        .and_then(trimmed_option)
        .unwrap_or_else(|| "general".into());
    let release_url = if repository.is_empty() {
        String::new()
    } else {
        format!(
            "{}/{repository}/releases/tag/{}",
            github_server_url_from_api(&args.api_base_url),
            tag.tag
        )
    };
    let entry_context = ReleaseNoteEntryContext::new(repository, &release_url, &audience);
    let markdown = backfill_write_template_if_requested(
        &args.repo_root,
        &args.output_file,
        &tag.tag,
        &artifact.notes,
    )?;
    let plaintext = backfill_write_template_if_requested(
        &args.repo_root,
        &args.output_text_file,
        &tag.tag,
        &artifact.plaintext,
    )?;
    let html = backfill_write_template_if_requested(
        &args.repo_root,
        &args.output_html_file,
        &tag.tag,
        &artifact.html,
    )?;
    let json_path = if args.output_json.trim().is_empty() {
        PathBuf::new()
    } else {
        backfill_append_json(
            &args.repo_root,
            &args.output_json,
            &artifact,
            &entry_context,
        )?
    };
    if !args.rss_feed_file.trim().is_empty() {
        feed_items.retain(|item| item.guid != tag.tag);
        feed_items.insert(
            0,
            FeedItem {
                title: if repository.is_empty() {
                    tag.tag.clone()
                } else {
                    format!("{repository} {}", tag.tag)
                },
                link: release_url,
                guid: tag.tag.clone(),
                description: artifact.html,
                pub_date: Utc::now().to_rfc2822(),
            },
        );
        feed_items.truncate(args.rss_max_entries);
    }
    Ok(BackfillArtifactRecord {
        tag: tag.tag.clone(),
        markdown: markdown.display().to_string(),
        plaintext: plaintext.display().to_string(),
        html: html.display().to_string(),
        json: json_path.display().to_string(),
        rss: backfill_output_path(&args.repo_root, &args.rss_feed_file, &tag.tag)
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
    })
}

pub(crate) fn backfill_plan_artifacts(
    args: &BackfillArgs,
    tag: &BackfillTag,
) -> BackfillArtifactRecord {
    BackfillArtifactRecord {
        tag: tag.tag.clone(),
        markdown: backfill_output_path(&args.repo_root, &args.output_file, &tag.tag)
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        plaintext: backfill_output_path(&args.repo_root, &args.output_text_file, &tag.tag)
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        html: backfill_output_path(&args.repo_root, &args.output_html_file, &tag.tag)
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        json: backfill_output_path(&args.repo_root, &args.output_json, &tag.tag)
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        rss: backfill_output_path(&args.repo_root, &args.rss_feed_file, &tag.tag)
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
    }
}

pub(crate) fn backfill_write_template_if_requested(
    repo_root: &Path,
    template: &str,
    tag: &str,
    content: &str,
) -> Result<PathBuf> {
    let Some(path) = backfill_output_path(repo_root, template, tag) else {
        return Ok(PathBuf::new());
    };
    ensure_parent(&path)?;
    fs::write(&path, content)?;
    Ok(path)
}

pub(crate) fn backfill_output_path(repo_root: &Path, template: &str, tag: &str) -> Option<PathBuf> {
    trimmed_option(template).map(|value| repo_root.join(value.replace("{version}", tag)))
}

pub(crate) fn backfill_append_json(
    repo_root: &Path,
    template: &str,
    artifact: &ReleaseNoteArtifact,
    context: &ReleaseNoteEntryContext,
) -> Result<PathBuf> {
    let path = repo_root.join(template.replace("{version}", &artifact.tag));
    let mut entries = if path.is_file() {
        serde_json::from_str::<Vec<Value>>(&fs::read_to_string(&path)?)?
    } else {
        Vec::new()
    };
    entries.retain(|entry| {
        entry["tag"].as_str() != Some(&artifact.tag)
            && entry["version"].as_str() != Some(&artifact.version)
    });
    entries.insert(0, artifact.json_entry(context));
    ensure_parent(&path)?;
    fs::write(&path, serde_json::to_string_pretty(&entries)? + "\n")?;
    Ok(path)
}

pub(crate) fn backfill_write_feed(
    args: &BackfillArgs,
    repository: &str,
    items: Vec<FeedItem>,
) -> Result<()> {
    if args.rss_feed_file.trim().is_empty() {
        return Ok(());
    }
    let path = args.repo_root.join(&args.rss_feed_file);
    let channel_link = if repository.is_empty() {
        default_release_url_base(repository)
    } else {
        format!(
            "{}/{}",
            github_server_url_from_api(&args.api_base_url),
            repository
        )
    };
    ensure_parent(&path)?;
    fs::write(path, render_feed(repository, &channel_link, &items))?;
    Ok(())
}

pub(crate) fn backfill_update_release_body(
    args: &BackfillArgs,
    repository: &str,
    release_id: i64,
    body: &str,
) -> Result<()> {
    let url = format!(
        "{}/repos/{}/releases/{}",
        args.api_base_url.trim_end_matches('/'),
        repository,
        release_id
    );
    let response = curl_json(
        "PATCH",
        &url,
        trimmed_option(&args.github_token).as_deref(),
        Some(&json!({ "body": body })),
    )?;
    if (200..300).contains(&response.status) {
        Ok(())
    } else {
        Err(format!(
            "GitHub release backfill update failed with HTTP {}",
            response.status
        )
        .into())
    }
}

pub(crate) fn github_server_url_from_api(api_base_url: &str) -> String {
    let trimmed = api_base_url.trim().trim_end_matches('/');
    if trimmed == "https://api.github.com" || trimmed == "http://api.github.com" {
        return trimmed.replace("api.github.com", "github.com");
    }
    if let Some(server) = trimmed.strip_suffix("/api/v3") {
        return server.to_string();
    }
    trimmed.to_string()
}

pub(crate) fn estimate_prompt_tokens(text: &str) -> usize {
    text.split_whitespace().count().max(1) * 4 / 3 + 64
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
