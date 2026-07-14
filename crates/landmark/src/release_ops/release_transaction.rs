use crate::*;
use fs2::FileExt as Fs2FileExt;
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

const TRANSACTION_SCHEMA: &str = "landmark.release-transaction.v1";
const ARTIFACT_MANIFEST_SCHEMA: &str = "landmark.release-artifact-manifest.v1";
const RELEASE_MANIFEST_SCHEMA: &str = "landmark.release-publication-manifest.v1";
const RELEASE_MANIFEST_MEDIA_TYPE: &str =
    "application/vnd.landmark.release-publication-manifest.v1+json";
const REQUIRED_ROLES: [&str; 3] = ["oci_image", "release_manifest", "signature_bundle"];
const SIGSTORE_BUNDLE_MEDIA_TYPE: &str = "application/vnd.dev.sigstore.bundle.v0.3+json";
const MAX_LOCAL_ARTIFACT_BYTES: u64 = 16 * 1024 * 1024;

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
struct ReleasePublicationManifest {
    schema_version: String,
    transaction_id: String,
    candidate: ReleaseCandidate,
    oci: ReleasePublicationOci,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleasePublicationOci {
    digest: String,
    media_type: String,
}

#[derive(Clone, Debug)]
struct RequestedVerificationPolicy {
    method: String,
    verification_key: Option<Vec<u8>>,
    verification_key_sha256: Option<String>,
    certificate_identity: Option<String>,
    certificate_oidc_issuer: Option<String>,
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
enum InjectedCrash {
    None,
    BeforeRename,
    AfterRename,
}

pub(crate) fn release_transaction(args: ReleaseTransactionArgs) -> Result<()> {
    match args.command {
        ReleaseTransactionCommand::Prepare(args) => prepare_release_transaction(args),
        ReleaseTransactionCommand::Bind(args) => bind_release_transaction(args),
    }
}

pub(crate) fn prepare_release_transaction(args: PrepareReleaseTransactionArgs) -> Result<()> {
    let run_args = RunArgs {
        provider: "local".into(),
        repo_root: args.repo_root.clone(),
        repository: args.repository.clone(),
        release_tag: args.release_tag.clone(),
        previous_tag: args.previous_tag.clone(),
        github_token: String::new(),
        api_base_url: "https://api.github.com".into(),
        server_url: String::new(),
        publish_release_body: false,
        dry_run: true,
        notes_file: args.notes_file.clone(),
        output_dir: PathBuf::new(),
        technical_changelog_file: String::new(),
        evidence_file: String::new(),
        output_file: String::new(),
        output_text_file: String::new(),
        output_html_file: String::new(),
        output_json: String::new(),
        rss_feed_file: String::new(),
        rss_max_entries: 1,
    };
    let release = resolve_local_release(&run_args)?;
    if release.decision.bump == "none" && args.release_tag.trim().is_empty() {
        return Err(
            "no release-worthy changes were found; refusing to prepare an existing tag".into(),
        );
    }
    let manifest =
        load_manifest(&args.repo_root)?.unwrap_or_else(|| infer_manifest(&args.repo_root));
    let notes = if let Some(path) =
        run_output_path(&args.repo_root, &args.notes_file, &release.release_tag)
    {
        read_nonempty(&path)?
    } else {
        render_local_public_notes(&manifest, &release)
    };
    let repository = trimmed_option(&args.repository)
        .or_else(|| {
            args.repo_root
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "local".into());
    if repository.contains('/') {
        validate_repo(&repository)?;
    } else {
        validate_nonblank(&repository, "repository")?;
    }
    let source_revision = run_ok("git", ["rev-parse", "HEAD"], &args.repo_root)?
        .trim()
        .to_ascii_lowercase();
    let candidate = ReleaseCandidate {
        repository,
        source_revision,
        previous_tag: release.previous_tag,
        version: release.version,
        release_tag: release.release_tag,
        notes_sha256: format!("sha256:{}", sha256_hex(notes.as_bytes())),
    };
    validate_candidate(&candidate)?;
    let transaction = ReleaseTransaction {
        schema_version: TRANSACTION_SCHEMA.into(),
        transaction_id: identity_digest(&candidate)?,
        state: "prepared".into(),
        prepared_at: Utc::now().to_rfc3339(),
        candidate,
        required_artifact_roles: REQUIRED_ROLES.iter().map(|role| (*role).into()).collect(),
        artifacts: Vec::new(),
        artifact_set_sha256: None,
        verification: None,
        bound_at: None,
    };
    let transaction_path = secure_state_path(&args.transaction, true)?;
    let _lock = lock_transaction(&transaction_path)?;
    if transaction_path.exists() {
        let existing = read_transaction(&transaction_path)?;
        if existing.transaction_id != transaction.transaction_id
            || existing.candidate != transaction.candidate
        {
            return Err("canonical transaction already contains a different candidate".into());
        }
        return emit_transaction(&existing);
    }
    write_transaction_cas(&transaction_path, None, &transaction, InjectedCrash::None)?;
    emit_transaction(&transaction)
}

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
fn bind_verified_transaction(
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

fn bind_verified_transaction_locked(
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

fn verify_local_artifacts(
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

fn resolve_verification_policy(
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

fn ready_transaction_matches_request(
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

fn validate_release_publication_manifest(
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

fn run_staged_cosign_verification(
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

fn create_private_verification_workspace() -> Result<PathBuf> {
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

fn write_new_private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = secure_write_options()
        .create_new(true)
        .write(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn validate_transaction(transaction: &ReleaseTransaction) -> Result<()> {
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
        "ready" => {
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
                Err("ready transaction has inconsistent artifact binding".into())
            } else {
                Ok(())
            }
        }
        state => Err(format!("unsupported transaction state {state}").into()),
    }
}

fn valid_verification_policy(verification: &ReleaseArtifactVerification) -> bool {
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

fn validate_candidate(candidate: &ReleaseCandidate) -> Result<()> {
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

fn validate_artifacts(artifacts: &[ReleaseArtifact]) -> Result<()> {
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

fn validate_hex_digest(value: &str, name: &str, prefixed: bool) -> Result<()> {
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

fn validate_relative_path(path: &Path, name: &str) -> Result<()> {
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

fn secure_existing_directory(path: &Path, name: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{name} must be a real directory, not a symlink").into());
    }
    Ok(fs::canonicalize(path)?)
}

#[cfg(unix)]
fn openat_nofollow(directory: &File, name: &OsStr, directory_only: bool) -> Result<File> {
    let name = CString::new(name.as_bytes())?;
    let mut flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    if directory_only {
        flags |= libc::O_DIRECTORY;
    }
    let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(unix)]
fn open_directory_path_nofollow(path: &Path, name: &str) -> Result<File> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{name} must be a real directory, not a symlink").into());
    }
    let canonical = fs::canonicalize(path)?;
    let mut directory = if canonical.is_absolute() {
        File::open("/")?
    } else {
        File::open(".")?
    };
    for component in canonical.components() {
        match component {
            std::path::Component::RootDir | std::path::Component::CurDir => {}
            std::path::Component::Normal(part) => {
                directory = openat_nofollow(&directory, part, true)?;
            }
            _ => return Err(format!("{name} must not contain parent or prefix components").into()),
        }
    }
    if !directory.metadata()?.is_dir() {
        return Err(format!("{name} must be a directory").into());
    }
    Ok(directory)
}

#[cfg(unix)]
fn open_relative_regular_file_nofollow(root: &File, relative: &Path) -> Result<File> {
    validate_relative_path(relative, "artifact path")?;
    let mut directory = root.try_clone()?;
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let std::path::Component::Normal(part) = component else {
            unreachable!();
        };
        if components.peek().is_some() {
            directory = openat_nofollow(&directory, part, true)?;
        } else {
            let file = openat_nofollow(&directory, part, false)?;
            if !file.metadata()?.is_file() {
                return Err("artifact path must resolve to a regular file".into());
            }
            return Ok(file);
        }
    }
    Err("artifact path must name a file".into())
}

#[cfg(unix)]
fn open_regular_path_nofollow(path: &Path, name: &str) -> Result<File> {
    let filename = path
        .file_name()
        .ok_or_else(|| format!("{name} must name a file"))?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let directory = open_directory_path_nofollow(parent, name)?;
    let file = openat_nofollow(&directory, filename, false)?;
    if !file.metadata()?.is_file() {
        return Err(format!("{name} must be a regular file").into());
    }
    Ok(file)
}

#[cfg(not(unix))]
fn open_directory_path_nofollow(path: &Path, name: &str) -> Result<File> {
    let canonical = secure_existing_directory(path, name)?;
    Ok(File::open(canonical)?)
}

#[cfg(not(unix))]
fn open_relative_regular_file_nofollow(root: &File, relative: &Path) -> Result<File> {
    let _ = (root, relative);
    Err("release artifact binding requires fd-relative path traversal on Unix".into())
}

#[cfg(not(unix))]
fn open_regular_path_nofollow(path: &Path, name: &str) -> Result<File> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("{name} must be a real regular file, not a symlink").into());
    }
    Ok(File::open(path)?)
}

fn read_bounded_file(file: File, label: &str) -> Result<Vec<u8>> {
    let size = file.metadata()?.len();
    if size > MAX_LOCAL_ARTIFACT_BYTES {
        return Err(format!("local artifact {label} exceeds 16 MiB").into());
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(MAX_LOCAL_ARTIFACT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_LOCAL_ARTIFACT_BYTES {
        return Err(format!("local artifact {label} exceeds 16 MiB").into());
    }
    Ok(bytes)
}

fn secure_state_path(path: &Path, create_parent: bool) -> Result<PathBuf> {
    let filename = path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or("transaction path must name a file")?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    if create_parent {
        fs::create_dir_all(parent)?;
    }
    let parent = secure_existing_directory(parent, "transaction parent")?;
    let target = parent.join(filename);
    if let Ok(metadata) = fs::symlink_metadata(&target)
        && metadata.file_type().is_symlink()
    {
        return Err("transaction path must not be a symlink".into());
    }
    Ok(target)
}

fn lock_transaction(path: &Path) -> Result<fs::File> {
    let filename = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or("invalid transaction filename")?;
    let lock_path = path.with_file_name(format!(".{filename}.lock"));
    if let Ok(metadata) = fs::symlink_metadata(&lock_path)
        && metadata.file_type().is_symlink()
    {
        return Err("transaction lock path must not be a symlink".into());
    }
    let lock = secure_write_options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)?;
    lock.lock_exclusive()?;
    Ok(lock)
}

fn read_transaction(path: &Path) -> Result<ReleaseTransaction> {
    let transaction: ReleaseTransaction = serde_json::from_slice(&fs::read(path)?)?;
    validate_transaction(&transaction)?;
    Ok(transaction)
}

fn write_transaction_cas(
    path: &Path,
    expected: Option<&[u8]>,
    transaction: &ReleaseTransaction,
    crash: InjectedCrash,
) -> Result<()> {
    match expected {
        Some(expected) if fs::read(path)? != expected => {
            return Err("canonical transaction changed during compare-and-swap".into());
        }
        None if path.exists() => {
            return Err("canonical transaction appeared during compare-and-swap".into());
        }
        _ => {}
    }
    let bytes = serde_json::to_vec_pretty(transaction)?;
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random)?;
    let filename = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or("invalid transaction filename")?;
    let temporary = path.with_file_name(format!(".{filename}.{}.tmp", hex::encode(random)));
    let result = (|| -> Result<()> {
        let mut file = secure_write_options()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        if crash == InjectedCrash::BeforeRename {
            return Err("injected crash before rename".into());
        }
        fs::rename(&temporary, path)?;
        if crash == InjectedCrash::AfterRename {
            return Err("injected crash after rename".into());
        }
        fs::File::open(path.parent().ok_or("transaction path has no parent")?)?.sync_all()?;
        Ok(())
    })();
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn secure_write_options() -> fs::OpenOptions {
    let mut options = fs::OpenOptions::new();
    #[cfg(unix)]
    {
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    options
}

fn identity_digest<T: Serialize>(value: &T) -> Result<String> {
    Ok(format!(
        "sha256:{}",
        sha256_hex(&serde_json::to_vec(value)?)
    ))
}

fn emit_transaction(transaction: &ReleaseTransaction) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(transaction)?);
    Ok(())
}

#[cfg(test)]
mod tests {
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
            if let Ok(file) = open_relative_regular_file_nofollow(
                &root_handle,
                Path::new("safe/sub/artifact.json"),
            ) {
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
}

#[cfg(test)]
#[path = "release_transaction_policy_tests.rs"]
mod policy_tests;
