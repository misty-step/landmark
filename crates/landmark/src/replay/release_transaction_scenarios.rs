use crate::*;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub(crate) fn scenario_release_transaction_prepare_and_bind(tmp_root: &Path) -> Result<Value> {
    let repo = tmp_root.join("release-transaction");
    init_fixture_repo(&repo, "v1.0.0")?;
    fs::write(repo.join("feature.txt"), "portable release\n")?;
    run_ok("git", ["add", "feature.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat: add portable release"],
        &repo,
    )?;
    let tags_before = git_tags(&repo)?;
    let transaction_path = repo.join("transaction.json");
    let prepare_args = [
        "release-transaction",
        "prepare",
        "--repo-root",
        repo.to_str().unwrap(),
        "--repository",
        "example/product",
        "--transaction",
        transaction_path.to_str().unwrap(),
    ];
    let prepared = Command::new(current_exe()).args(prepare_args).output()?;
    if !prepared.status.success() {
        return Err(format!(
            "release transaction prepare failed: {}",
            String::from_utf8_lossy(&prepared.stderr)
        )
        .into());
    }
    let prepared_json: Value = serde_json::from_slice(&prepared.stdout)?;
    assert_json_eq(&prepared_json, "/state", "prepared", "prepared state")?;
    assert_json_eq(
        &prepared_json,
        "/candidate/release_tag",
        "v1.1.0",
        "computed release tag",
    )?;
    let same = Command::new(current_exe()).args(prepare_args).output()?;
    if !same.status.success() || serde_json::from_slice::<Value>(&same.stdout)? != prepared_json {
        return Err("same-candidate prepare did not return canonical state idempotently".into());
    }
    fs::write(repo.join("other.txt"), "different candidate\n")?;
    run_ok("git", ["add", "other.txt"], &repo)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "fix: change candidate"],
        &repo,
    )?;
    let different = Command::new(current_exe()).args(prepare_args).output()?;
    if different.status.success()
        || !String::from_utf8_lossy(&different.stderr).contains("different candidate")
    {
        return Err("different-candidate prepare did not fail closed".into());
    }
    if git_tags(&repo)? != tags_before {
        return Err("release transaction prepare mutated git tags".into());
    }

    let artifact_root = repo.join("local-artifacts");
    fs::create_dir_all(&artifact_root)?;
    let image = br#"{"mediaType":"application/vnd.oci.image.index.v1+json","schemaVersion":2,"manifests":[]}"#;
    let image_digest = format!("sha256:{}", sha256_hex(image));
    let release_manifest = serde_json::to_vec_pretty(&json!({
        "schema_version": "landmark.release-publication-manifest.v1",
        "transaction_id": prepared_json["transaction_id"],
        "candidate": prepared_json["candidate"],
        "oci": {
            "digest": image_digest,
            "media_type": "application/vnd.oci.image.index.v1+json"
        }
    }))?;
    fs::write(artifact_root.join("image-index.json"), image)?;
    fs::write(
        artifact_root.join("release-manifest.json"),
        &release_manifest,
    )?;

    let private_key = repo.join("fixture-private.pem");
    let public_key = repo.join("fixture-public.pem");
    let signature = repo.join("fixture-signature.bin");
    run_ok(
        "openssl",
        [
            "genpkey",
            "-algorithm",
            "RSA",
            "-pkeyopt",
            "rsa_keygen_bits:2048",
            "-out",
            private_key.to_str().unwrap(),
        ],
        &repo,
    )?;
    run_ok(
        "openssl",
        [
            "pkey",
            "-in",
            private_key.to_str().unwrap(),
            "-pubout",
            "-out",
            public_key.to_str().unwrap(),
        ],
        &repo,
    )?;
    run_ok(
        "openssl",
        [
            "dgst",
            "-sha256",
            "-sign",
            private_key.to_str().unwrap(),
            "-out",
            signature.to_str().unwrap(),
            artifact_root
                .join("release-manifest.json")
                .to_str()
                .unwrap(),
        ],
        &repo,
    )?;
    let signature_b64 = run_ok(
        "openssl",
        ["base64", "-A", "-in", signature.to_str().unwrap()],
        &repo,
    )?;
    let bundle = serde_json::to_vec_pretty(&json!({
        "mediaType": "application/vnd.dev.sigstore.bundle.v0.3+json",
        "fixtureSignature": signature_b64.trim()
    }))?;
    fs::write(
        artifact_root.join("release-manifest.sigstore.json"),
        &bundle,
    )?;
    let verifier = repo.join("fixture-cosign");
    fs::write(
        &verifier,
        r#"#!/usr/bin/env bash
set -euo pipefail
test "$1" = verify-blob
shift
bundle=""
key=""
manifest=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --bundle) bundle="$2"; shift 2 ;;
    --key) key="$2"; shift 2 ;;
    --insecure-ignore-tlog) shift ;;
    *) manifest="$1"; shift ;;
  esac
