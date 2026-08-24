use crate::*;

use super::*;
use std::sync::{Arc, Barrier};
use tempfile::tempdir;

fn prepared() -> ReleaseTransaction {
    let candidate = ReleaseCandidate {
        repository: "example/product".into(),
        source_revision: "a".repeat(40),
        previous_tag: "v1.0.0".into(),
        version: "1.1.0".into(),
        release_tag: "v1.1.0".into(),
        notes_sha256: format!("sha256:{}", "b".repeat(64)),
    };
    ReleaseTransaction {
        schema_version: TRANSACTION_SCHEMA.into(),
        transaction_id: identity_digest(&candidate).unwrap(),
        state: "prepared".into(),
        prepared_at: Utc::now().to_rfc3339(),
        candidate,
        required_artifact_roles: REQUIRED_ROLES.iter().map(|role| (*role).into()).collect(),
        artifacts: Vec::new(),
        artifact_set_sha256: None,
        verification: None,
        bound_at: None,
        receipt: None,
    }
}

fn artifacts(marker: char) -> Vec<ReleaseArtifact> {
    let image_digest = format!("sha256:{}", marker.to_string().repeat(64));
    vec![
        ReleaseArtifact {
            role: "oci_image".into(),
            name: "image".into(),
            media_type: "application/vnd.oci.image.index.v1+json".into(),
            digest: image_digest.clone(),
            path: "image.json".into(),
        },
        ReleaseArtifact {
            role: "release_manifest".into(),
            name: "manifest".into(),
            media_type: RELEASE_MANIFEST_MEDIA_TYPE.into(),
            digest: format!(
                "sha256:{}",
                ((marker as u8 + 1) as char).to_string().repeat(64)
            ),
            path: "manifest.json".into(),
        },
        ReleaseArtifact {
            role: "signature_bundle".into(),
            name: "bundle".into(),
            media_type: SIGSTORE_BUNDLE_MEDIA_TYPE.into(),
            digest: format!(
                "sha256:{}",
                ((marker as u8 + 2) as char).to_string().repeat(64)
            ),
            path: "bundle.json".into(),
        },
    ]
}

fn verification(artifacts: &[ReleaseArtifact]) -> ReleaseArtifactVerification {
    ReleaseArtifactVerification {
        method: "sigstore-key".into(),
        manifest_digest: artifacts[1].digest.clone(),
        signature_bundle_digest: artifacts[2].digest.clone(),
        verification_key_sha256: Some(format!("sha256:{}", "f".repeat(64))),
        certificate_identity: None,
        certificate_oidc_issuer: None,
        verified_at: Utc::now().to_rfc3339(),
    }
}

