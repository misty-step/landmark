use crate::*;
pub(crate) fn run_pipeline(args: RunArgs) -> Result<()> {
    let provider = args.provider.trim().to_ascii_lowercase();
    if !matches!(provider.as_str(), "local" | "github") {
        return Err(format!(
            "unsupported provider '{provider}'; this build supports provider=local or provider=github"
        )
        .into());
    }
    if args.rss_max_entries == 0 {
        return Err("--rss-max-entries must be positive".into());
    }
    if !args.dry_run {
        fs::create_dir_all(args.repo_root.join(&args.output_dir))?;
    }
    let manifest =
        load_manifest(&args.repo_root)?.unwrap_or_else(|| infer_manifest(&args.repo_root));
    let repository = trimmed_option(&args.repository)
        .or_else(|| {
            args.repo_root
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "local".into());
    let release = resolve_local_release(&args)?;
    let technical_changelog = render_local_technical_changelog(&release);
    let notes = if let Some(notes_file) =
        run_output_path(&args.repo_root, &args.notes_file, &release.release_tag)
    {
        read_nonempty(&notes_file)?
    } else {
        render_local_public_notes(&manifest, &release)
    };
    let mut artifacts = write_run_artifacts(
        &args,
        &manifest,
        &repository,
        &release.release_tag,
        &release_url_base(&args, &repository),
        &technical_changelog,
        &notes,
    )?;
    let publication =
        publish_run_release_body(&args, &provider, &repository, &release.release_tag, &notes)?;
    artifacts.release_kit = release_kit::artifact_path(&args).display().to_string();
    artifacts.release_kit_schema = release_kit::schema_version().into();
    let generated_at = Utc::now().to_rfc3339();
    let release_kit = release_kit::plan(release_kit::PlanInput {
        args: &args,
        manifest: &manifest,
        repository: &repository,
        release: &release,
        artifacts: &artifacts,
        publication: &publication,
        technical_changelog: &technical_changelog,
        notes: &notes,
        generated_at: &generated_at,
    });
    let release_kit_json = serde_json::to_string_pretty(&release_kit)? + "\n";
    artifacts.release_kit_sha256 = sha256_hex(release_kit_json.as_bytes());
    if !args.dry_run && !artifacts.release_kit.trim().is_empty() {
        write_path(Path::new(&artifacts.release_kit), &release_kit_json)?;
    }
    let evidence = RunEvidence {
        provider,
        generated_at,
        repo_root: args.repo_root.display().to_string(),
        repository,
        release_tag: release.release_tag.clone(),
        version: release.version.clone(),
        previous_tag: release.previous_tag.clone(),
        source: "git_range".into(),
        technical_changelog_sha256: sha256_hex(technical_changelog.as_bytes()),
        notes_sha256: sha256_hex(notes.as_bytes()),
        version_decision: release.decision,
        changed_files: context_changed_files(&args.repo_root, &release.version),
        artifacts,
        release_kit,
        publication,
    };
    let evidence_json = serde_json::to_string_pretty(&evidence)? + "\n";
    let evidence_path = run_output_path(&args.repo_root, &args.evidence_file, &release.release_tag)
        .ok_or("--evidence-file must not be empty")?;
    if !args.dry_run {
        ensure_parent(&evidence_path)?;
        fs::write(&evidence_path, &evidence_json)?;
    }
    println!("{evidence_json}");
    Ok(())
}

pub(crate) fn resolve_local_release(args: &RunArgs) -> Result<RunReleaseContext> {
    let tags = backfill_tags(&args.repo_root)?;
    let latest_tag = tags.iter().rfind(|tag| !tag.prerelease).cloned();
    let explicit_release_tag = trimmed_option(&args.release_tag);
    let explicit_tag = explicit_release_tag.as_deref().and_then(backfill_parse_tag);
    let previous_tag = trimmed_option(&args.previous_tag)
        .or_else(|| {
            explicit_tag
                .as_ref()
                .and_then(|tag| previous_backfill_tag(&tags, tag))
        })
        .or_else(|| latest_tag.as_ref().map(|tag| tag.tag.clone()))
        .unwrap_or_default();
    let target_ref = explicit_release_tag
        .as_ref()
        .filter(|tag| tags.iter().any(|existing| existing.tag == **tag))
        .cloned()
        .unwrap_or_else(|| "HEAD".into());
    let commits = local_release_commits(&args.repo_root, previous_tag.as_str(), &target_ref)?;
    let classified: Vec<ClassifiedCommit> = commits
        .iter()
        .map(|commit| classify_commit(&commit.short_hash, &commit.subject, &commit.body))
        .collect();
    let api_evidence = CargoSemverChecksProvider.collect(&VersionApiEvidenceRequest {
        repo_root: &args.repo_root,
        previous_tag: &previous_tag,
        target_ref: &target_ref,
    });
    let decision = decide_version_with_api_evidence(&classified, api_evidence);
    let bump = decision.bump.map(VersionBump::as_str).unwrap_or("none");
    let release_tag =
        explicit_release_tag.unwrap_or_else(|| next_release_tag(latest_tag.as_ref(), bump));
    let version = release_tag.trim_start_matches('v').to_string();
    let range = if previous_tag.is_empty() {
        target_ref.clone()
    } else {
        format!("{previous_tag}..{target_ref}")
    };
    let conventional_commit_count = classified.len() - decision.unknown_commits.len();
    Ok(RunReleaseContext {
        release_tag,
        previous_tag,
        version,
        decision: RunVersionDecision {
            latest_tag: latest_tag.map(|tag| tag.tag).unwrap_or_default(),
            bump: bump.to_string(),
            commit_bump: decision.commit_bump,
            api_evidence_bump: decision.api_evidence_bump,
            reconciliation: decision.reconciliation,
            commit_count: commits.len(),
            conventional_commit_count,
            range,
            decisive_commit: decision.decisive.map(|commit| commit.evidence_line()),
            decisive_signals: decision.decisive_signals,
            unknown_commits: decision
                .unknown_commits
                .iter()
                .map(ClassifiedCommit::evidence_line)
                .collect(),
            api_evidence: decision.api_evidence,
            waiver: decision.waiver,
        },
        commits,
    })
}

pub(crate) fn local_release_commits(
    repo_root: &Path,
    previous_tag: &str,
    target_ref: &str,
) -> Result<Vec<RunCommit>> {
    let range = if previous_tag.trim().is_empty() {
        target_ref.to_string()
    } else {
        format!("{previous_tag}..{target_ref}")
    };
    let log = run_ok(
        "git",
        [
            "log",
            "--reverse",
            "--format=%x1e%s%x1f%h%x1f%b",
            range.as_str(),
        ],
        repo_root,
    )?;
    Ok(log
        .split('\x1e')
        .filter_map(|record| {
            let record = record.trim_matches('\n');
            if record.trim().is_empty() {
                return None;
            }
            let mut parts = record.splitn(3, '\x1f');
            Some(RunCommit {
                subject: parts.next().unwrap_or("").trim().to_string(),
                short_hash: parts.next().unwrap_or("").trim().to_string(),
                body: parts.next().unwrap_or("").trim().to_string(),
            })
        })
        .collect())
}

pub(crate) fn conventional_commit_type(subject: &str) -> Option<&str> {
    let subject = subject.trim();
    let header = subject.split(':').next()?;
    let header = header.strip_suffix('!').unwrap_or(header);
    let header = header.split('(').next().unwrap_or(header);
    if header
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch == '-')
    {
        Some(header)
    } else {
        None
    }
}

