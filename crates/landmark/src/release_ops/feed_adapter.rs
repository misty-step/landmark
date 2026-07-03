use crate::*;

const SIGNATURE_HEADER: &str = "X-Signature-256";
const FEED_RECEIVER_CONTRACT_ID: &str = "release-feed-receiver";

pub(crate) fn notify_release_feed(args: NotifyReleaseFeedArgs) -> Result<()> {
    let receipt = notify_release_feed_receipt(args)?;
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}

pub(crate) fn notify_release_feed_receipt(args: NotifyReleaseFeedArgs) -> Result<Value> {
    let Some(receiver_url) = release_feed_config(
        &args.receiver_url,
        &[
            "RELEASE_FEED_URL",
            "LANDMARK_RELEASE_FEED_URL",
            "RELEASE_KIT_FEED_URL",
        ],
    ) else {
        return write_release_feed_skip(&args.receipt_file, "missing RELEASE_FEED_URL");
    };
    let Some(receiver_secret) = release_feed_config(
        &args.receiver_secret,
        &[
            "RELEASE_FEED_SECRET",
            "LANDMARK_RELEASE_FEED_SECRET",
            "RELEASE_KIT_FEED_SECRET",
        ],
    ) else {
        return write_release_feed_skip(&args.receipt_file, "missing RELEASE_FEED_SECRET");
    };

    validate_url(&receiver_url)?;
    let evidence: Value = serde_json::from_str(&fs::read_to_string(&args.evidence_file)?)?;
    let mut release_kit: Value =
        serde_json::from_str(&fs::read_to_string(&args.release_kit_file)?)?;
    enrich_release_feed_kit(
        &mut release_kit,
        &evidence,
        &args.evidence_file,
        &args.release_kit_file,
        &args.receipt_file,
    )?;
    let body = serde_json::to_string_pretty(&release_kit)? + "\n";
    let signature = compute_signature(&receiver_secret, body.as_bytes())?;
    let response = post_release_feed_event(&receiver_url, &signature, &body)?;
    let receipt = json!({
        "sent": (200..300).contains(&response.status),
        "skipped": false,
        "status": response.status,
        "release_tag": evidence["release_tag"],
        "artifact_count": release_kit["artifacts"].as_array().map(Vec::len).unwrap_or_default(),
        "producer_contract": FEED_RECEIVER_CONTRACT_ID,
    });
    write_json_if_requested(&args.receipt_file, &receipt)?;
    if (200..300).contains(&response.status) {
        Ok(receipt)
    } else {
        Err(format!("release feed receiver returned HTTP {}", response.status).into())
    }
}

fn release_feed_config(arg_value: &str, env_names: &[&str]) -> Option<String> {
    trimmed_option(arg_value).or_else(|| {
        env_names
            .iter()
            .filter_map(|name| env::var(name).ok())
            .find_map(|value| trimmed_option(&value))
    })
}

fn write_release_feed_skip(receipt_file: &Path, reason: &str) -> Result<Value> {
    let receipt = json!({
        "sent": false,
        "skipped": true,
        "reason": reason,
    });
    write_json_if_requested(receipt_file, &receipt)?;
    Ok(receipt)
}

fn enrich_release_feed_kit(
    release_kit: &mut Value,
    evidence: &Value,
    evidence_file: &Path,
    release_kit_file: &Path,
    receipt_file: &Path,
) -> Result<()> {
    if release_kit["schema_version"].as_str() != Some(release_kit::SCHEMA_VERSION) {
        return Err("release kit schema_version must be landmark.release-kit.v1".into());
    }
    let version_decision = evidence
        .get("version_decision")
        .ok_or("evidence missing version_decision")?;
    let changed_files = evidence
        .get("changed_files")
        .and_then(Value::as_array)
        .ok_or("evidence missing changed_files")?;
    let changelog_path = evidence
        .pointer("/artifacts/technical_changelog")
        .and_then(Value::as_str)
        .and_then(trimmed_option)
        .ok_or("evidence missing artifacts.technical_changelog")?;
    let changelog_sha = evidence["technical_changelog_sha256"]
        .as_str()
        .and_then(trimmed_option)
        .unwrap_or_else(|| {
            fs::read(&changelog_path)
                .map(|bytes| sha256_hex(&bytes))
                .unwrap_or_default()
        });
    if changelog_sha.is_empty() {
        return Err("evidence missing technical_changelog_sha256".into());
    }

    if let Some(release_decision) = release_kit.pointer_mut("/release/version_decision") {
        *release_decision = version_decision.clone();
    }

    let evidence_ref = evidence_file.display().to_string();
    let release_kit_ref = release_kit_file.display().to_string();
    upsert_artifact(
        release_kit,
        json!({
            "id": "version-decision",
            "kind": "other",
            "audience": "developer-operator",
            "owner": "producer-adapter",
            "status": "produced",
            "path": format!("{evidence_ref}#/version_decision"),
            "sha256": sha256_json(version_decision)?,
            "acceptance": [
                "Carries the exact deterministic version decision from run-evidence.v1.",
                "Names the bump, release range, decisive commit, and unknown commits."
            ]
        }),
    )?;
    upsert_artifact(
        release_kit,
        json!({
            "id": "changed-files",
            "kind": "other",
            "audience": "developer-operator",
            "owner": "producer-adapter",
            "status": "produced",
            "path": format!("{evidence_ref}#/changed_files"),
            "sha256": sha256_json(&Value::Array(changed_files.clone()))?,
            "acceptance": [
                "Carries the release changed-file list from run-evidence.v1.",
                "Lets the feed receiver show scope without re-reading the repository."
            ]
        }),
    )?;
    upsert_artifact(
        release_kit,
        json!({
            "id": "changelog-diff",
            "kind": "technical_changelog",
            "audience": "developer-operator",
            "owner": "producer-adapter",
            "status": "produced",
            "path": changelog_path,
            "sha256": changelog_sha,
            "acceptance": [
                "Points at the technical changelog generated from the release range.",
                "Matches the technical_changelog_sha256 recorded in run-evidence.v1."
            ]
        }),
    )?;

    for artifact_id in ["version-decision", "changed-files", "changelog-diff"] {
        upsert_provenance(
            release_kit,
            json!({
                "artifact_id": artifact_id,
                "sources": [
                    format!("run_evidence:{evidence_ref}"),
                    format!("release_kit:{release_kit_ref}")
                ],
                "notes": "Produced by the release feed adapter from Landmark evidence."
            }),
        )?;
        upsert_approval(
            release_kit,
            json!({
                "artifact_id": artifact_id,
                "state": "not-required",
                "approver": "release-feed-adapter",
                "reason": "Text-floor artifact is derived directly from run evidence."
            }),
        )?;
    }
    upsert_producer_contract(
        release_kit,
        json!({
            "id": FEED_RECEIVER_CONTRACT_ID,
            "producer": "release feed receiver",
            "adapter_kind": "remote-service",
            "input_artifacts": ["technical-changelog", "release-notes"],
            "output_artifacts": ["version-decision", "changed-files", "changelog-diff"],
            "command": "POST /v1/events with X-Signature-256 over the raw release-kit JSON body",
            "mutates": true,
            "acceptance": [
                "Receiver validates the HMAC-SHA256 signature before accepting the event.",
                "Receiver stores the full landmark.release-kit.v1 JSON body, including text-floor evidence artifacts."
            ],
            "evidence_path": if receipt_file.as_os_str().is_empty() {
                ".landmark/run/producers/release-feed-receiver.json".to_string()
            } else {
                receipt_file.display().to_string()
            }
        }),
    )?;
    Ok(())
}

