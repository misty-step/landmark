use crate::*;

/// Observed public release state for a candidate, before any mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RemoteReleaseState {
    /// Neither the tag nor a release record exists.
    Absent,
    /// The tag exists at this revision; no release record yet.
    TagAt(String),
    /// A release record already exists for the tag.
    Present {
        draft: bool,
        /// Creation input only; never treated as authoritative identity.
        target_commitish: String,
        html_url: String,
        release_id: u64,
        /// Authoritative tag identity resolved from the git ref API.
        tag_sha: Option<String>,
    },
}

/// Mutation decision derived from observed state. Pure and unit-testable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CommitPlan {
    /// Create the release from the candidate source revision.
    Create,
    /// Create a release record for an existing tag that matches the candidate.
    AdoptExistingTag,
    /// Release is already public and consistent with the candidate.
    Reconcile { release_id: u64, html_url: String },
}

pub(crate) fn is_full_hex(value: &str) -> bool {
    value.len() == 40 || value.len() == 64
}

/// The immutable publication identity shared by transaction commits and
/// self-release publication: one release, one source revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReleaseIdentity {
    pub(crate) release_tag: String,
    pub(crate) source_revision: String,
}

impl From<&ReleaseCandidate> for ReleaseIdentity {
    fn from(candidate: &ReleaseCandidate) -> Self {
        Self {
            release_tag: candidate.release_tag.clone(),
            source_revision: candidate.source_revision.clone(),
        }
    }
}

pub(crate) fn plan_publication(
    identity: &ReleaseIdentity,
    observed: &RemoteReleaseState,
) -> Result<CommitPlan> {
    let tag = &identity.release_tag;
    let revision = &identity.source_revision;
    match observed {
        RemoteReleaseState::Absent => Ok(CommitPlan::Create),
        RemoteReleaseState::TagAt(sha) => {
            if sha != revision {
                return Err(format!(
                    "tag {tag} exists at revision {sha} but the candidate binds {revision}; refusing to publish a mismatched release",
                )
                .into());
            }
            Ok(CommitPlan::AdoptExistingTag)
        }
        RemoteReleaseState::Present {
            draft,
            target_commitish,
            html_url,
            release_id,
            tag_sha,
        } => {
            if *draft {
                return Err(format!(
                    "draft release {tag} blocks committing the transaction; publish or delete it first",
                )
                .into());
            }
            let Some(sha) = tag_sha else {
                return Err(format!(
                    "existing release {tag} has no resolvable tag; refusing to adopt an unanchored release",
                )
                .into());
            };
            if sha != revision {
                return Err(format!(
                    "tag {tag} moved to revision {sha} but the candidate binds {revision}; refusing to adopt a mismatched release",
                )
                .into());
            }
            if is_full_hex(target_commitish) && target_commitish != revision {
                return Err(format!(
                    "existing release {tag} targets revision {target_commitish} but the candidate binds {revision}; refusing to adopt a mismatched release",
                )
                .into());
            }
            Ok(CommitPlan::Reconcile {
                release_id: *release_id,
                html_url: html_url.clone(),
            })
        }
    }
}

pub(crate) fn plan_commit(
    candidate: &ReleaseCandidate,
    observed: &RemoteReleaseState,
) -> Result<CommitPlan> {
    plan_publication(&ReleaseIdentity::from(candidate), observed)
}

/// Resolves the tag ref first so every observation carries authoritative tag
/// identity; a release record alone never proves where the tag points.
pub(crate) fn observe_release_state(
    provider: &GitHubProvider,
    repository: &str,
    release_tag: &str,
) -> Result<RemoteReleaseState> {
    let tag_sha = provider.tag_ref(repository, release_tag)?;
    let Some(release) = provider.release_by_tag(repository, release_tag)? else {
        return Ok(match tag_sha {
            Some(sha) => RemoteReleaseState::TagAt(sha),
            None => RemoteReleaseState::Absent,
        });
    };
    Ok(RemoteReleaseState::Present {
        draft: release["draft"].as_bool().unwrap_or(false),
        target_commitish: release["target_commitish"]
            .as_str()
            .unwrap_or_default()
            .to_ascii_lowercase(),
        html_url: release["html_url"].as_str().unwrap_or_default().to_string(),
        release_id: release["id"]
            .as_u64()
            .ok_or("release response missing numeric id")?,
        tag_sha,
    })
}

