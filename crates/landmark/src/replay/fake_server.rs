use crate::*;
use tiny_http::{Header, Method, Response, Server};
pub(crate) struct FakeServer {
    pub(crate) url: String,
    pub(crate) state: Arc<Mutex<FakeState>>,
}

pub(crate) fn start_fake_server(mut state: FakeState) -> Result<FakeServer> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let server = Server::from_listener(listener, None).map_err(|error| error.to_string())?;
    let shared = Arc::new(Mutex::new({
        state.llm_notes = state.llm_notes.trim().to_string();
        state
    }));
    let thread_state = Arc::clone(&shared);
    thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            let path = request.url().to_string();
            let method = request.method().clone();
            let mut state = thread_state.lock().unwrap();
            state
                .requests
                .push(json!({"method": method.as_str(), "path": path, "body": body}));
            let response = match (method, request.url()) {
                (Method::Post, "/chat/completions") => {
                    let (status, notes) = state
                        .llm_responses
                        .pop_front()
                        .unwrap_or_else(|| (state.llm_status, state.llm_notes.clone()));
                    if status >= 400 {
                        json_response(status, json!({"error": {"message": "fake LLM failure"}}))
                    } else {
                        json_response(200, json!({"choices": [{"message": {"content": notes}}]}))
                    }
                }
                (Method::Get, url) if url.contains("/releases/tags/") => {
                    let tag = url.rsplit("/releases/tags/").next().unwrap();
                    let tag = urlencoding::decode(tag).unwrap_or_default().to_string();
                    if let Some(release) = state.releases.get(&tag) {
                        json_response(200, release.clone())
                    } else {
                        json_response(404, json!({"message": "Not Found"}))
                    }
                }
                (Method::Patch, url) if url.contains("/releases/") => {
                    if state.update_status >= 400 {
                        json_response(state.update_status, json!({"message": "update failed"}))
                    } else {
                        let id: i64 = url.rsplit('/').next().unwrap_or("0").parse().unwrap_or(0);
                        let payload: Value =
                            serde_json::from_str(&body).unwrap_or_else(|_| json!({}));
                        let mut found = None;
                        for release in state.releases.values_mut() {
                            if release["id"].as_i64() == Some(id) {
                                if let Some(new_body) = payload["body"].as_str() {
                                    release["body"] = Value::String(new_body.to_string());
                                }
                                found = Some(release.clone());
                                break;
                            }
                        }
                        found
                            .map(|release| json_response(200, release))
                            .unwrap_or_else(|| json_response(404, json!({"message": "Not Found"})))
                    }
                }
                (Method::Post, url) if url.contains("/repos/") && url.ends_with("/releases") => {
                    let payload: Value = serde_json::from_str(&body).unwrap_or_else(|_| json!({}));
                    let tag = payload["tag_name"].as_str().unwrap_or("").to_string();
                    if tag.is_empty() {
                        json_response(422, json!({"message": "tag_name is required"}))
                    } else if state.releases.contains_key(&tag) {
                        json_response(422, json!({"message": "already_exists"}))
                    } else {
                        let id = state.releases.len() as i64 + 1;
                        let release = json!({
                            "id": id,
                            "tag_name": tag,
                            "target_commitish": payload["target_commitish"],
                            "name": payload["name"],
                            "body": payload["body"],
                            "html_url": format!("https://example.invalid/releases/{}", payload["tag_name"].as_str().unwrap_or(""))
                        });
                        state.releases.insert(tag, release.clone());
                        json_response(201, release)
                    }
                }
                _ => json_response(404, json!({"message": "not found"})),
            };
            let _ = request.respond(response);
        }
    });
    thread::sleep(Duration::from_millis(50));
    Ok(FakeServer {
        url: format!("http://{addr}"),
        state: shared,
    })
}

pub(crate) fn start_slow_http_server(delay: Duration) -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let server = Server::from_listener(listener, None).map_err(|error| error.to_string())?;
    thread::spawn(move || {
        if let Some(request) = server.incoming_requests().next() {
            thread::sleep(delay);
            let _ = request.respond(json_response(200, json!({"ok": true})));
        }
    });
    Ok(format!("http://{addr}/slow"))
}

pub(crate) fn json_response(status: u16, payload: Value) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::to_vec(&payload).unwrap();
    Response::from_data(body)
        .with_status_code(status)
        .with_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
}
