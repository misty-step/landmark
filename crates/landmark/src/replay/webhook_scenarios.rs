use crate::*;
use tiny_http::{Response, Server};

pub(crate) fn scenario_webhook_delivery_outcomes(tmp_root: &Path) -> Result<Value> {
    let notes_file = tmp_root.join("webhook-notes.md");
    fs::write(&notes_file, VALID_NOTES)?;
    let mut outcomes = Vec::new();
    for (name, statuses, signed, succeeds) in [
        ("accepted", vec![204_u16], true, true),
        ("unsigned", vec![204], false, true),
        ("binary-accepted", vec![200], true, true),
        ("binary-rejected", vec![400], true, false),
        ("rejected", vec![400], true, false),
        ("exhausted", vec![503, 503, 503], true, false),
        ("recovered", vec![503, 204], true, true),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let server = Server::from_listener(listener, None).map_err(|error| error.to_string())?;
        let expected_requests = statuses.len();
        let final_status = *statuses.last().unwrap();
        let receiver = thread::spawn(move || {
            let mut received = Vec::new();
            for status in statuses {
                let Some(mut request) = server
                    .recv_timeout(Duration::from_secs(3))
                    .expect("receive loopback webhook")
                else {
                    break;
                };
                let mut body = String::new();
                request.as_reader().read_to_string(&mut body).unwrap();
                let signature = request
                    .headers()
                    .iter()
                    .find(|header| header.field.equiv("X-Signature-256"))
                    .map(|header| header.value.as_str().to_string());
                received.push((request.method().as_str().to_string(), body, signature));
                let response_body = if name.starts_with("binary-") {
                    vec![0xff]
                } else {
                    b"private receiver detail".to_vec()
                };
                request
                    .respond(Response::from_data(response_body).with_status_code(status))
                    .unwrap();
            }
            received
        });
        let output = Command::new(current_exe())
            .args([
                "notify-webhook",
                "--webhook-url",
                &format!("http://{address}/notify"),
                "--webhook-secret",
                if signed { "synthetic-webhook-key" } else { "" },
                "--version",
                "v1.2.3",
                "--repository",
                "fixture/product",
                "--release-url",
                "https://example.invalid/releases/v1.2.3",
                "--notes-file",
            ])
            .arg(&notes_file)
            .output()?;
        let received = receiver.join().map_err(|_| "webhook receiver panicked")?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.success() != succeeds || received.len() != expected_requests {
            return Err(format!(
                "webhook {name}: exit {:?}, {} requests; expected success={succeeds}, {expected_requests} requests",
                output.status.code(), received.len()
            )
            .into());
        }
        if (!succeeds && !stderr.contains(&format!("HTTP {final_status}")))
            || stderr.contains("private receiver detail")
        {
            return Err(format!("webhook {name}: missing status or exposed receiver content").into());
        }
        for (method, body, signature) in &received {
            let expected_signature = if signed {
                Some(compute_signature("synthetic-webhook-key", body.as_bytes())?)
            } else {
                None
            };
            let payload: Value = serde_json::from_str(body)?;
            if method != "POST"
                || signature != &expected_signature
                || payload["version"] != "v1.2.3"
                || payload["repository"] != "fixture/product"
                || payload["notes"] != VALID_NOTES.trim()
            {
                return Err(format!("webhook {name}: method, payload or signature mismatch").into());
            }
        }
        if received.windows(2).any(|pair| pair[0].1 != pair[1].1) {
            return Err(format!("webhook {name}: retry changed signed payload").into());
        }
        outcomes.push(json!({
            "case": name,
            "returncode": output.status.code(),
            "requests": received.len(),
            "signed": signed,
            "status": final_status,
        }));
    }
    Ok(json!({"outcomes": outcomes}))
}