/// A completed retry must prove the recorded release still backs the receipt.
fn verify_completed_receipt(
    receipt: &ReleaseTransactionReceipt,
    candidate: &ReleaseCandidate,
    observed: &RemoteReleaseState,
) -> Result<()> {
    let RemoteReleaseState::Present {
        draft,
        release_id,
        tag_sha,
        ..
    } = observed
    else {
        return Err(format!(
            "completed receipt references release {} which is no longer public; repair required",
            receipt.tag_name
        )
        .into());
    };
    let tag_matches = tag_sha.as_deref() == Some(candidate.source_revision.as_str());
    if *draft || !tag_matches || release_id != &receipt.release_id {
        return Err(format!(
            "public release {} drifted from the completed receipt (recorded id {}, tag still bound: {tag_matches}); repair required",
            receipt.tag_name, receipt.release_id
        )
        .into());
    }
    Ok(())
}

fn receipt_from_release(release: &Value, tag_name: String) -> Result<ReleaseTransactionReceipt> {
    let release_id = release["id"]
        .as_u64()
        .ok_or("release response missing numeric id")?;
    let release_url = release["html_url"].as_str().unwrap_or_default().to_string();
    validate_nonblank(&release_url, "release html_url")?;
    if !release_url.starts_with("https://") {
        return Err("release html_url must be an https URL".into());
    }
    Ok(ReleaseTransactionReceipt {
        committed_at: Utc::now().to_rfc3339(),
        release_id,
        release_url,
        tag_name,
    })
}

pub(crate) fn commit_release_transaction(args: CommitReleaseTransactionArgs) -> Result<()> {
    let transaction_path = secure_state_path(&args.transaction, false)?;
    // Hold one exclusive lock across inspection, mutation, and CAS write so a
    // concurrent retry cannot interleave a second publication decision.
    let _lock = lock_transaction(&transaction_path)?;
    let original = fs::read(&transaction_path)?;
    let transaction: ReleaseTransaction = serde_json::from_slice(&original)?;
    validate_transaction(&transaction)?;

    if let Some(requested) = trimmed_option(&args.repository)
        && requested != transaction.candidate.repository
    {
        return Err(format!(
            "--repository {requested} disagrees with the immutable transaction candidate {}; re-prepare instead of overriding",
            transaction.candidate.repository
        )
        .into());
    }
    let repository = transaction.candidate.repository.clone();
    validate_repo(&repository)?;

    let token = trimmed_option(&args.github_token)
        .or_else(|| {
            env::var("GITHUB_TOKEN")
                .ok()
                .and_then(|value| trimmed_option(&value))
        })
        .or_else(|| {
            env::var("GH_TOKEN")
                .ok()
                .and_then(|value| trimmed_option(&value))
        })
        .ok_or("commit requires --github-token, GITHUB_TOKEN, or GH_TOKEN")?;
    let provider = GitHubProvider::required(&args.api_base_url, &token);

    if transaction.state == "completed" {
        // Retry of a completed transaction: prove the recorded release still
        // backs the receipt before returning the same completed result.
        let receipt = transaction
            .receipt
            .as_ref()
            .ok_or("completed transaction is missing its receipt")?;
        let observed = observe_release_state(&provider, &repository, &receipt.tag_name)?;
        verify_completed_receipt(receipt, &transaction.candidate, &observed)?;
        return emit_transaction(&transaction);
    }
    if transaction.state != "ready" {
        return Err("bind verified artifacts before committing the release transaction".into());
    }

    let observed =
        observe_release_state(&provider, &repository, &transaction.candidate.release_tag)?;
    let plan = plan_commit(&transaction.candidate, &observed)?;

    if args.dry_run {
        let action = match &plan {
            CommitPlan::Create => "create",
            CommitPlan::AdoptExistingTag => "adopt_existing_tag",
            CommitPlan::Reconcile { .. } => "reconcile",
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": "landmark.release-transaction-commit-plan.v1",
                "transaction_id": transaction.transaction_id,
                "state": transaction.state,
                "repository": repository,
                "action": action,
                "dry_run": true,
            }))?
        );
        return Ok(());
    }

    let receipt = match plan {
        CommitPlan::Create | CommitPlan::AdoptExistingTag => {
            let notes = if args.notes_file.trim().is_empty() {
                String::new()
            } else {
                read_nonempty(Path::new(args.notes_file.trim()))?
            };
            provider.create_release(
                &repository,
                &transaction.candidate.release_tag,
                &transaction.candidate.source_revision,
                &notes,
            )?;
            // Inspect after write so the receipt binds the stored record.
            let created = provider
                .release_by_tag(&repository, &transaction.candidate.release_tag)?
                .ok_or("created release is missing immediately after publication")?;
            receipt_from_release(&created, transaction.candidate.release_tag.clone())?
        }
        CommitPlan::Reconcile {
            release_id,
            html_url,
        } => ReleaseTransactionReceipt {
            committed_at: Utc::now().to_rfc3339(),
            release_id,
            release_url: html_url,
            tag_name: transaction.candidate.release_tag.clone(),
        },
    };

    // Close the inspection-to-mutation race before completion is canonical:
    // another actor may have created or moved the tag in between. The
    // receipt persists only when the public release still matches every
    // immutable candidate identity.
    let post_mutation =
        observe_release_state(&provider, &repository, &transaction.candidate.release_tag)?;
    verify_completed_receipt(&receipt, &transaction.candidate, &post_mutation)?;

    let transaction =
        complete_transaction_locked(&transaction_path, original, transaction, receipt)?;
    emit_transaction(&transaction)
}

