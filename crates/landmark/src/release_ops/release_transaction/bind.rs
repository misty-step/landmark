use crate::*;
use std::fs::File;
#[cfg(unix)]
use std::os::unix::fs::DirBuilderExt;

pub(crate) fn bind_release_transaction(args: BindReleaseTransactionArgs) -> Result<()> {
    let transaction_path = secure_state_path(&args.transaction, false)?;
    let artifact_root = open_directory_path_nofollow(&args.artifact_root, "artifact root")?;
    let mut manifest: ReleaseArtifactManifest =
        serde_json::from_str(&fs::read_to_string(&args.artifact_manifest)?)?;
    if manifest.schema_version != ARTIFACT_MANIFEST_SCHEMA {
        return Err(format!(
            "unsupported artifact manifest schema {}",
            manifest.schema_version
        )
        .into());
    }
    manifest
        .artifacts
        .sort_by(|left, right| left.role.cmp(&right.role));
    let _lock = lock_transaction(&transaction_path)?;
    let original = fs::read(&transaction_path)?;
    let transaction: ReleaseTransaction = serde_json::from_slice(&original)?;
    validate_transaction(&transaction)?;
    if manifest.transaction_id != transaction.transaction_id {
        return Err("artifact manifest transaction_id does not match canonical transaction".into());
    }
    validate_artifacts(&manifest.artifacts)?;
    let artifact_set_sha256 = identity_digest(&manifest.artifacts)?;
    let policy = resolve_verification_policy(&args)?;
    if ready_transaction_matches_request(
        &transaction,
        &manifest.artifacts,
        &artifact_set_sha256,
        &policy,
    )? {
        return emit_transaction(&transaction);
    }
    let verified = verify_local_artifacts(
        &artifact_root,
        &manifest.artifacts,
        &args,
        &transaction,
        &policy,
    )?;
    let transaction = bind_verified_transaction_locked(
        &transaction_path,
        original,
        transaction,
        manifest.artifacts,
        artifact_set_sha256,
        verified,
    )?;
    emit_transaction(&transaction)
}

#[cfg(test)]
pub(crate) fn bind_verified_transaction(
    transaction_path: &Path,
    manifest_transaction_id: String,
    artifacts: Vec<ReleaseArtifact>,
    artifact_set_sha256: String,
    verified: ReleaseArtifactVerification,
) -> Result<ReleaseTransaction> {
    let _lock = lock_transaction(transaction_path)?;
    let original = fs::read(transaction_path)?;
    let transaction: ReleaseTransaction = serde_json::from_slice(&original)?;
    validate_transaction(&transaction)?;
    if manifest_transaction_id != transaction.transaction_id {
        return Err("artifact manifest transaction_id does not match canonical transaction".into());
    }
    bind_verified_transaction_locked(
        transaction_path,
        original,
        transaction,
        artifacts,
        artifact_set_sha256,
        verified,
    )
}

pub(crate) fn bind_verified_transaction_locked(
    transaction_path: &Path,
    original: Vec<u8>,
    mut transaction: ReleaseTransaction,
    artifacts: Vec<ReleaseArtifact>,
    artifact_set_sha256: String,
    verified: ReleaseArtifactVerification,
) -> Result<ReleaseTransaction> {
    if transaction.state == "ready" {
        if transaction.artifacts != artifacts
            || transaction.artifact_set_sha256.as_deref() != Some(&artifact_set_sha256)
        {
            return Err("artifact substitution rejected for an already-bound transaction".into());
        }
        return Ok(transaction);
    }
    transaction.state = "ready".into();
    transaction.artifacts = artifacts;
    transaction.artifact_set_sha256 = Some(artifact_set_sha256);
    transaction.verification = Some(verified);
    transaction.bound_at = Some(Utc::now().to_rfc3339());
    write_transaction_cas(
        transaction_path,
        Some(&original),
        &transaction,
        InjectedCrash::None,
    )?;
    Ok(transaction)
}

