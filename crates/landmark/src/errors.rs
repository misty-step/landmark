use crate::*;

pub(crate) fn structured_error_json(message: &str) -> String {
    let failure = classify_failure(message);
    serde_json::to_string_pretty(&json!({
        "error": {
            "code": failure.code,
            "stage": failure.stage,
            "retryable": failure.retryable,
            "user_action": failure.user_action,
            "context": {
                "message": redact_context(message)
            }
        }
    }))
    .unwrap_or_else(|_| "{\"error\":{\"code\":\"internal_error\",\"stage\":\"internal\",\"retryable\":false,\"user_action\":\"inspect stderr\",\"context\":{}}}".into())
}

pub(crate) struct FailureClass {
    pub(crate) code: &'static str,
    pub(crate) stage: &'static str,
    pub(crate) retryable: bool,
    pub(crate) user_action: &'static str,
}

pub(crate) fn classify_failure(message: &str) -> FailureClass {
    let lower = message.to_ascii_lowercase();
    if lower.contains("github-token") || lower.contains("gh_token") || lower.contains("auth") {
        FailureClass {
            code: "provider_auth",
            stage: "provider",
            retryable: false,
            user_action: "Provide a valid provider token through the documented secret or environment variable.",
        }
    } else if lower.contains("http 429")
        || lower.contains("rate limit")
        || lower.contains("timeout")
    {
        FailureClass {
            code: "provider_outage",
            stage: "provider",
            retryable: true,
            user_action: "Retry after the provider recovers or reduce request volume.",
        }
    } else if lower.contains("changelog.source") || lower.contains("invalid changelog") {
        FailureClass {
            code: "invalid_changelog_source",
            stage: "configuration",
            retryable: false,
            user_action: "Set changelog.source to auto, changelog, release-body, or prs.",
        }
    } else if lower.contains("budget") || lower.contains("model.policy=off") {
        FailureClass {
            code: "budget_skip",
            stage: "synthesis",
            retryable: false,
            user_action: "Raise the configured budget, change model policy, or accept synthesis skip.",
        }
    } else if lower.contains("degraded") || lower.contains("quality") {
        FailureClass {
            code: "synthesis_degradation",
            stage: "synthesis",
            retryable: false,
            user_action: "Inspect synthesis attempts and either improve source context or relax strict synthesis policy.",
        }
    } else if lower.contains("release body") || lower.contains("publish-release-body") {
        FailureClass {
            code: "publication_mutation_failure",
            stage: "publication",
            retryable: true,
            user_action: "Check release existence, provider permissions, and publication mode.",
        }
    } else if lower.contains("rss") || lower.contains("feed") {
        FailureClass {
            code: "feed_failure",
            stage: "artifact",
            retryable: false,
            user_action: "Check feed path, max entries, and existing feed XML.",
        }
    } else if lower.contains("write") || lower.contains("file") || lower.contains("permission") {
        FailureClass {
            code: "artifact_write_failure",
            stage: "artifact",
            retryable: false,
            user_action: "Check output paths and filesystem permissions.",
        }
    } else if lower.contains("unsupported provider")
        || lower.contains("requires")
        || lower.contains("must")
    {
        FailureClass {
            code: "invalid_input",
            stage: "configuration",
            retryable: false,
            user_action: "Correct the command arguments and retry.",
        }
    } else {
        FailureClass {
            code: "command_failed",
            stage: "runtime",
            retryable: false,
            user_action: "Inspect the command context and Landmark evidence packet.",
        }
    }
}

pub(crate) fn redact_context(value: &str) -> String {
    static TOKEN_RE: OnceLock<Regex> = OnceLock::new();
    let token_re = TOKEN_RE.get_or_init(|| {
        Regex::new(r"(ghp|github_pat|sk|xox[baprs])[-_][-_=A-Za-z0-9]{8,}").unwrap()
    });
    redact_configured_secrets(&token_re.replace_all(value, "[REDACTED]"))
}

pub(crate) fn redact_known_secrets(value: &str) -> String {
    redact_context(value)
}

pub(crate) fn redact_configured_secrets(value: &str) -> String {
    redact_secret_values(
        value,
        configured_secret_env_names()
            .into_iter()
            .filter_map(|name| env::var(name).ok()),
    )
}

pub(crate) fn redact_secret_values<I>(value: &str, secrets: I) -> String
where
    I: IntoIterator<Item = String>,
{
    let mut redacted = value.to_string();
    for secret in secrets {
        if secret.len() >= 8 && redacted.contains(&secret) {
            redacted = redacted.replace(&secret, "[REDACTED]");
        }
    }
    redacted
}

pub(crate) fn configured_secret_env_names() -> [&'static str; 5] {
    [
        "GITHUB_TOKEN",
        "GH_TOKEN",
        "OPENROUTER_API_KEY",
        "WEBHOOK_SECRET",
        "SLACK_WEBHOOK_URL",
    ]
}