pub(crate) fn is_breaking_commit(commit: &RunCommit) -> bool {
    commit.subject.contains("!:")
        || commit.subject.contains(")!:")
        || commit.body.lines().any(|line| {
            let line = line.trim();
            line.starts_with("BREAKING CHANGE:") || line.starts_with("BREAKING-CHANGE:")
        })
}

pub(crate) fn next_release_tag(latest: Option<&BackfillTag>, bump: &str) -> String {
    let Some(latest) = latest else {
        return match bump {
            "major" => "v1.0.0".into(),
            "minor" => "v0.1.0".into(),
            "none" => "v0.0.0".into(),
            _ => "v0.0.1".into(),
        };
    };
    let (mut major, mut minor, mut patch) = latest.key;
    match bump {
        "major" => {
            major += 1;
            minor = 0;
            patch = 0;
        }
        "minor" => {
            minor += 1;
            patch = 0;
        }
        "none" => {}
        _ => {
            patch += 1;
        }
    }
    if latest.package.is_empty() {
        format!("v{major}.{minor}.{patch}")
    } else {
        format!("{}@v{major}.{minor}.{patch}", latest.package)
    }
}

pub(crate) fn render_local_technical_changelog(release: &RunReleaseContext) -> String {
    let mut markdown = format!("## Technical Changelog {}\n\n", release.release_tag);
    if release.commits.is_empty() {
        markdown.push_str("- No commits were found in the selected release range.\n");
    } else {
        for commit in &release.commits {
            markdown.push_str("- ");
            markdown.push_str(&commit.display_line());
            markdown.push('\n');
        }
    }
    markdown
}