/// CAS transition to `completed`; refuses to rewrite an existing receipt.
pub(crate) fn complete_transaction_locked(
    transaction_path: &Path,
    original: Vec<u8>,
    mut transaction: ReleaseTransaction,
    receipt: ReleaseTransactionReceipt,
) -> Result<ReleaseTransaction> {
    if transaction.state == "completed" {
        if transaction.receipt.as_ref() != Some(&receipt) {
            return Err("completed transaction receipt differs from reconciled state".into());
        }
        return Ok(transaction);
    }
    transaction.state = "completed".into();
    transaction.receipt = Some(receipt);
    validate_transaction(&transaction)?;
    write_transaction_cas(
        transaction_path,
        Some(&original),
        &transaction,
        InjectedCrash::None,
    )?;
    Ok(transaction)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate() -> ReleaseCandidate {
        ReleaseCandidate {
            repository: "example/product".into(),
            source_revision: "a".repeat(40),
            previous_tag: "v1.0.0".into(),
            version: "1.1.0".into(),
            release_tag: "v1.1.0".into(),
            notes_sha256: format!("sha256:{}", "b".repeat(64)),
        }
    }

    fn public_release(tag_sha: Option<&str>, release_id: u64) -> RemoteReleaseState {
        RemoteReleaseState::Present {
            draft: false,
            target_commitish: String::new(),
            html_url: "https://github.com/example/product/releases/tag/v1.1.0".into(),
            release_id,
            tag_sha: tag_sha.map(str::to_string),
        }
    }

    fn receipt(release_id: u64) -> ReleaseTransactionReceipt {
        ReleaseTransactionReceipt {
            committed_at: Utc::now().to_rfc3339(),
            release_id,
            release_url: "https://github.com/example/product/releases/tag/v1.1.0".into(),
            tag_name: "v1.1.0".into(),
        }
    }

    #[test]
    fn absent_remote_plans_creation_from_source_revision() {
        assert_eq!(
            plan_commit(&candidate(), &RemoteReleaseState::Absent).unwrap(),
            CommitPlan::Create
        );
    }

    #[test]
    fn matching_existing_tag_is_adopted() {
        let sha = "a".repeat(40);
        assert_eq!(
            plan_commit(&candidate(), &RemoteReleaseState::TagAt(sha)).unwrap(),
            CommitPlan::AdoptExistingTag
        );
    }

    #[test]
    fn mismatched_tag_refusal_names_both_revisions() {
        let error = plan_commit(&candidate(), &RemoteReleaseState::TagAt("f".repeat(40)))
            .unwrap_err()
            .to_string();
        assert!(error.contains("mismatched release"), "{error}");
        assert!(error.contains(&"f".repeat(40)), "{error}");
    }

    #[test]
    fn consistent_public_release_reconciles_without_mutation() {
        assert_eq!(
            plan_commit(&candidate(), &public_release(Some(&"a".repeat(40)), 7)).unwrap(),
            CommitPlan::Reconcile {
                release_id: 7,
                html_url: "https://github.com/example/product/releases/tag/v1.1.0".into()
            }
        );
    }

    #[test]
    fn unanchored_release_blocks_adoption() {
        let error = plan_commit(&candidate(), &public_release(None, 7))
            .unwrap_err()
            .to_string();
        assert!(error.contains("unanchored release"), "{error}");
    }

    #[test]
    fn moved_tag_blocks_adoption_even_when_record_matches() {
        let error = plan_commit(&candidate(), &public_release(Some(&"f".repeat(40)), 7))
            .unwrap_err()
            .to_string();
        assert!(error.contains("mismatched release"), "{error}");
    }

    #[test]
    fn hex_target_contradiction_blocks_adoption() {
        let mut observed = public_release(Some(&"a".repeat(40)), 7);
        if let RemoteReleaseState::Present {
            target_commitish, ..
        } = &mut observed
        {
            *target_commitish = "f".repeat(40);
        }
        let error = plan_commit(&candidate(), &observed)
            .unwrap_err()
            .to_string();
        assert!(error.contains("mismatched release"), "{error}");
    }

    #[test]
    fn draft_release_blocks_commit() {
        let mut observed = public_release(Some(&"a".repeat(40)), 7);
        if let RemoteReleaseState::Present { draft, .. } = &mut observed {
            *draft = true;
        }
        let error = plan_commit(&candidate(), &observed)
            .unwrap_err()
            .to_string();
        assert!(error.contains("draft release"), "{error}");
    }

    #[test]
    fn completed_retry_accepts_matching_release() {
        verify_completed_receipt(
            &receipt(7),
            &candidate(),
            &public_release(Some(&"a".repeat(40)), 7),
        )
        .unwrap();
    }

    #[test]
    fn completed_retry_rejects_vanished_release() {
        let error =
            verify_completed_receipt(&receipt(7), &candidate(), &RemoteReleaseState::Absent)
                .unwrap_err()
                .to_string();
        assert!(error.contains("no longer public"), "{error}");
    }

    #[test]
    fn completed_retry_rejects_drifted_release_id() {
        let error = verify_completed_receipt(
            &receipt(7),
            &candidate(),
            &public_release(Some(&"a".repeat(40)), 9),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("drifted"), "{error}");
    }

    #[test]
    fn completed_retry_rejects_moved_tag() {
        let error = verify_completed_receipt(
            &receipt(7),
            &candidate(),
            &public_release(Some(&"f".repeat(40)), 7),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("drifted"), "{error}");
    }

    #[test]
    fn receipt_requires_https_identity() {
        let mut release = json!({"id": 7_u64, "html_url": "http://example.test/v1.1.0"});
        assert!(receipt_from_release(&release, "v1.1.0".into()).is_err());
        release["html_url"] = json!("https://github.com/example/product/releases/tag/v1.1.0");
        let parsed = receipt_from_release(&release, "v1.1.0".into()).unwrap();
        assert_eq!(parsed.release_id, 7);
        assert_eq!(parsed.tag_name, "v1.1.0");
    }
}