pub(crate) fn verify_local_artifacts(
    root: &File,
    artifacts: &[ReleaseArtifact],
    args: &BindReleaseTransactionArgs,
    transaction: &ReleaseTransaction,
    policy: &RequestedVerificationPolicy,
) -> Result<ReleaseArtifactVerification> {
    validate_artifacts(artifacts)?;
    let mut bytes_by_role = BTreeMap::new();
    for artifact in artifacts {
        let file = open_relative_regular_file_nofollow(root, Path::new(&artifact.path))?;
        let bytes = read_bounded_file(file, &artifact.path)?;
        let actual = format!("sha256:{}", sha256_hex(&bytes));
        if actual != artifact.digest {
            return Err(format!(
                "{} digest does not match local artifact bytes",
                artifact.role
            )
            .into());
        }
        if artifact.role == "oci_image" {
            let descriptor: Value = serde_json::from_slice(&bytes)?;
            if descriptor["mediaType"].as_str() != Some(&artifact.media_type) {
                return Err("OCI descriptor mediaType does not match the artifact manifest".into());
            }
        } else if artifact.role == "release_manifest" {
            validate_release_publication_manifest(&bytes, transaction, &artifacts[0])?;
        } else if artifact.role == "signature_bundle" {
            let bundle: Value = serde_json::from_slice(&bytes)?;
            if bundle["mediaType"].as_str() != Some(SIGSTORE_BUNDLE_MEDIA_TYPE) {
                return Err(format!(
                    "signature bundle document must declare {SIGSTORE_BUNDLE_MEDIA_TYPE}"
                )
                .into());
            }
        }
        bytes_by_role.insert(artifact.role.as_str(), bytes);
    }
    let output = run_staged_cosign_verification(
        &args.cosign,
        &bytes_by_role["release_manifest"],
        &bytes_by_role["signature_bundle"],
        policy.verification_key.as_deref(),
        policy.certificate_identity.as_deref().unwrap_or_default(),
        policy
            .certificate_oidc_issuer
            .as_deref()
            .unwrap_or_default(),
    )?;
    if !output.status.success() {
        return Err("Sigstore verification failed for the local release manifest".into());
    }
    Ok(ReleaseArtifactVerification {
        method: policy.method.clone(),
        manifest_digest: artifacts[1].digest.clone(),
        signature_bundle_digest: artifacts[2].digest.clone(),
        verification_key_sha256: policy.verification_key_sha256.clone(),
        certificate_identity: policy.certificate_identity.clone(),
        certificate_oidc_issuer: policy.certificate_oidc_issuer.clone(),
        verified_at: Utc::now().to_rfc3339(),
    })
}

pub(crate) fn resolve_verification_policy(
    args: &BindReleaseTransactionArgs,
) -> Result<RequestedVerificationPolicy> {
    if let Some(key) = &args.verification_key {
        if !args.certificate_identity.trim().is_empty()
            || !args.certificate_oidc_issuer.trim().is_empty()
        {
            return Err(
                "verification-key and keyless certificate policy are mutually exclusive".into(),
            );
        }
        let key = open_regular_path_nofollow(key, "verification key")?;
        let bytes = read_bounded_file(key, "verification key")?;
        Ok(RequestedVerificationPolicy {
            method: "sigstore-key".into(),
            verification_key_sha256: Some(format!("sha256:{}", sha256_hex(&bytes))),
            verification_key: Some(bytes),
            certificate_identity: None,
            certificate_oidc_issuer: None,
        })
    } else {
        validate_nonblank(&args.certificate_identity, "certificate-identity")?;
        validate_nonblank(&args.certificate_oidc_issuer, "certificate-oidc-issuer")?;
        Ok(RequestedVerificationPolicy {
            method: "sigstore-keyless".into(),
            verification_key: None,
            verification_key_sha256: None,
            certificate_identity: Some(args.certificate_identity.clone()),
            certificate_oidc_issuer: Some(args.certificate_oidc_issuer.clone()),
        })
    }
}

