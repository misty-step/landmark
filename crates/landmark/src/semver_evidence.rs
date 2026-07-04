use crate::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct VersionApiEvidence {
    pub(crate) provider: String,
    pub(crate) status: String,
    pub(crate) bump: String,
    pub(crate) baseline: String,
    pub(crate) target: String,
    pub(crate) command: String,
    pub(crate) exit_code: i32,
    pub(crate) summary: String,
    pub(crate) findings: Vec<String>,
    pub(crate) failure_message: String,
}

pub(crate) struct VersionApiEvidenceRequest<'a> {
    pub(crate) repo_root: &'a Path,
    pub(crate) previous_tag: &'a str,
    pub(crate) target_ref: &'a str,
}

pub(crate) trait VersionApiEvidenceProvider {
    fn collect(&self, request: &VersionApiEvidenceRequest<'_>) -> VersionApiEvidence;
}

pub(crate) struct CargoSemverChecksProvider;

pub(crate) fn no_version_api_evidence(reason: &str) -> VersionApiEvidence {
    VersionApiEvidence {
        provider: "none".into(),
        status: "skipped".into(),
        bump: "none".into(),
        baseline: String::new(),
        target: String::new(),
        command: String::new(),
        exit_code: 0,
        summary: format!("no evidence provider: {reason}"),
        findings: Vec::new(),
        failure_message: String::new(),
    }
}

impl VersionApiEvidenceProvider for CargoSemverChecksProvider {
    fn collect(&self, request: &VersionApiEvidenceRequest<'_>) -> VersionApiEvidence {
        if !request.repo_root.join("Cargo.toml").is_file() {
            return no_version_api_evidence("no Cargo.toml at repository root");
        }
        if request.previous_tag.trim().is_empty() {
            return skipped_cargo_semver_evidence(
                request,
                "no previous release tag; cargo-semver-checks needs a baseline",
            );
        }
        if !target_matches_current_checkout(request.repo_root, request.target_ref) {
            return skipped_cargo_semver_evidence(
                request,
                "target ref does not resolve to the current checkout; cargo-semver-checks evidence would not match the selected release range",
            );
        }

        let command = cargo_semver_command(request.previous_tag);
        let output = match run_cmd(
            "cargo",
            ["semver-checks", "--baseline-rev", request.previous_tag],
            request.repo_root,
        ) {
            Ok(output) => output,
            Err(error) => {
                return VersionApiEvidence {
                    provider: "cargo-semver-checks".into(),
                    status: "failed".into(),
                    bump: "none".into(),
                    baseline: request.previous_tag.into(),
                    target: request.target_ref.into(),
                    command,
                    exit_code: -1,
                    summary: "cargo-semver-checks could not be started".into(),
                    findings: Vec::new(),
                    failure_message: sanitize_text(&error.to_string()),
                };
            }
        };

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");
        if output.status.success() {
            return VersionApiEvidence {
                provider: "cargo-semver-checks".into(),
                status: "passed".into(),
                bump: "none".into(),
                baseline: request.previous_tag.into(),
                target: request.target_ref.into(),
                command,
                exit_code,
                summary: "cargo-semver-checks passed; no public API breaking evidence found".into(),
                findings: output_excerpt(&combined),
                failure_message: String::new(),
            };
        }

        if looks_like_semver_violation(&combined) {
            VersionApiEvidence {
                provider: "cargo-semver-checks".into(),
                status: "findings".into(),
                bump: "major".into(),
                baseline: request.previous_tag.into(),
                target: request.target_ref.into(),
                command,
                exit_code,
                summary: "cargo-semver-checks reported public API SemVer violations; major bump evidence recorded".into(),
                findings: output_excerpt(&combined),
                failure_message: String::new(),
            }
        } else {
            VersionApiEvidence {
                provider: "cargo-semver-checks".into(),
                status: "failed".into(),
                bump: "none".into(),
                baseline: request.previous_tag.into(),
                target: request.target_ref.into(),
                command,
                exit_code,
                summary: "cargo-semver-checks failed before producing trusted SemVer findings"
                    .into(),
                findings: Vec::new(),
                failure_message: output_excerpt(&combined).join(" | "),
            }
        }
    }
}

fn skipped_cargo_semver_evidence(
    request: &VersionApiEvidenceRequest<'_>,
    reason: &str,
) -> VersionApiEvidence {
    VersionApiEvidence {
        provider: "cargo-semver-checks".into(),
        status: "skipped".into(),
        bump: "none".into(),
        baseline: request.previous_tag.into(),
        target: request.target_ref.into(),
        command: cargo_semver_command(request.previous_tag),
        exit_code: 0,
        summary: reason.into(),
        findings: Vec::new(),
        failure_message: String::new(),
    }
}

fn cargo_semver_command(previous_tag: &str) -> String {
    format!("cargo semver-checks --baseline-rev {previous_tag}")
}

fn target_matches_current_checkout(repo_root: &Path, target_ref: &str) -> bool {
    if target_ref == "HEAD" {
        return true;
    }
    let Ok(target) = git_commit(repo_root, target_ref) else {
        return false;
    };
    let Ok(head) = git_commit(repo_root, "HEAD") else {
        return false;
    };
    target == head
}

fn git_commit(repo_root: &Path, rev: &str) -> Result<String> {
    Ok(run_ok(
        "git",
        ["rev-parse", &format!("{rev}^{{commit}}")],
        repo_root,
    )?
    .trim()
    .to_string())
}

fn looks_like_semver_violation(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    lower.contains("breaking changes detected")
        || lower.contains("semver violation")
        || lower.contains("semver check failed")
        || lower.contains("required bump")
}

fn output_excerpt(output: &str) -> Vec<String> {
    output
        .lines()
        .map(sanitize_text)
        .filter(|line| !line.trim().is_empty())
        .take(12)
        .collect()
}