pub(crate) fn render_local_public_notes(
    manifest: &LandmarkManifest,
    release: &RunReleaseContext,
) -> String {
    let product = manifest
        .product
        .name
        .as_deref()
        .and_then(trimmed_option)
        .unwrap_or_else(|| "This project".into());
    let mut markdown = format!("## Improvements in {}\n\n", release.release_tag);
    if release.commits.is_empty() {
        markdown.push_str(&format!(
            "- {product} has no user-visible commit entries in this release range.\n"
        ));
    } else {
        for commit in &release.commits {
            markdown.push_str("- ");
            markdown.push_str(&humanize_commit_subject(&commit.subject));
            markdown.push('\n');
        }
    }
    markdown
}

impl RunCommit {
    pub(crate) fn display_line(&self) -> String {
        if self.short_hash.is_empty() {
            self.subject.clone()
        } else {
            format!("{} ({})", self.subject, self.short_hash)
        }
    }
}

pub(crate) fn humanize_commit_subject(subject: &str) -> String {
    let text = subject
        .split_once(':')
        .map(|(_, rest)| rest)
        .unwrap_or(subject)
        .trim();
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => subject.to_string(),
    }
}

pub(crate) fn write_run_artifacts(
    args: &RunArgs,
    manifest: &LandmarkManifest,
    repository: &str,
    release_tag: &str,
    release_url_base: &str,
    technical_changelog: &str,
    notes: &str,
) -> Result<RunArtifactRecord> {
    let artifact = ReleaseNoteArtifact::from_markdown(release_tag, notes);
    let technical = run_template_path(&args.repo_root, &args.technical_changelog_file, release_tag);
    if !args.dry_run && !technical.as_os_str().is_empty() {
        write_path(&technical, technical_changelog)?;
    }
    let markdown = run_template_path(&args.repo_root, &args.output_file, release_tag);
    if !args.dry_run && !markdown.as_os_str().is_empty() {
        write_path(&markdown, &artifact.notes)?;
    }
    let plaintext = run_template_path(&args.repo_root, &args.output_text_file, release_tag);
    if !args.dry_run && !plaintext.as_os_str().is_empty() {
        write_path(&plaintext, &artifact.plaintext)?;
    }
    let html = run_template_path(&args.repo_root, &args.output_html_file, release_tag);
    if !args.dry_run && !html.as_os_str().is_empty() {
        write_path(&html, &artifact.html)?;
    }
    let json_path = if args.output_json.trim().is_empty() {
        PathBuf::new()
    } else if args.dry_run {
        run_template_path(&args.repo_root, &args.output_json, release_tag)
    } else {
        backfill_append_json(&args.repo_root, &args.output_json, &artifact)?
    };
    let rss_path = if args.rss_feed_file.trim().is_empty() {
        PathBuf::new()
    } else if args.dry_run {
        args.repo_root.join(&args.rss_feed_file)
    } else {
        let path = args.repo_root.join(&args.rss_feed_file);
        let existing = fs::read_to_string(&path).unwrap_or_default();
        let mut items = parse_existing_feed_items(&existing);
        items.retain(|item| item.guid != release_tag);
        items.insert(
            0,
            FeedItem {
                title: format!("{repository} {release_tag}"),
                link: release_link(release_url_base, repository, release_tag),
                guid: release_tag.to_string(),
                description: artifact.html,
                pub_date: Utc::now().to_rfc2822(),
            },
        );
        items.truncate(args.rss_max_entries);
        ensure_parent(&path)?;
        fs::write(&path, render_feed(repository, release_url_base, &items))?;
        path
    };
    let evidence =
        run_output_path(&args.repo_root, &args.evidence_file, release_tag).unwrap_or_default();
    Ok(RunArtifactRecord {
        technical_changelog: technical.display().to_string(),
        technical_changelog_audience: "internal-developer-operator".into(),
        technical_changelog_schema: "landmark.internal-technical-changelog.v1".into(),
        markdown: markdown.display().to_string(),
        public_notes_audience: manifest
            .audience
            .as_deref()
            .and_then(trimmed_option)
            .unwrap_or_else(|| "general".into()),
        public_notes_schema: "landmark.public-release-notes.v1".into(),
        plaintext: plaintext.display().to_string(),
        html: html.display().to_string(),
        json: json_path.display().to_string(),
        rss: rss_path.display().to_string(),
        evidence: evidence.display().to_string(),
        release_kit: String::new(),
        release_kit_schema: String::new(),
        release_kit_sha256: String::new(),
    })
}

