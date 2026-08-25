use crate::*;
pub(crate) fn validate_transaction(transaction: &ReleaseTransaction) -> Result<()> {
    if transaction.schema_version != TRANSACTION_SCHEMA {
        return Err(format!(
            "unsupported transaction schema {}",
            transaction.schema_version
        )
        .into());
    }
    validate_candidate(&transaction.candidate)?;
    if transaction.transaction_id != identity_digest(&transaction.candidate)? {
        return Err("transaction_id does not match immutable candidate fields".into());
    }
    let expected: Vec<String> = REQUIRED_ROLES.iter().map(|role| (*role).into()).collect();
    if transaction.required_artifact_roles != expected {
        return Err("required_artifact_roles does not match the v1 contract".into());
    }
    match transaction.state.as_str() {
        "prepared"
            if !transaction.artifacts.is_empty()
                || transaction.artifact_set_sha256.is_some()
                || transaction.verification.is_some()
                || transaction.bound_at.is_some() =>
        {
            Err("prepared transaction must not contain bound artifacts".into())
        }
        "prepared" => Ok(()),
        "ready" => validate_ready_binding(transaction).map(|_| ()),
        "completed" => {
            validate_ready_binding(transaction)?;
            let receipt = transaction
                .receipt
                .as_ref()
                .ok_or("completed transaction is missing its receipt")?;
            if receipt.tag_name != transaction.candidate.release_tag {
                return Err("receipt tag_name disagrees with the release candidate".into());
            }
            validate_nonblank(&receipt.release_url, "receipt release_url")?;
            if !receipt.release_url.starts_with("https://") {
                return Err("receipt release_url must be an https URL".into());
            }
            Ok(())
        }
        state => Err(format!("unsupported transaction state {state}").into()),
    }
}

fn validate_ready_binding(transaction: &ReleaseTransaction) -> Result<()> {
    validate_artifacts(&transaction.artifacts)?;
    let digest = identity_digest(&transaction.artifacts)?;
    let verification = transaction
        .verification
        .as_ref()
        .ok_or("ready transaction is missing verification evidence")?;
    if transaction.artifact_set_sha256.as_deref() != Some(&digest)
        || transaction.bound_at.is_none()
        || verification.manifest_digest != transaction.artifacts[1].digest
        || verification.signature_bundle_digest != transaction.artifacts[2].digest
        || !valid_verification_policy(verification)
    {
        return Err("ready transaction has inconsistent artifact binding".into());
    }
    Ok(())
}

pub(crate) fn valid_verification_policy(verification: &ReleaseArtifactVerification) -> bool {
    match verification.method.as_str() {
        "sigstore-key" => {
            verification
                .verification_key_sha256
                .as_deref()
                .is_some_and(|digest| validate_hex_digest(digest, "verification key", true).is_ok())
                && verification.certificate_identity.is_none()
                && verification.certificate_oidc_issuer.is_none()
        }
        "sigstore-keyless" => {
            verification.verification_key_sha256.is_none()
                && verification
                    .certificate_identity
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty())
                && verification
                    .certificate_oidc_issuer
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty())
        }
        _ => false,
    }
}

pub(crate) fn validate_candidate(candidate: &ReleaseCandidate) -> Result<()> {
    validate_nonblank(&candidate.repository, "repository")?;
    validate_hex_digest(&candidate.source_revision, "source_revision", false)?;
    if normalize_version(&candidate.version)? != candidate.version {
        return Err("candidate version must be normalized semver".into());
    }
    let parsed_tag = backfill_parse_tag(&candidate.release_tag)
        .ok_or_else(|| format!("invalid release_tag {}", candidate.release_tag))?;
    if parsed_tag.version != candidate.version {
        return Err("release_tag and version disagree".into());
    }
    validate_hex_digest(&candidate.notes_sha256, "notes_sha256", true)
}

pub(crate) fn validate_artifacts(artifacts: &[ReleaseArtifact]) -> Result<()> {
    let roles: Vec<&str> = artifacts
        .iter()
        .map(|artifact| artifact.role.as_str())
        .collect();
    if roles != REQUIRED_ROLES {
        return Err(format!(
            "artifact roles must be exactly {}",
            REQUIRED_ROLES.join(", ")
        )
        .into());
    }
    let mut digests = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for artifact in artifacts {
        validate_nonblank(&artifact.name, "artifact name")?;
        validate_nonblank(&artifact.media_type, "artifact media_type")?;
        validate_nonblank(&artifact.path, "artifact path")?;
        validate_hex_digest(&artifact.digest, "artifact digest", true)?;
        validate_relative_path(Path::new(&artifact.path), "artifact path")?;
        if !digests.insert(artifact.digest.clone()) || !paths.insert(artifact.path.clone()) {
            return Err("artifact digests and paths must be unique".into());
        }
    }
    let image = &artifacts[0];
    if !matches!(
        image.media_type.as_str(),
        "application/vnd.oci.image.manifest.v1+json" | "application/vnd.oci.image.index.v1+json"
    ) {
        return Err("oci_image must use an OCI image manifest or index media type".into());
    }
    if artifacts[1].media_type != RELEASE_MANIFEST_MEDIA_TYPE {
        return Err(format!("release_manifest must use {RELEASE_MANIFEST_MEDIA_TYPE}").into());
    }
    if artifacts[2].media_type != SIGSTORE_BUNDLE_MEDIA_TYPE {
        return Err(format!("signature_bundle must use {SIGSTORE_BUNDLE_MEDIA_TYPE}").into());
    }
    Ok(())
}

pub(crate) fn validate_hex_digest(value: &str, name: &str, prefixed: bool) -> Result<()> {
    let hex = if prefixed {
        value
            .strip_prefix("sha256:")
            .ok_or_else(|| format!("{name} must use a sha256: prefix"))?
    } else {
        value
    };
    if hex.len() != 64 && !(name == "source_revision" && hex.len() == 40) {
        return Err(format!("{name} must contain a SHA-256 or Git SHA-1 digest").into());
    }
    if !hex
        .chars()
        .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
    {
        return Err(format!("{name} must contain lowercase hex only").into());
    }
    Ok(())
}

pub(crate) fn validate_relative_path(path: &Path, name: &str) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err(format!("{name} must be a normalized relative path").into());
    }
    Ok(())
}
