use crate::*;
#[derive(Clone, Serialize)]
pub(crate) struct ReleaseNoteArtifact {
    pub(crate) version: String,
    pub(crate) tag: String,
    pub(crate) notes: String,
    pub(crate) plaintext: String,
    pub(crate) html: String,
    pub(crate) slack: String,
    pub(crate) sections: Vec<NoteSection>,
    published_at: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ReleaseNoteEntryContext {
    pub(crate) repository: String,
    pub(crate) release_url: String,
    pub(crate) audience: String,
}

#[derive(Clone, Serialize)]
pub(crate) struct NoteSection {
    pub(crate) title: String,
    pub(crate) bullets: Vec<NoteBullet>,
}

#[derive(Clone, Serialize)]
pub(crate) struct NoteBullet {
    pub(crate) text: String,
    pub(crate) links: Vec<NoteLink>,
}

#[derive(Clone, Serialize)]
pub(crate) struct NoteLink {
    pub(crate) label: String,
    pub(crate) href: String,
}

impl ReleaseNoteArtifact {
    pub(crate) fn from_markdown(version: &str, notes: &str) -> Self {
        let trimmed = notes.trim().to_string();
        Self {
            version: version.trim_start_matches('v').to_string(),
            tag: version.to_string(),
            plaintext: markdown_to_plaintext(&trimmed),
            html: markdown_to_html_fragment(&trimmed),
            slack: markdown_to_slack(&trimmed),
            sections: parse_note_sections(&trimmed),
            published_at: Utc::now().to_rfc3339(),
            notes: trimmed,
        }
    }

    pub(crate) fn json_entry(&self, context: &ReleaseNoteEntryContext) -> Value {
        json!({
            "schema_version": "landmark.public-release-notes.v1",
            "version": self.version,
            "tag": self.tag,
            "repository": context.repository,
            "release_url": context.release_url,
            "audience": context.audience,
            "notes": self.notes,
            "markdown": self.notes,
            "html": self.html,
            "plaintext": self.plaintext,
            "slack": self.slack,
            "sections": self.sections,
            "published_at": self.published_at,
        })
    }

    pub(crate) fn webhook_payload(&self, repository: &str, release_url: &str) -> Value {
        json!({
            "version": self.tag,
            "repository": repository,
            "release_url": release_url,
            "notes": self.notes,
            "markdown": self.notes,
            "html": self.html,
            "plaintext": self.plaintext,
            "sections": self.sections,
            "published_at": self.published_at,
        })
    }