pub(crate) fn release_url_base(args: &RunArgs, repository: &str) -> String {
    trimmed_option(&args.server_url)
        .map(|url| format!("{}/{}", url.trim_end_matches('/'), repository))
        .unwrap_or_else(|| default_release_url_base(repository))
}

pub(crate) fn release_link(base: &str, repository: &str, release_tag: &str) -> String {
    if repository.contains('/') {
        format!("{}/releases/tag/{release_tag}", base.trim_end_matches('/'))
    } else {
        format!("local://{repository}/releases/{release_tag}")
    }
}

pub(crate) fn publish_run_release_body(
    args: &RunArgs,
    provider: &str,
    repository: &str,
    release_tag: &str,
    notes: &str,
) -> Result<RunPublicationRecord> {
    if args.dry_run {
        return Ok(RunPublicationRecord {
            provider: provider.into(),
            enabled: args.publish_release_body,
            release_body_updated: false,
            release_url: release_link(&release_url_base(args, repository), repository, release_tag),
            status: "dry-run; release-body publication previewed but not mutated".into(),
        });
    }
    if provider == "local" {
        return Ok(RunPublicationRecord {
            provider: provider.into(),
            enabled: false,
            release_body_updated: false,
            release_url: format!("local://{repository}/releases/{release_tag}"),
            status: "local provider does not mutate remote releases".into(),
        });
    }
    let release_url = if repository.contains('/') {
        format!("https://github.com/{repository}/releases/tag/{release_tag}")
    } else {
        String::new()
    };
    if !args.publish_release_body {
        return Ok(RunPublicationRecord {
            provider: provider.into(),
            enabled: false,
            release_body_updated: false,
            release_url,
            status: "release-body publication skipped".into(),
        });
    }
    let token = trimmed_option(&args.github_token)
        .or_else(|| {
            env::var("GITHUB_TOKEN")
                .ok()
                .and_then(|value| trimmed_option(&value))
        })
        .or_else(|| {
            env::var("GH_TOKEN")
                .ok()
                .and_then(|value| trimmed_option(&value))
        })
        .ok_or("--publish-release-body requires --github-token, GITHUB_TOKEN, or GH_TOKEN")?;
    let gh_provider = GitHubProvider::required(&args.api_base_url, &token);
    let release_url = gh_provider.update_release_body(repository, release_tag, notes)?;
    Ok(RunPublicationRecord {
        provider: provider.into(),
        enabled: true,
        release_body_updated: true,
        release_url,
        status: "updated".into(),
    })
}

pub(crate) fn run_template_path(repo_root: &Path, template: &str, tag: &str) -> PathBuf {
    run_output_path(repo_root, template, tag).unwrap_or_default()
}

pub(crate) fn write_path(path: &Path, content: &str) -> Result<()> {
    ensure_parent(path)?;
    fs::write(path, content)?;
    Ok(())
}

pub(crate) fn run_output_path(repo_root: &Path, template: &str, tag: &str) -> Option<PathBuf> {
    trimmed_option(template).map(|value| repo_root.join(value.replace("{version}", tag)))
}
