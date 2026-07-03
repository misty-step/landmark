use super::*;

#[test]
fn notify_release_feed_posts_signed_release_kit_with_text_floor_artifacts() {
    let repo = tempfile::tempdir().unwrap();
    let run_dir = repo.path().join(".landmark/run");
    fs::create_dir_all(&run_dir).unwrap();
    let technical_path = run_dir.join("technical-changelog.md");
    fs::write(
        &technical_path,
        "## Technical Changelog v1.2.3\n\n- feat(feed): publish release evidence (abc1234)\n",
    )
    .unwrap();
    let technical_sha = sha256_hex(fs::read(&technical_path).unwrap().as_slice());
    let release_kit_path = run_dir.join("release-kit.json");
    let evidence_path = run_dir.join("evidence.json");
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
            "version_decision": {
                "latest_tag": "v1.2.2",
                "bump": "minor",
                "commit_count": 1,
                "conventional_commit_count": 1,
                "range": "v1.2.2..HEAD",
                "decisive_commit": "feat(feed): publish release evidence (abc1234)",
                "unknown_commits": []
            }
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
        "repo_root": repo.path().display().to_string(),
        "repository": "misty-step/landmark",
        "release_tag": "v1.2.3",
        "version": "1.2.3",
        "previous_tag": "v1.2.2",
        "source": "git_range",
        "technical_changelog_sha256": technical_sha,
        "notes_sha256": "notes-sha",
        "version_decision": release_kit["release"]["version_decision"].clone(),
        "changed_files": ["crates/landmark/src/release_ops/artifacts.rs", ".github/workflows/release.yml"],
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
        serde_json::to_string_pretty(&evidence["release_kit"]).unwrap() + "\n",
    )
    .unwrap();
    fs::write(
        &evidence_path,
        serde_json::to_string_pretty(&evidence).unwrap() + "\n",
    )
    .unwrap();

    let receiver = start_release_feed_capture("feed-secret");
    let receipt_path = run_dir.join("feed-receipt.json");

    notify_release_feed(NotifyReleaseFeedArgs {
        receiver_url: receiver.url.clone(),
        receiver_secret: "feed-secret".into(),
        evidence_file: evidence_path.clone(),
        release_kit_file: release_kit_path.clone(),
        receipt_file: receipt_path.clone(),
    })
    .unwrap();

    let captured = receiver.state.lock().unwrap().clone().unwrap();
    assert_eq!(captured.path, "/v1/events");
    assert_eq!(
        captured.signature,
        compute_signature("feed-secret", captured.body.as_bytes()).unwrap()
    );
    let payload: Value = serde_json::from_str(&captured.body).unwrap();
    assert_eq!(payload["schema_version"], "landmark.release-kit.v1");
    assert_release_feed_artifact(&payload, "version-decision", "other");
    assert_release_feed_artifact(&payload, "changed-files", "other");
    assert_release_feed_artifact(&payload, "changelog-diff", "technical_changelog");
    assert!(
        payload["producer_contracts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|contract| contract["id"] == "release-feed-receiver"
                && contract["adapter_kind"] == "remote-service")
    );
    let receipt: Value = serde_json::from_str(&fs::read_to_string(receipt_path).unwrap()).unwrap();
    assert_eq!(receipt["sent"], true);
    assert_eq!(receipt["status"], 202);
}

#[derive(Clone, Debug)]
struct CapturedReleaseFeedRequest {
    path: String,
    signature: String,
    body: String,
}

struct ReleaseFeedCapture {
    url: String,
    state: Arc<Mutex<Option<CapturedReleaseFeedRequest>>>,
}

fn start_release_feed_capture(secret: &'static str) -> ReleaseFeedCapture {
    use tiny_http::{Header, Method, Response, Server};

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = Server::from_listener(listener, None).unwrap();
    let state = Arc::new(Mutex::new(None));
    let thread_state = Arc::clone(&state);
    thread::spawn(move || {
        let mut request = server.incoming_requests().next().unwrap();
        let path = request.url().to_string();
        let method = request.method().clone();
        let signature = request
            .headers()
            .iter()
            .find(|header| header.field.equiv("X-Signature-256"))
            .map(|header| header.value.as_str().to_string())
            .unwrap_or_default();
        let mut body = String::new();
        request.as_reader().read_to_string(&mut body).unwrap();
        let expected = compute_signature(secret, body.as_bytes()).unwrap();
        let payload: Value = serde_json::from_str(&body).unwrap_or_else(|_| json!({}));
        let has_text_floor = ["version-decision", "changed-files", "changelog-diff"]
            .iter()
            .all(|id| {
                payload["artifacts"]
                    .as_array()
                    .unwrap_or(&Vec::new())
                    .iter()
                    .any(|artifact| artifact["id"] == *id && artifact["status"] == "produced")
            });
        *thread_state.lock().unwrap() = Some(CapturedReleaseFeedRequest {
            path: path.clone(),
            signature: signature.clone(),
            body,
        });
        let status = if method == Method::Post
            && path == "/v1/events"
            && signature == expected
            && has_text_floor
        {
            202
        } else {
            400
        };
        let _ = request.respond(
            Response::from_string("{}")
                .with_status_code(status)
                .with_header(
                    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                ),
        );
    });
    ReleaseFeedCapture {
        url: format!("http://{addr}/v1/events"),
        state,
    }
}

fn assert_release_feed_artifact(payload: &Value, id: &str, kind: &str) {
    let artifact = payload["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| artifact["id"] == id)
        .unwrap_or_else(|| panic!("missing artifact {id}"));
    assert_eq!(artifact["kind"], kind);
    assert_eq!(artifact["owner"], "producer-adapter");
    assert_eq!(artifact["status"], "produced");
    assert!(artifact["sha256"].as_str().unwrap_or_default().len() >= 64);
}