#[test]
fn crash_before_rename_preserves_canonical_state_and_cleans_temp() {
    let root = tempdir().unwrap();
    let path = root.path().join("transaction.json");
    let first = prepared();
    write_transaction_cas(&path, None, &first, InjectedCrash::None).unwrap();
    let original = fs::read(&path).unwrap();
    let mut replacement = first;
    replacement.state = "ready".into();
    assert!(
        write_transaction_cas(
            &path,
            Some(&original),
            &replacement,
            InjectedCrash::BeforeRename
        )
        .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn crash_after_rename_is_recoverable_from_new_canonical_state() {
    let root = tempdir().unwrap();
    let path = root.path().join("transaction.json");
    let first = prepared();
    write_transaction_cas(&path, None, &first, InjectedCrash::None).unwrap();
    let original = fs::read(&path).unwrap();
    let mut replacement = first;
    replacement.prepared_at = "2026-07-14T00:00:00Z".into();
    assert!(
        write_transaction_cas(
            &path,
            Some(&original),
            &replacement,
            InjectedCrash::AfterRename
        )
        .is_err()
    );
    assert_eq!(
        read_transaction(&path).unwrap().prepared_at,
        replacement.prepared_at
    );
}

#[test]
fn concurrent_different_binds_have_one_winner_and_reject_substitution() {
    let root = tempdir().unwrap();
    let path = root.path().join("transaction.json");
    let initial = prepared();
    let transaction_id = initial.transaction_id.clone();
    write_transaction_cas(&path, None, &initial, InjectedCrash::None).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let mut threads = Vec::new();
    for marker in ['a', 'd'] {
        let path = path.clone();
        let transaction_id = transaction_id.clone();
        let barrier = Arc::clone(&barrier);
        threads.push(std::thread::spawn(move || {
            let artifacts = artifacts(marker);
            let digest = identity_digest(&artifacts).unwrap();
            let verified = verification(&artifacts);
            barrier.wait();
            bind_verified_transaction(&path, transaction_id, artifacts, digest, verified)
                .map(|transaction| transaction.state)
                .map_err(|error| error.to_string())
        }));
    }
    barrier.wait();
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert!(
        results
            .iter()
            .filter_map(|result| result.as_ref().err())
            .any(|error| error.to_string().contains("substitution rejected"))
    );
    assert_eq!(read_transaction(&path).unwrap().state, "ready");
}

#[cfg(unix)]
#[test]
fn confined_artifact_path_rejects_symlinks() {
    use std::os::unix::fs::symlink;
    let root = tempdir().unwrap();
    let outside = tempdir().unwrap();
    fs::write(outside.path().join("artifact.json"), "{}").unwrap();
    symlink(outside.path(), root.path().join("escape")).unwrap();
    let root_handle = open_directory_path_nofollow(root.path(), "artifact root").unwrap();
    assert!(
        open_relative_regular_file_nofollow(&root_handle, Path::new("escape/artifact.json"))
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn concurrent_ancestor_swap_never_reads_outside_root() {
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicBool, Ordering};

    let root = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let safe = root.path().join("safe");
    let parked = root.path().join("parked");
    fs::create_dir_all(safe.join("sub")).unwrap();
    fs::create_dir_all(outside.path().join("sub")).unwrap();
    fs::write(safe.join("sub/artifact.json"), b"safe").unwrap();
    fs::write(outside.path().join("sub/artifact.json"), b"evil").unwrap();
    let root_handle = open_directory_path_nofollow(root.path(), "artifact root").unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_swapper = Arc::clone(&stop);
    let outside_path = outside.path().to_path_buf();
    let swapper = std::thread::spawn(move || {
        while !stop_swapper.load(Ordering::Relaxed) {
            if fs::rename(&safe, &parked).is_err() {
                continue;
            }
            let _ = symlink(&outside_path, &safe);
            let _ = fs::remove_file(&safe);
            let _ = fs::rename(&parked, &safe);
        }
        if fs::symlink_metadata(&safe).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            let _ = fs::remove_file(&safe);
        }
        if parked.exists() && !safe.exists() {
            let _ = fs::rename(&parked, &safe);
        }
    });
    for _ in 0..2_000 {
        if let Ok(file) =
            open_relative_regular_file_nofollow(&root_handle, Path::new("safe/sub/artifact.json"))
        {
            assert_eq!(read_bounded_file(file, "artifact").unwrap(), b"safe");
        }
    }
    stop.store(true, Ordering::Relaxed);
    swapper.join().unwrap();
}

#[test]
fn signed_publication_manifest_binds_candidate_and_oci_digest() {
    let transaction = prepared();
    let artifact = &artifacts('a')[0];
    let manifest = serde_json::json!({
        "schema_version": RELEASE_MANIFEST_SCHEMA,
        "transaction_id": transaction.transaction_id,
        "candidate": transaction.candidate,
        "oci": {
            "digest": artifact.digest,
            "media_type": artifact.media_type,
        }
    });
    validate_release_publication_manifest(
        &serde_json::to_vec(&manifest).unwrap(),
        &transaction,
        artifact,
    )
    .unwrap();

    let mut substituted = manifest;
    substituted["oci"]["digest"] = Value::String(format!("sha256:{}", "e".repeat(64)));
    assert!(
        validate_release_publication_manifest(
            &serde_json::to_vec(&substituted).unwrap(),
            &transaction,
            artifact,
        )
        .unwrap_err()
        .to_string()
        .contains("exact OCI artifact")
    );
}

#[cfg(unix)]
#[test]
fn canonical_transaction_rejects_symlink_target() {
    use std::os::unix::fs::symlink;
    let root = tempdir().unwrap();
    let outside = root.path().join("outside.json");
    fs::write(&outside, "{}").unwrap();
    let target = root.path().join("transaction.json");
    symlink(&outside, &target).unwrap();
    assert!(secure_state_path(&target, false).is_err());
}
