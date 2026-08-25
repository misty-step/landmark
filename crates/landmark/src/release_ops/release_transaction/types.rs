use crate::*;
pub(crate) const TRANSACTION_SCHEMA: &str = "landmark.release-transaction.v1";
pub(crate) const ARTIFACT_MANIFEST_SCHEMA: &str = "landmark.release-artifact-manifest.v1";
pub(crate) const RELEASE_MANIFEST_SCHEMA: &str = "landmark.release-publication-manifest.v1";
pub(crate) const RELEASE_MANIFEST_MEDIA_TYPE: &str =
    "application/vnd.landmark.release-publication-manifest.v1+json";
pub(crate) const REQUIRED_ROLES: [&str; 3] = ["oci_image", "release_manifest", "signature_bundle"];
pub(crate) const SIGSTORE_BUNDLE_MEDIA_TYPE: &str = "application/vnd.dev.sigstore.bundle.v0.3+json";
pub(crate) const MAX_LOCAL_ARTIFACT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReleaseTransaction {
    pub(crate) schema_version: String,
    pub(crate) transaction_id: String,
    pub(crate) state: String,
    pub(crate) prepared_at: String,
    pub(crate) candidate: ReleaseCandidate,
    pub(crate) required_artifact_roles: Vec<String>,
    pub(crate) artifacts: Vec<ReleaseArtifact>,
    pub(crate) artifact_set_sha256: Option<String>,
    pub(crate) verification: Option<ReleaseArtifactVerification>,
    pub(crate) bound_at: Option<String>,
    #[serde(default)]
    pub(crate) receipt: Option<ReleaseTransactionReceipt>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReleaseCandidate {
    pub(crate) repository: String,
    pub(crate) source_revision: String,
    pub(crate) previous_tag: String,
    pub(crate) version: String,
    pub(crate) release_tag: String,
    pub(crate) notes_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReleaseArtifactManifest {
    pub(crate) schema_version: String,
    pub(crate) transaction_id: String,
    pub(crate) artifacts: Vec<ReleaseArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReleaseArtifact {
    pub(crate) role: String,
    pub(crate) name: String,
    pub(crate) media_type: String,
    pub(crate) digest: String,
    pub(crate) path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReleasePublicationManifest {
    pub(crate) schema_version: String,
    pub(crate) transaction_id: String,
    pub(crate) candidate: ReleaseCandidate,
    pub(crate) oci: ReleasePublicationOci,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReleasePublicationOci {
    pub(crate) digest: String,
    pub(crate) media_type: String,
}

#[derive(Clone, Debug)]
pub(crate) struct RequestedVerificationPolicy {
    pub(crate) method: String,
    pub(crate) verification_key: Option<Vec<u8>>,
    pub(crate) verification_key_sha256: Option<String>,
    pub(crate) certificate_identity: Option<String>,
    pub(crate) certificate_oidc_issuer: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReleaseArtifactVerification {
    pub(crate) method: String,
    pub(crate) manifest_digest: String,
    pub(crate) signature_bundle_digest: String,
    pub(crate) verification_key_sha256: Option<String>,
    pub(crate) certificate_identity: Option<String>,
    pub(crate) certificate_oidc_issuer: Option<String>,
    pub(crate) verified_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InjectedCrash {
    None,
    BeforeRename,
    AfterRename,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReleaseTransactionReceipt {
    pub(crate) committed_at: String,
    pub(crate) release_id: u64,
    pub(crate) release_url: String,
    pub(crate) tag_name: String,
}
