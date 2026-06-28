use crate::*;

#[derive(Debug)]
pub(crate) struct HttpResponse {
    pub(crate) status: u16,
    pub(crate) body: String,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct HttpPolicy {
    pub(crate) connect_timeout_seconds: u64,
    pub(crate) max_time_seconds: u64,
    pub(crate) attempts: usize,
    pub(crate) retry_delay_ms: u64,
}

impl Default for HttpPolicy {
    fn default() -> Self {
        Self {
            connect_timeout_seconds: 5,
            max_time_seconds: 30,
            attempts: 3,
            retry_delay_ms: 250,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CurlInvocation {
    pub(crate) args: Vec<String>,
    pub(crate) config: String,
}

pub(crate) fn curl_json(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<&Value>,
) -> Result<HttpResponse> {
    curl_json_with_policy(method, url, token, body, HttpPolicy::default())
}

pub(crate) fn curl_json_with_policy(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<&Value>,
    policy: HttpPolicy,
) -> Result<HttpResponse> {
    let attempts = policy.attempts.max(1);
    let mut last_error = String::new();
    for attempt in 1..=attempts {
        match curl_json_once(method, url, token, body, policy) {
            Ok(response) if !http_status_retryable(response.status) || attempt == attempts => {
                return Ok(response);
            }
            Ok(response) => {
                last_error = format!("HTTP {}", response.status);
            }
            Err(error) if attempt == attempts => return Err(error),
            Err(error) => {
                last_error = error.to_string();
            }
        }
        thread::sleep(Duration::from_millis(policy.retry_delay_ms));
    }
    Err(last_error.into())
}

pub(crate) fn curl_json_once(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<&Value>,
    policy: HttpPolicy,
) -> Result<HttpResponse> {
    let invocation = build_curl_invocation(method, url, token, body, policy);
    let mut child = Command::new("curl")
        .args(&invocation.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("failed to open curl stdin")?
        .write_all(invocation.config.as_bytes())?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(redact_known_secrets(&String::from_utf8_lossy(&output.stderr)).into());
    }
    let raw = String::from_utf8(output.stdout)?;
    let raw = raw.trim_end();
    let (body, status) = raw.rsplit_once('\n').ok_or("curl status marker missing")?;
    Ok(HttpResponse {
        status: status.parse()?,
        body: body.to_string(),
    })
}

pub(crate) fn build_curl_invocation(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<&Value>,
    policy: HttpPolicy,
) -> CurlInvocation {
    let args = vec![
        "-sS".to_string(),
        "-L".to_string(),
        "--connect-timeout".to_string(),
        policy.connect_timeout_seconds.to_string(),
        "--max-time".to_string(),
        policy.max_time_seconds.to_string(),
        "-K".to_string(),
        "-".to_string(),
    ];
    let mut config = String::new();
    push_curl_config(&mut config, "request", method);
    push_curl_config(&mut config, "header", "Accept: application/vnd.github+json");
    push_curl_config(&mut config, "header", "User-Agent: landmark");
    push_curl_config(&mut config, "write-out", "\n%{http_code}");
    push_curl_config(&mut config, "url", url);
    if let Some(token) = token {
        push_curl_config(
            &mut config,
            "header",
            &format!("Authorization: Bearer {token}"),
        );
    }
    if let Some(body) = body {
        push_curl_config(&mut config, "header", "Content-Type: application/json");
        push_curl_config(&mut config, "data", &body.to_string());
    }
    CurlInvocation { args, config }
}

pub(crate) fn push_curl_config(config: &mut String, key: &str, value: &str) {
    config.push_str(key);
    config.push_str(" = \"");
    config.push_str(&escape_curl_config_value(value));
    config.push_str("\"\n");
}

pub(crate) fn escape_curl_config_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

pub(crate) fn http_status_retryable(status: u16) -> bool {
    status == 408 || status == 425 || status == 429 || (500..600).contains(&status)
}

#[derive(Clone, Debug)]
pub(crate) struct GitHubProvider {
    pub(crate) api_base_url: String,
    pub(crate) token: Option<String>,
}

impl GitHubProvider {
    pub(crate) fn new(api_base_url: &str, token: Option<&str>) -> Self {
        Self {
            api_base_url: api_base_url.trim_end_matches('/').to_string(),
            token: token.map(str::to_string),
        }
    }

    pub(crate) fn required(api_base_url: &str, token: &str) -> Self {
        Self::new(api_base_url, Some(token))
    }

    pub(crate) fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub(crate) fn release_by_tag(&self, repository: &str, tag: &str) -> Result<Option<Value>> {
        validate_repo(repository)?;
        let response = curl_json(
            "GET",
            &self.release_by_tag_url(repository, tag),
            self.token(),
            None,
        )?;
        if response.status == 404 {
            return Ok(None);
        }
        if !(200..300).contains(&response.status) {
            return Err(
                format!("GitHub release fetch failed with HTTP {}", response.status).into(),
            );
        }
        Ok(Some(serde_json::from_str(&response.body)?))
    }

    pub(crate) fn update_release_body(
        &self,
        repository: &str,
        tag: &str,
        notes: &str,
    ) -> Result<String> {
        let release = self
            .release_by_tag(repository, tag)?
            .ok_or_else(|| format!("release {tag} not found"))?;
        let id = release["id"]
            .as_i64()
            .ok_or("release response missing id")?;
        let existing = release["body"].as_str().unwrap_or("");
        let update = curl_json(
            "PATCH",
            &self.release_by_id_url(repository, id),
            self.token(),
            Some(&json!({ "body": compose_release_body(notes, existing) })),
        )?;
        if !(200..300).contains(&update.status) {
            return Err(format!("GitHub release update failed with HTTP {}", update.status).into());
        }
        Ok(release["html_url"]
            .as_str()
            .unwrap_or(&format!(
                "https://github.com/{repository}/releases/tag/{tag}"
            ))
            .to_string())
    }

    pub(crate) fn create_release(
        &self,
        repository: &str,
        tag: &str,
        target_commitish: &str,
        body: &str,
    ) -> Result<String> {
        validate_repo(repository)?;
        let response = curl_json(
            "POST",
            &format!("{}/repos/{repository}/releases", self.api_base_url),
            self.token(),
            Some(&json!({
                "tag_name": tag,
                "target_commitish": target_commitish,
                "name": tag,
                "body": body,
                "draft": false,
                "prerelease": false
            })),
        )?;
        if !(200..300).contains(&response.status) {
            return Err(format!(
                "GitHub release creation failed with HTTP {}",
                response.status
            )
            .into());
        }
        let value: Value = serde_json::from_str(&response.body)?;
        Ok(value["html_url"].as_str().unwrap_or("").to_string())
    }

    pub(crate) fn closed_pull_requests(&self, repository: &str) -> Result<Vec<Value>> {
        validate_repo(repository)?;
        let response = curl_json(
            "GET",
            &format!(
                "{}/repos/{repository}/pulls?state=closed&per_page=100",
                self.api_base_url
            ),
            self.token(),
            None,
        )?;
        if !(200..300).contains(&response.status) {
            return Err(format!("GitHub PR fetch failed with HTTP {}", response.status).into());
        }
        Ok(serde_json::from_str(&response.body)?)
    }

    pub(crate) fn tree_paths(&self, repository: &str, branch: &str) -> Result<Vec<String>> {
        let output = run_gh_ok(
            vec![
                "api".into(),
                format!(
                    "repos/{repository}/git/trees/{}?recursive=1",
                    urlencoding::encode(branch)
                ),
                "--jq".into(),
                "[.tree[].path]".into(),
            ],
            self.token(),
        )?;
        Ok(serde_json::from_str(&output)?)
    }

    pub(crate) fn tags(&self, repository: &str) -> Result<Vec<String>> {
        let output = run_gh_ok(
            vec![
                "api".into(),
                format!("repos/{repository}/tags?per_page=30"),
                "--jq".into(),
                "[.[].name]".into(),
            ],
            self.token(),
        )?;
        Ok(serde_json::from_str(&output)?)
    }

    pub(crate) fn workflow_texts(
        &self,
        repository: &str,
        branch: &str,
        workflows: &[String],
    ) -> Vec<(String, String)> {
        workflows
            .iter()
            .filter_map(|workflow| {
                let output = run_gh_ok(
                    vec![
                        "api".into(),
                        format!(
                            "repos/{repository}/contents/.github/workflows/{}?ref={}",
                            urlencoding::encode(workflow),
                            urlencoding::encode(branch)
                        ),
                        "--header".into(),
                        "Accept: application/vnd.github.raw".into(),
                    ],
                    self.token(),
                )
                .ok()?;
                Some((workflow.clone(), output))
            })
            .collect()
    }

    pub(crate) fn branch_protection_status(&self, repository: &str, branch: &str) -> String {
        let Some(token) = self.token() else {
            return "unavailable: no GitHub token supplied".into();
        };
        let url = format!(
            "{}/repos/{repository}/branches/{}/protection",
            self.api_base_url,
            urlencoding::encode(branch)
        );
        match curl_json("GET", &url, Some(token), None) {
            Ok(response) if response.status == 200 => "protected".into(),
            Ok(response) if response.status == 404 => "unprotected-or-unavailable".into(),
            Ok(response) => format!("unavailable: HTTP {}", response.status),
            Err(error) => format!("unavailable: {error}"),
        }
    }

    pub(crate) fn secret_statuses(
        &self,
        repository: &str,
        required: &[&str],
    ) -> Vec<FleetSecretStatus> {
        let Some(token) = self.token() else {
            return unavailable_secret_statuses(
                required,
                "secret metadata requires a GitHub token with repository access",
            );
        };
        let response = match self.secret_names(repository, token) {
            Ok(response) => response,
            Err(error) => return unavailable_secret_statuses(required, &error.to_string()),
        };
        required
            .iter()
            .map(|name| FleetSecretStatus {
                name: (*name).to_string(),
                status: if response.names.contains(*name) {
                    "present".into()
                } else if response.org_unavailable.is_some() {
                    "unavailable".into()
                } else {
                    "missing".into()
                },
                detail: if response.repo_names.contains(*name) {
                    "repo secret metadata only; value not read".into()
                } else if response.org_names.contains(*name) {
                    "org secret metadata only; value not read".into()
                } else if let Some(error) = &response.org_unavailable {
                    format!("org secret metadata unavailable: {error}")
                } else {
                    "required secret name is absent from Actions secret metadata".into()
                },
            })
            .collect()
    }

    pub(crate) fn secret_names(&self, repository: &str, token: &str) -> Result<FleetSecretNames> {
        let repo_names = self.repo_secret_names(repository, token)?;
        let (org_names, org_unavailable) = match self.org_secret_names(repository, token) {
            Ok(Some(names)) => (names, None),
            Ok(None) => (BTreeSet::new(), None),
            Err(error) => (BTreeSet::new(), Some(error.to_string())),
        };
        let names = repo_names.union(&org_names).cloned().collect();
        Ok(FleetSecretNames {
            names,
            repo_names,
            org_names,
            org_unavailable,
        })
    }

    pub(crate) fn repo_secret_names(
        &self,
        repository: &str,
        token: &str,
    ) -> Result<BTreeSet<String>> {
        let response = curl_json(
            "GET",
            &format!(
                "{}/repos/{repository}/actions/secrets?per_page=100",
                self.api_base_url
            ),
            Some(token),
            None,
        )
        .map_err(|error| format!("secret metadata unavailable: {error}"))?;
        if !(200..300).contains(&response.status) {
            return Err(format!(
                "GitHub returned HTTP {} for secret metadata",
                response.status
            )
            .into());
        }
        let value: Value = serde_json::from_str(&response.body)
            .map_err(|error| format!("secret metadata parse failed: {error}"))?;
        Ok(secret_names_from_array(&value))
    }

    pub(crate) fn org_secret_names(
        &self,
        repository: &str,
        token: &str,
    ) -> Result<Option<BTreeSet<String>>> {
        let (owner, repo_name) = repository
            .split_once('/')
            .ok_or_else(|| format!("invalid repository {repository}"))?;
        let response = curl_json(
            "GET",
            &format!(
                "{}/orgs/{owner}/actions/secrets?per_page=100",
                self.api_base_url
            ),
            Some(token),
            None,
        )?;
        if response.status == 404 {
            return Ok(None);
        }
        if !(200..300).contains(&response.status) {
            return Err(format!(
                "GitHub returned HTTP {} for org secret metadata",
                response.status
            )
            .into());
        }
        let value: Value = serde_json::from_str(&response.body)?;
        Ok(Some(org_secret_names_for_repo(
            &value, repository, repo_name,
        )))
    }

    pub(crate) fn find_failure_issues(
        &self,
        repository: &str,
        release_tag: &str,
    ) -> Result<Vec<Value>> {
        validate_repo(repository)?;
        let response = curl_json(
            "GET",
            &format!(
                "{}/repos/{repository}/issues?state=open&labels=landmark,release-notes&per_page=100",
                self.api_base_url
            ),
            self.token(),
            None,
        )?;
        if !(200..300).contains(&response.status) {
            return Err(format!("issue search failed with HTTP {}", response.status).into());
        }
        let issues: Vec<Value> = serde_json::from_str(&response.body)?;
        let title = failure_issue_title(release_tag);
        Ok(issues
            .into_iter()
            .filter(|issue| issue["title"].as_str() == Some(&title))
            .collect())
    }

    pub(crate) fn create_failure_issue(
        &self,
        repository: &str,
        title: &str,
        body: &str,
    ) -> Result<()> {
        let response = curl_json(
            "POST",
            &format!("{}/repos/{repository}/issues", self.api_base_url),
            self.token(),
            Some(&json!({"title": title, "body": body, "labels": ["landmark", "release-notes"]})),
        )?;
        if (200..300).contains(&response.status) {
            Ok(())
        } else {
            Err(format!("issue creation failed with HTTP {}", response.status).into())
        }
    }

    pub(crate) fn comment_issue(&self, repository: &str, number: i64, body: &str) -> Result<()> {
        let _ = curl_json(
            "POST",
            &format!(
                "{}/repos/{repository}/issues/{number}/comments",
                self.api_base_url
            ),
            self.token(),
            Some(&json!({"body": body})),
        )?;
        Ok(())
    }

    pub(crate) fn close_issue(&self, repository: &str, number: i64) -> Result<()> {
        let _ = curl_json(
            "PATCH",
            &format!("{}/repos/{repository}/issues/{number}", self.api_base_url),
            self.token(),
            Some(&json!({"state": "closed"})),
        )?;
        Ok(())
    }

    pub(crate) fn release_by_tag_url(&self, repository: &str, tag: &str) -> String {
        format!(
            "{}/repos/{}/releases/tags/{}",
            self.api_base_url,
            repository,
            urlencoding::encode(tag)
        )
    }

    pub(crate) fn release_by_id_url(&self, repository: &str, id: i64) -> String {
        format!("{}/repos/{repository}/releases/{id}", self.api_base_url)
    }
}
