use crate::*;

pub(crate) fn scenario_release_feed_adapter(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("release-feed-adapter");
    let run_dir = repo.join(".landmark/run");
    fs::create_dir_all(&run_dir)?;
    let technical_path = run_dir.join("technical-changelog.md");
    fs::write(
        &technical_path,
        "## Technical Changelog v1.2.3\n\n- feat(feed): publish release evidence (abc1234)\n",
    )?;
    let technical_sha = sha256_hex(&fs::read(&technical_path)?);
    let release_kit_path = run_dir.join("release-kit.json");
    let evidence_path = run_dir.join("evidence.json");
    let version_decision = json!({
        "latest_tag": "v1.2.2",
        "bump": "minor",
        "commit_count": 1,
        "conventional_commit_count": 1,
        "range": "v1.2.2..HEAD",
        "decisive_commit": "feat(feed): publish release evidence (abc1234)",
        "unknown_commits": []
    });
    let release_kit = json!({
        "schema_version": "landmark.release-kit.v1",
        "generated_at": "2026-07-03T12:00:00Z",
        "product": {"name": "Landmark", "repository": "misty-step/landmark", "audience": "developer", "description": "Release intelligence"},
        "release": {
            "tag": "v1.2.3",
            "version": "1.2.3",
            "previous_tag": "v1.2.2",
            "repository": "misty-step/landmark",
            "release_url": "https://github.com/misty-step/landmark/releases/tag/v1.2.3",
            "version_decision": version_decision
        },
        "classification": {"importance": "medium", "audiences": ["developer"], "why_it_matters": "release feed evidence"},
        "artifacts": [
            {
                "id": "technical-changelog",
                "kind": "technical_changelog",
                "audience": "developer-operator",
                "owner": "landmark",
                "status": "produced",
                "path": technical_path.display().to_string(),
                "sha256": technical_sha,
                "acceptance": ["Preserves raw commit subjects."]
            },
            {
                "id": "release-notes",
                "kind": "release_notes",
                "audience": "developer",
                "owner": "landmark",
                "status": "produced",
                "path": "docs/releases/v1.2.3.md",
                "sha256": "notes-sha",
                "acceptance": ["Summarizes the release."]
            }
        ],
        "producer_contracts": [],
        "provenance": [
            {"artifact_id": "technical-changelog", "sources": ["git:v1.2.2..HEAD"]},
            {"artifact_id": "release-notes", "sources": ["git:v1.2.2..HEAD"]}
        ],
        "approvals": [
            {"artifact_id": "technical-changelog", "state": "not-required"},
            {"artifact_id": "release-notes", "state": "not-required"}
        ],
        "status": {"complete": true, "blocked": false, "summary": "fixture kit"}
    });
    let evidence = json!({
        "provider": "github",
        "generated_at": "2026-07-03T12:00:00Z",
        "repo_root": repo.display().to_string(),
        "repository": "misty-step/landmark",
        "release_tag": "v1.2.3",
        "version": "1.2.3",
        "previous_tag": "v1.2.2",
        "source": "git_range",
        "technical_changelog_sha256": technical_sha,
        "notes_sha256": "notes-sha",
        "version_decision": release_kit["release"]["version_decision"].clone(),
        "changed_files": ["crates/landmark/src/release_ops/feed_adapter.rs"],
        "artifacts": {
            "technical_changelog": technical_path.display().to_string(),
            "technical_changelog_audience": "internal-developer-operator",
            "technical_changelog_schema": "landmark.internal-technical-changelog.v1",
            "markdown": "docs/releases/v1.2.3.md",
            "public_notes_audience": "developer",
            "public_notes_schema": "landmark.public-release-notes.v1",
            "plaintext": "docs/releases/v1.2.3.txt",
            "html": "docs/releases/v1.2.3.html",
            "json": "docs/releases/releases.json",
            "rss": "",
            "evidence": evidence_path.display().to_string(),
            "release_kit": release_kit_path.display().to_string(),
            "release_kit_schema": "landmark.release-kit.v1",
            "release_kit_sha256": "kit-sha"
        },
        "release_kit": release_kit,
        "publication": {
            "provider": "github",
            "enabled": true,
            "release_body_updated": true,
            "release_url": "https://github.com/misty-step/landmark/releases/tag/v1.2.3",
            "status": "updated"
        }
    });
    fs::write(
        &release_kit_path,
        serde_json::to_string_pretty(&evidence["release_kit"])? + "\n",
    )?;
    fs::write(
        &evidence_path,
        serde_json::to_string_pretty(&evidence)? + "\n",
    )?;

    let receiver = start_replay_release_feed_receiver("feed-secret")?;
    let receipt_file = run_dir.join("release-feed-receipt.json");
    notify_release_feed_receipt(NotifyReleaseFeedArgs {
        receiver_url: receiver.url,
        receiver_secret: "feed-secret".into(),
        evidence_file: evidence_path,
        release_kit_file: release_kit_path,
        receipt_file: receipt_file.clone(),
    })?;
    let payload = receiver
        .payload
        .lock()
        .unwrap()
        .clone()
        .ok_or("release feed replay receiver did not capture payload")?;
    let artifact_ids: BTreeSet<String> = payload["artifacts"]
        .as_array()
        .ok_or("release feed payload missing artifacts")?
        .iter()
        .filter_map(|artifact| artifact["id"].as_str().map(str::to_string))
        .collect();
    for id in ["version-decision", "changed-files", "changelog-diff"] {
        if !artifact_ids.contains(id) {
            return Err(format!("release feed payload missing {id}").into());
        }
    }
    Ok(json!({
        "posted": true,
        "receipt": receipt_file,
        "artifact_ids": artifact_ids,
    }))
}

struct ReplayReleaseFeedReceiver {
    url: String,
    payload: Arc<Mutex<Option<Value>>>,
}

fn start_replay_release_feed_receiver(secret: &'static str) -> Result<ReplayReleaseFeedReceiver> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let server =
        tiny_http::Server::from_listener(listener, None).map_err(|error| error.to_string())?;
    let payload = Arc::new(Mutex::new(None));
    let thread_payload = Arc::clone(&payload);
    thread::spawn(move || {
        if let Some(mut request) = server.incoming_requests().next() {
            let signature = request
                .headers()
                .iter()
                .find(|header| header.field.equiv("X-Signature-256"))
                .map(|header| header.value.as_str().to_string())
                .unwrap_or_default();
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            let expected = compute_signature(secret, body.as_bytes()).unwrap_or_default();
            let parsed: Value = serde_json::from_str(&body).unwrap_or_else(|_| json!({}));
            let status = if request.method() == &tiny_http::Method::Post
                && request.url() == "/v1/events"
                && signature == expected
                && parsed["schema_version"] == release_kit::SCHEMA_VERSION
            {
                *thread_payload.lock().unwrap() = Some(parsed);
                202
            } else {
                400
            };
            let _ = request.respond(json_response(status, json!({})));
        }
    });
    thread::sleep(Duration::from_millis(50));
    Ok(ReplayReleaseFeedReceiver {
        url: format!("http://{addr}/v1/events"),
        payload,
    })
}
