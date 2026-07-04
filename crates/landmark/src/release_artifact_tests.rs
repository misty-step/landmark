use super::*;

#[test]
fn markdown_filters_unsafe_links() {
    let html = markdown_to_html_fragment("[bad](javascript:alert(1)) [ok](https://example.com)");
    assert!(html.contains("href=\"#\""));
    assert!(html.contains("href=\"https://example.com\""));
}

#[test]
fn typed_artifact_renders_shared_outputs() {
    let artifact = ReleaseNoteArtifact::from_markdown(
        "v1.2.3",
        "## Added\n\n- See [docs](https://example.com) and [bad](javascript:alert(1))",
    );
    assert_eq!(artifact.version, "1.2.3");
    assert!(artifact.html.contains("href=\"https://example.com\""));
    assert!(artifact.html.contains("href=\"#\""));
    assert!(artifact.plaintext.contains("See docs and bad"));
    assert!(artifact.slack.contains("<https://example.com|docs>"));
    assert!(!artifact.slack.contains("javascript:"));
    assert_eq!(artifact.sections[0].title, "Added");
    assert_eq!(
        artifact.sections[0].bullets[0].links[0].href,
        "https://example.com"
    );
    let context = ReleaseNoteEntryContext::new(
        "misty-step/example",
        "https://github.com/misty-step/example/releases/tag/v1.2.3",
        "end-user",
    );
    let entry = artifact.json_entry(&context);
    assert_eq!(entry["schema_version"], "landmark.public-release-notes.v1");
    assert_eq!(entry["repository"], "misty-step/example");
    assert_eq!(entry["audience"], "end-user");
    assert!(entry["sections"].is_array());
}

#[test]
fn public_notes_skip_internal_process_commits() {
    let feature = RunCommit {
        subject: "feat(cli): add portable release run".into(),
        short_hash: "abc1234".into(),
        body: String::new(),
    };
    let internal = RunCommit {
        subject: "chore(ci): refresh agent lane".into(),
        short_hash: "def5678".into(),
        body: String::new(),
    };
    let scoped_process_feature = RunCommit {
        subject: "feat(ci): refresh agent lane".into(),
        short_hash: "fed9012".into(),
        body: String::new(),
    };
    let internal_revert = RunCommit {
        subject: "Revert \"chore(ci): refresh agent lane\"".into(),
        short_hash: "9999999".into(),
        body: String::new(),
    };
    let public_fix = RunCommit {
        subject: "fix(ui): keep release cards readable".into(),
        short_hash: "1111111".into(),
        body: String::new(),
    };

    assert_eq!(
        public_note_for_commit(&feature).as_deref(),
        Some("Add portable release run")
    );
    assert_eq!(public_note_for_commit(&internal), None);
    assert_eq!(public_note_for_commit(&scoped_process_feature), None);
    assert_eq!(public_note_for_commit(&internal_revert), None);
    assert_eq!(
        public_note_for_commit(&public_fix).as_deref(),
        Some("Keep release cards readable")
    );
}

#[test]
fn backfill_release_urls_follow_api_base_url() {
    assert_eq!(
        github_server_url_from_api("https://api.github.com"),
        "https://github.com"
    );
    assert_eq!(
        github_server_url_from_api("https://github.example.com/api/v3"),
        "https://github.example.com"
    );
}

#[test]
fn backfill_feed_channel_uses_api_base_url() {
    let repo = tempfile::tempdir().unwrap();
    let args = BackfillArgs {
        repo_root: repo.path().to_path_buf(),
        since: String::new(),
        mode: "artifacts-only".into(),
        dry_run: false,
        repository: "owner/repo".into(),
        github_token: String::new(),
        api_base_url: "https://github.example.com/api/v3".into(),
        confirm_release_body: false,
        max_tags: 0,
        output_file: String::new(),
        output_text_file: String::new(),
        output_html_file: String::new(),
        output_json: String::new(),
        rss_feed_file: "feed.xml".into(),
        rss_max_entries: 50,
        resume_file: PathBuf::new(),
    };
    backfill_write_feed(
        &args,
        "owner/repo",
        vec![FeedItem {
            title: "owner/repo v1.0.0".into(),
            link: "https://github.example.com/owner/repo/releases/tag/v1.0.0".into(),
            guid: "v1.0.0".into(),
            description: "<p>Release</p>".into(),
            pub_date: "Sat, 04 Jul 2026 00:00:00 +0000".into(),
        }],
    )
    .unwrap();

    let feed = fs::read_to_string(repo.path().join("feed.xml")).unwrap();
    assert!(feed.contains("<link>https://github.example.com/owner/repo</link>"));
}