done
test -n "$bundle" && test -n "$key" && test -n "$manifest"
sig="$(mktemp)"
trap 'rm -f "$sig"' EXIT
jq -r .fixtureSignature "$bundle" | openssl base64 -d -A -out "$sig"
openssl dgst -sha256 -verify "$key" -signature "$sig" "$manifest" >/dev/null
"#,
    )?;
    #[cfg(unix)]
    fs::set_permissions(&verifier, fs::Permissions::from_mode(0o700))?;

    let manifest_digest = format!("sha256:{}", sha256_hex(&release_manifest));
    let bundle_digest = format!("sha256:{}", sha256_hex(&bundle));
    let artifact_manifest = json!({
        "schema_version": "landmark.release-artifact-manifest.v1",
        "transaction_id": prepared_json["transaction_id"],
        "artifacts": [
            {
                "role": "signature_bundle",
                "name": "release-manifest.sigstore.json",
                "media_type": "application/vnd.dev.sigstore.bundle.v0.3+json",
                "digest": bundle_digest,
                "path": "release-manifest.sigstore.json"
            },
            {
                "role": "oci_image",
                "name": "product-image",
                "media_type": "application/vnd.oci.image.index.v1+json",
                "digest": image_digest,
                "path": "image-index.json"
            },
            {
                "role": "release_manifest",
                "name": "release-manifest.json",
                "media_type": "application/vnd.landmark.release-publication-manifest.v1+json",
                "digest": manifest_digest,
                "path": "release-manifest.json"
            }
        ]
    });
    let artifact_manifest_path = repo.join("artifacts.json");
    fs::write(
        &artifact_manifest_path,
        serde_json::to_string_pretty(&artifact_manifest)? + "\n",
    )?;
    let bind_args = [
        "release-transaction",
        "bind",
        "--transaction",
        transaction_path.to_str().unwrap(),
        "--artifact-manifest",
        artifact_manifest_path.to_str().unwrap(),
        "--artifact-root",
        artifact_root.to_str().unwrap(),
        "--cosign",
        verifier.to_str().unwrap(),
        "--verification-key",
        public_key.to_str().unwrap(),
    ];
    let bound = Command::new(current_exe()).args(bind_args).output()?;
    if !bound.status.success() {
        return Err(format!(
            "release transaction bind failed: {}",
            String::from_utf8_lossy(&bound.stderr)
        )
        .into());
    }
    let ready: Value = serde_json::from_slice(&bound.stdout)?;
    assert_json_eq(&ready, "/state", "ready", "bound state")?;
    assert_json_eq(
        &ready,
        "/verification/method",
        "sigstore-key",
        "verification method",
    )?;
    let retry = Command::new(current_exe()).args(bind_args).output()?;
    if !retry.status.success() || serde_json::from_slice::<Value>(&retry.stdout)? != ready {
        return Err("identical bind retry did not return the same canonical packet".into());
    }

    let alternate_image = br#"{"mediaType":"application/vnd.oci.image.index.v1+json","schemaVersion":2,"manifests":[{}]}"#;
    fs::write(artifact_root.join("alternate-index.json"), alternate_image)?;
    let alternate_digest = format!("sha256:{}", sha256_hex(alternate_image));
    let mut substituted = artifact_manifest;
    let image = substituted["artifacts"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|artifact| artifact["role"] == "oci_image")
        .unwrap();
    image["digest"] = json!(alternate_digest);
    image["path"] = json!("alternate-index.json");
    let substituted_path = repo.join("substituted.json");
    fs::write(
        &substituted_path,
        serde_json::to_string_pretty(&substituted)? + "\n",
    )?;
    let mut substituted_args = bind_args;
    substituted_args[5] = substituted_path.to_str().unwrap();
    let rejected = Command::new(current_exe())
        .args(substituted_args)
        .output()?;
    if rejected.status.success()
        || !String::from_utf8_lossy(&rejected.stderr).contains("substitution rejected")
    {
        return Err("artifact substitution was not rejected".into());
    }
    if git_tags(&repo)? != tags_before {
        return Err("release transaction bind mutated git tags".into());
    }

    Ok(json!({
        "transaction_id": ready["transaction_id"],
        "artifact_set_sha256": ready["artifact_set_sha256"],
        "release_tag": ready["candidate"]["release_tag"],
        "remote_mutations": 0,
        "same_candidate_prepare_idempotent": true,
        "different_candidate_rejected": true,
        "local_digests_recomputed": true,
        "verifier_adapter_exercised": true,
        "idempotent_retry": true,
        "substitution_rejected": true
    }))
}