pub(crate) fn ready_transaction_matches_request(
    transaction: &ReleaseTransaction,
    artifacts: &[ReleaseArtifact],
    artifact_set_sha256: &str,
    policy: &RequestedVerificationPolicy,
) -> Result<bool> {
    if transaction.state != "ready" {
        return Ok(false);
    }
    if transaction.artifacts != artifacts
        || transaction.artifact_set_sha256.as_deref() != Some(artifact_set_sha256)
    {
        return Err("artifact substitution rejected for an already-bound transaction".into());
    }
    let stored = transaction
        .verification
        .as_ref()
        .ok_or("ready transaction is missing verification evidence")?;
    if stored.method != policy.method
        || stored.verification_key_sha256 != policy.verification_key_sha256
        || stored.certificate_identity != policy.certificate_identity
        || stored.certificate_oidc_issuer != policy.certificate_oidc_issuer
    {
        return Err(
            "requested verification policy does not match the canonical ready transaction".into(),
        );
    }
    Ok(true)
}

pub(crate) fn validate_release_publication_manifest(
    bytes: &[u8],
    transaction: &ReleaseTransaction,
    oci_artifact: &ReleaseArtifact,
) -> Result<()> {
    let manifest: ReleasePublicationManifest = serde_json::from_slice(bytes)?;
    if manifest.schema_version != RELEASE_MANIFEST_SCHEMA {
        return Err(format!(
            "unsupported release publication manifest schema {}",
            manifest.schema_version
        )
        .into());
    }
    if manifest.transaction_id != transaction.transaction_id {
        return Err(
            "signed release manifest transaction_id does not match the canonical transaction"
                .into(),
        );
    }
    if manifest.candidate != transaction.candidate {
        return Err(
            "signed release manifest candidate does not match the canonical transaction".into(),
        );
    }
    if manifest.oci.digest != oci_artifact.digest
        || manifest.oci.media_type != oci_artifact.media_type
    {
        return Err("signed release manifest does not bind the exact OCI artifact".into());
    }
    Ok(())
}

pub(crate) fn run_staged_cosign_verification(
    cosign: &Path,
    manifest: &[u8],
    bundle: &[u8],
    verification_key: Option<&[u8]>,
    certificate_identity: &str,
    certificate_oidc_issuer: &str,
) -> Result<Output> {
    let workspace = create_private_verification_workspace()?;
    let result = (|| -> Result<Output> {
        let manifest_path = workspace.join("release-manifest.json");
        let bundle_path = workspace.join("signature-bundle.json");
        write_new_private_file(&manifest_path, manifest)?;
        write_new_private_file(&bundle_path, bundle)?;
        let mut command = Command::new(cosign);
        command.arg("verify-blob").arg("--bundle").arg(&bundle_path);
        if let Some(key) = verification_key {
            let key_path = workspace.join("verification-key.pub");
            write_new_private_file(&key_path, key)?;
            command
                .arg("--insecure-ignore-tlog")
                .arg("--key")
                .arg(key_path);
        } else {
            command
                .arg("--certificate-identity")
                .arg(certificate_identity)
                .arg("--certificate-oidc-issuer")
                .arg(certificate_oidc_issuer);
        }
        fs::File::open(&workspace)?.sync_all()?;
        command.arg(manifest_path);
        command.output().map_err(|_| {
            "failed to execute cosign verifier; install cosign or pass --cosign explicitly".into()
        })
    })();
    let _ = fs::remove_dir_all(&workspace);
    result
}

pub(crate) fn create_private_verification_workspace() -> Result<PathBuf> {
    for _ in 0..4 {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random)?;
        let path = env::temp_dir().join(format!("landmark-verify-{}", hex::encode(random)));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        builder.mode(0o700);
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err("could not allocate a private verification workspace".into())
}

pub(crate) fn write_new_private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = secure_write_options()
        .create_new(true)
        .write(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
