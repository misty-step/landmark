use crate::*;
pub(crate) fn prepare_release_transaction(args: PrepareReleaseTransactionArgs) -> Result<()> {
    let run_args = RunArgs {
        provider: "local".into(),
        repo_root: args.repo_root.clone(),
        repository: args.repository.clone(),
        release_tag: args.release_tag.clone(),
        previous_tag: args.previous_tag.clone(),
        github_token: String::new(),
        api_base_url: "https://api.github.com".into(),
        server_url: String::new(),
        publish_release_body: false,
        dry_run: true,
        notes_file: args.notes_file.clone(),
        output_dir: PathBuf::new(),
        technical_changelog_file: String::new(),
        evidence_file: String::new(),
        output_file: String::new(),
        output_text_file: String::new(),
        output_html_file: String::new(),
        output_json: String::new(),
        rss_feed_file: String::new(),
        rss_max_entries: 1,
    };
    let release = resolve_local_release(&run_args)?;
    if release.decision.bump == "none" && args.release_tag.trim().is_empty() {
        return Err(
            "no release-worthy changes were found; refusing to prepare an existing tag".into(),
        );
    }
    let manifest =
        load_manifest(&args.repo_root)?.unwrap_or_else(|| infer_manifest(&args.repo_root));
    let notes = if let Some(path) =
        run_output_path(&args.repo_root, &args.notes_file, &release.release_tag)
    {
        read_nonempty(&path)?
    } else {
        render_local_public_notes(&manifest, &release)
    };
    let repository = trimmed_option(&args.repository)
        .or_else(|| {
            args.repo_root
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "local".into());
    if repository.contains('/') {
        validate_repo(&repository)?;
    } else {
        validate_nonblank(&repository, "repository")?;
    }
    let source_revision = run_ok("git", ["rev-parse", "HEAD"], &args.repo_root)?
        .trim()
        .to_ascii_lowercase();
    let candidate = ReleaseCandidate {
        repository,
        source_revision,
        previous_tag: release.previous_tag,
        version: release.version,
        release_tag: release.release_tag,
        notes_sha256: format!("sha256:{}", sha256_hex(notes.as_bytes())),
    };
    validate_candidate(&candidate)?;
    let transaction = ReleaseTransaction {
        schema_version: TRANSACTION_SCHEMA.into(),
        transaction_id: identity_digest(&candidate)?,
        state: "prepared".into(),
        prepared_at: Utc::now().to_rfc3339(),
        candidate,
        required_artifact_roles: REQUIRED_ROLES.iter().map(|role| (*role).into()).collect(),
        artifacts: Vec::new(),
        artifact_set_sha256: None,
        verification: None,
        bound_at: None,
        receipt: None,
    };
    let transaction_path = secure_state_path(&args.transaction, true)?;
    let _lock = lock_transaction(&transaction_path)?;
    if transaction_path.exists() {
        let existing = read_transaction(&transaction_path)?;
        if existing.transaction_id != transaction.transaction_id
            || existing.candidate != transaction.candidate
        {
            return Err("canonical transaction already contains a different candidate".into());
        }
        return emit_transaction(&existing);
    }
    write_transaction_cas(&transaction_path, None, &transaction, InjectedCrash::None)?;
    emit_transaction(&transaction)
}