fn sha256_json(value: &Value) -> Result<String> {
    Ok(sha256_hex(&serde_json::to_vec(value)?))
}

fn upsert_artifact(release_kit: &mut Value, artifact: Value) -> Result<()> {
    upsert_object_by_key(release_kit, "/artifacts", "id", artifact)
}

fn upsert_provenance(release_kit: &mut Value, provenance: Value) -> Result<()> {
    upsert_object_by_key(release_kit, "/provenance", "artifact_id", provenance)
}

fn upsert_approval(release_kit: &mut Value, approval: Value) -> Result<()> {
    upsert_object_by_key(release_kit, "/approvals", "artifact_id", approval)
}

fn upsert_producer_contract(release_kit: &mut Value, contract: Value) -> Result<()> {
    upsert_object_by_key(release_kit, "/producer_contracts", "id", contract)
}

fn upsert_object_by_key(
    release_kit: &mut Value,
    pointer: &str,
    key: &str,
    replacement: Value,
) -> Result<()> {
    let id = replacement[key]
        .as_str()
        .ok_or_else(|| format!("replacement missing {key}"))?
        .to_string();
    let items = release_kit
        .pointer_mut(pointer)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("release kit missing array at {pointer}"))?;
    if let Some(existing) = items
        .iter_mut()
        .find(|item| item[key].as_str() == Some(id.as_str()))
    {
        *existing = replacement;
    } else {
        items.push(replacement);
    }
    Ok(())
}

fn post_release_feed_event(url: &str, signature: &str, body: &str) -> Result<HttpResponse> {
    let mut last_error = String::new();
    for attempt in 1..=3 {
        match post_release_feed_event_once(url, signature, body) {
            Ok(response) if !http_status_retryable(response.status) || attempt == 3 => {
                return Ok(response);
            }
            Ok(response) => {
                last_error = format!("HTTP {}", response.status);
            }
            Err(error) if attempt == 3 => return Err(error),
            Err(error) => {
                last_error = error.to_string();
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err(last_error.into())
}

fn post_release_feed_event_once(url: &str, signature: &str, body: &str) -> Result<HttpResponse> {
    let mut config = String::new();
    push_curl_config(&mut config, "request", "POST");
    push_curl_config(&mut config, "header", "Accept: application/json");
    push_curl_config(&mut config, "header", "Content-Type: application/json");
    push_curl_config(&mut config, "header", "User-Agent: landmark");
    push_curl_config(
        &mut config,
        "header",
        &format!("{SIGNATURE_HEADER}: {signature}"),
    );
    push_curl_config(&mut config, "write-out", "\n%{http_code}");
    push_curl_config(&mut config, "url", url);
    push_curl_config(&mut config, "data-binary", body);
    let mut child = Command::new("curl")
        .args([
            "-sS",
            "-L",
            "--connect-timeout",
            "5",
            "--max-time",
            "30",
            "-K",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("failed to open curl stdin")?
        .write_all(config.as_bytes())?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(redact_known_secrets(&String::from_utf8_lossy(&output.stderr)).into());
    }
    let raw = String::from_utf8(output.stdout)?;
    let (body, status) = raw
        .trim_end()
        .rsplit_once('\n')
        .ok_or("curl status marker missing")?;
    Ok(HttpResponse {
        status: status.parse()?,
        body: body.to_string(),
    })
}
