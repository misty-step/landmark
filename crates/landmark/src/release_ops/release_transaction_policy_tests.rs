use super::*;
use tempfile::tempdir;

fn args(key: Option<PathBuf>, identity: &str, issuer: &str) -> BindReleaseTransactionArgs {
    BindReleaseTransactionArgs {
        transaction: "transaction.json".into(),
        artifact_manifest: "artifacts.json".into(),
        artifact_root: "artifacts".into(),
        cosign: "cosign".into(),
        verification_key: key,
        certificate_identity: identity.into(),
        certificate_oidc_issuer: issuer.into(),
    }
}

fn ready(
    policy: &RequestedVerificationPolicy,
) -> (ReleaseTransaction, Vec<ReleaseArtifact>, String) {
    let artifacts = vec![
        ReleaseArtifact {
            role: "oci_image".into(),
            name: "image".into(),
            media_type: "application/vnd.oci.image.index.v1+json".into(),
            digest: format!("sha256:{}", "a".repeat(64)),
            path: "image.json".into(),
        },
        ReleaseArtifact {
            role: "release_manifest".into(),
            name: "manifest".into(),
            media_type: RELEASE_MANIFEST_MEDIA_TYPE.into(),
            digest: format!("sha256:{}", "b".repeat(64)),
            path: "manifest.json".into(),
        },
        ReleaseArtifact {
            role: "signature_bundle".into(),
            name: "bundle".into(),
            media_type: SIGSTORE_BUNDLE_MEDIA_TYPE.into(),
            digest: format!("sha256:{}", "c".repeat(64)),
            path: "bundle.json".into(),
        },
    ];
    let digest = identity_digest(&artifacts).unwrap();
    let candidate = ReleaseCandidate {
        repository: "example/product".into(),
        source_revision: "a".repeat(40),
        previous_tag: "v1.0.0".into(),
        version: "1.1.0".into(),
        release_tag: "v1.1.0".into(),
        notes_sha256: format!("sha256:{}", "d".repeat(64)),
    };
    let transaction = ReleaseTransaction {
        schema_version: TRANSACTION_SCHEMA.into(),
        transaction_id: identity_digest(&candidate).unwrap(),
        state: "ready".into(),
        prepared_at: "2026-07-14T00:00:00Z".into(),
        candidate,
        required_artifact_roles: REQUIRED_ROLES.iter().map(|role| (*role).into()).collect(),
        artifacts: artifacts.clone(),
        artifact_set_sha256: Some(digest.clone()),
        verification: Some(ReleaseArtifactVerification {
            method: policy.method.clone(),
            manifest_digest: artifacts[1].digest.clone(),
            signature_bundle_digest: artifacts[2].digest.clone(),
            verification_key_sha256: policy.verification_key_sha256.clone(),
            certificate_identity: policy.certificate_identity.clone(),
            certificate_oidc_issuer: policy.certificate_oidc_issuer.clone(),
            verified_at: "2026-07-14T00:00:01Z".into(),
        }),
        bound_at: Some("2026-07-14T00:00:01Z".into()),
    };
    (transaction, artifacts, digest)
}

#[test]
fn ready_retry_rejects_nonexistent_or_different_key() {
    let root = tempdir().unwrap();
    let key_a = root.path().join("key-a.pub");
    let key_b = root.path().join("key-b.pub");
    fs::write(&key_a, b"trusted key A").unwrap();
    fs::write(&key_b, b"trusted key B").unwrap();
    let policy_a = resolve_verification_policy(&args(Some(key_a), "", "")).unwrap();
    let (transaction, artifacts, digest) = ready(&policy_a);
    assert!(
        ready_transaction_matches_request(&transaction, &artifacts, &digest, &policy_a).unwrap()
    );

    let missing = root.path().join("missing.pub");
    assert!(resolve_verification_policy(&args(Some(missing), "", "")).is_err());
    let policy_b = resolve_verification_policy(&args(Some(key_b), "", "")).unwrap();
    assert!(
        ready_transaction_matches_request(&transaction, &artifacts, &digest, &policy_b)
            .unwrap_err()
            .to_string()
            .contains("verification policy")
    );
}

#[test]
fn ready_retry_rejects_keyless_identity_or_issuer_mismatch() {
    let policy =
        resolve_verification_policy(&args(None, "release@example.com", "https://issuer.example"))
            .unwrap();
    let (transaction, artifacts, digest) = ready(&policy);
    for changed in [
        args(None, "other@example.com", "https://issuer.example"),
        args(None, "release@example.com", "https://other-issuer.example"),
    ] {
        let changed = resolve_verification_policy(&changed).unwrap();
        assert!(
            ready_transaction_matches_request(&transaction, &artifacts, &digest, &changed)
                .unwrap_err()
                .to_string()
                .contains("verification policy")
        );
    }
}