    pub(crate) fn slack_payload(&self, repository: &str, release_url: &str) -> Value {
        json!({
            "blocks": [
                {"type": "header", "text": {"type": "plain_text", "text": format!("{} {}", repository, self.tag)}},
                {"type": "section", "text": {"type": "mrkdwn", "text": self.slack}},
                {"type": "context", "elements": [{"type": "mrkdwn", "text": format!("<{}|View release>", release_url)}]}
            ]
        })
    }
}

impl ReleaseNoteEntryContext {
    pub(crate) fn new(repository: &str, release_url: &str, audience: &str) -> Self {
        Self {
            repository: repository.trim().to_string(),
            release_url: release_url.trim().to_string(),
            audience: trimmed_option(audience).unwrap_or_else(|| "general".into()),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct SynthesisStatus {
    pub(crate) synthesis_enabled: bool,
    pub(crate) released: bool,
    pub(crate) succeeded: bool,
    pub(crate) quality: String,
    pub(crate) failure_stage: String,
    pub(crate) failure_message: String,
    pub(crate) model_attempts: Vec<Value>,
    pub(crate) context: Value,
    pub(crate) destinations: BTreeMap<String, DestinationStatus>,
}

#[derive(Serialize)]
pub(crate) struct DestinationStatus {
    pub(crate) enabled: bool,
    pub(crate) succeeded: bool,
    pub(crate) failure_stage: String,
    pub(crate) failure_message: String,
}

pub(crate) fn write_artifacts(args: WriteArtifactsArgs) -> Result<()> {
    let notes = read_nonempty(&args.notes_file)?;
    let artifact = ReleaseNoteArtifact::from_markdown(&args.version, &notes);
    let repository = trimmed_option(&args.repository)
        .or_else(|| {
            env::var("GITHUB_REPOSITORY")
                .ok()
                .and_then(|value| trimmed_option(&value))
        })
        .unwrap_or_default();
    let release_url = trimmed_option(&args.release_url).unwrap_or_else(|| {
        if repository.is_empty() {
            String::new()
        } else {
            release_link(
                &default_release_url_base(&repository),
                &repository,
                &args.version,
            )
        }
    });
    let context = ReleaseNoteEntryContext::new(&repository, &release_url, &args.audience);
    if !args.output_file.trim().is_empty() {
        write_notes_file(&artifact.notes, &args.output_file, &args.version)?;
    }
    if !args.output_text_file.trim().is_empty() {
        write_notes_file(&artifact.plaintext, &args.output_text_file, &args.version)?;
    }
    if !args.output_html_file.trim().is_empty() {
        write_notes_file(&artifact.html, &args.output_html_file, &args.version)?;
    }
    if !args.output_json.trim().is_empty() {
        append_json_entry(&args.output_json, &artifact, &context)?;
    }
    print!("{}", artifact.notes);
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BackfillManifest {
    pub(crate) generated_at: String,
    pub(crate) mode: String,
    pub(crate) dry_run: bool,
    pub(crate) repo_root: String,
    pub(crate) repository: String,
    pub(crate) since: String,
    pub(crate) processed_tags: Vec<BackfillTagRecord>,
    pub(crate) skipped_tags: Vec<BackfillSkipRecord>,
    pub(crate) remaining_tags: Vec<String>,
    pub(crate) estimated_cost: BackfillCostEstimate,
    pub(crate) artifacts: Vec<BackfillArtifactRecord>,
    pub(crate) release_body_updates: Vec<BackfillReleaseBodyUpdate>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BackfillTagRecord {
    pub(crate) tag: String,
    pub(crate) version: String,
    pub(crate) package: String,
    pub(crate) source: String,
    pub(crate) release_status: String,
    pub(crate) notes_sha256: String,
    pub(crate) estimated_prompt_tokens: usize,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BackfillSkipRecord {
    pub(crate) tag: String,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BackfillCostEstimate {
    pub(crate) llm_calls: usize,
    pub(crate) estimated_prompt_tokens: usize,
    pub(crate) estimated_usd: f64,
    pub(crate) policy: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BackfillArtifactRecord {
    pub(crate) tag: String,
    pub(crate) markdown: String,
    pub(crate) plaintext: String,
    pub(crate) html: String,
    pub(crate) json: String,
    pub(crate) rss: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BackfillReleaseBodyUpdate {
    pub(crate) tag: String,
    pub(crate) release_id: i64,
    pub(crate) dry_run: bool,
    pub(crate) updated: bool,
    pub(crate) preview_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RunEvidence {
    pub(crate) provider: String,
    pub(crate) generated_at: String,
    pub(crate) repo_root: String,
    pub(crate) repository: String,
    pub(crate) release_tag: String,
    pub(crate) version: String,
    pub(crate) previous_tag: String,
    pub(crate) source: String,
    pub(crate) technical_changelog_sha256: String,
    pub(crate) notes_sha256: String,
    pub(crate) version_decision: RunVersionDecision,
    pub(crate) changed_files: Vec<String>,
    pub(crate) artifacts: RunArtifactRecord,
    pub(crate) release_kit: release_kit::ReleaseKit,
    pub(crate) publication: RunPublicationRecord,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RunVersionDecision {
    pub(crate) latest_tag: String,
    pub(crate) bump: String,
    /// The bump before pre-stable demotion. Equal to `bump` on a stable (>=1.0.0)
    /// line; on a 0.x line it preserves the pre-stability decision so the
    /// demotion is visible, not silent. See card landmark-016.
    pub(crate) raw_bump: String,
    /// `pre-stable` when the current version is below 1.0.0 (Cargo-style 0.x
    /// rules applied), otherwise `stable`.
    pub(crate) stability: String,
    pub(crate) commit_bump: String,
    pub(crate) api_evidence_bump: String,
    pub(crate) reconciliation: String,
    pub(crate) commit_count: usize,
    pub(crate) conventional_commit_count: usize,
    pub(crate) range: String,
    pub(crate) decisive_commit: Option<String>,
    pub(crate) decisive_signals: Vec<String>,
    pub(crate) unknown_commits: Vec<String>,
    pub(crate) api_evidence: VersionApiEvidence,
    pub(crate) waiver: VersionDecisionWaiver,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RunArtifactRecord {
    pub(crate) technical_changelog: String,
    pub(crate) technical_changelog_audience: String,
    pub(crate) technical_changelog_schema: String,
    pub(crate) markdown: String,
    pub(crate) public_notes_audience: String,
    pub(crate) public_notes_schema: String,
    pub(crate) plaintext: String,
    pub(crate) html: String,
    pub(crate) json: String,
    pub(crate) rss: String,
    pub(crate) evidence: String,
    pub(crate) release_kit: String,
    pub(crate) release_kit_schema: String,
    pub(crate) release_kit_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RunPublicationRecord {
    pub(crate) provider: String,
    pub(crate) enabled: bool,
    pub(crate) release_body_updated: bool,
    pub(crate) release_url: String,
    pub(crate) status: String,
}

#[derive(Clone, Debug)]
pub(crate) struct RunReleaseContext {
    pub(crate) release_tag: String,
    pub(crate) previous_tag: String,
    pub(crate) version: String,
    pub(crate) decision: RunVersionDecision,
    pub(crate) commits: Vec<RunCommit>,
}

#[derive(Clone, Debug)]
pub(crate) struct RunCommit {
    pub(crate) subject: String,
    pub(crate) short_hash: String,
    pub(crate) body: String,
}

#[derive(Clone, Debug)]
pub(crate) struct BackfillTag {
    pub(crate) tag: String,
    pub(crate) version: String,
    pub(crate) key: (u64, u64, u64),
    pub(crate) package: String,
    pub(crate) prerelease: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct BackfillReleaseLookup {
    pub(crate) status: String,
    pub(crate) id: Option<i64>,
    pub(crate) body: String,
}

#[derive(Clone, Debug)]
pub(crate) struct BackfillSource {
    pub(crate) source: String,
    pub(crate) notes: String,
    pub(crate) duplicate_changelog: bool,
}
