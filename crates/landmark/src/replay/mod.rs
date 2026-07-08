use crate::*;
mod contract_scenarios;
mod fake_server;
mod fleet_scenarios;
mod provider_scenarios;
mod release_feed_scenarios;
mod support;

pub(crate) use contract_scenarios::*;
pub(crate) use fake_server::*;
pub(crate) use fleet_scenarios::*;
pub(crate) use provider_scenarios::*;
pub(crate) use release_feed_scenarios::*;
pub(crate) use support::*;

pub(crate) fn replay_action(args: ReplayArgs) -> Result<()> {
    if !matches!(args.format.as_str(), "text" | "json") {
        return Err("--format must be text or json".into());
    }
    let scenarios = scenario_map();
    let selected: Vec<String> = if args.scenario.is_empty() {
        canonical_scenarios()
            .into_iter()
            .map(str::to_string)
            .collect()
    } else {
        args.scenario.clone()
    };
    for name in &selected {
        if !scenarios.contains_key(name) {
            eprintln!("unknown scenario: {name}");
            std::process::exit(2);
        }
    }
    let evidence_dir = if args.evidence_dir.is_empty() {
        env::temp_dir().join(format!("landmark-replay-{}", std::process::id()))
    } else {
        PathBuf::from(&args.evidence_dir)
    };
    fs::create_dir_all(&evidence_dir)?;
    let tmp_root = env::temp_dir().join(format!("landmark-replay-fixtures-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp_root);
    fs::create_dir_all(&tmp_root)?;
    let mut results = Vec::new();
    for name in selected {
        let scenario = scenarios.get(&name).unwrap();
        match scenario(&tmp_root) {
            Ok(mut value) => {
                value["name"] = Value::String(name);
                value["verdict"] = Value::String("passed".to_string());
                results.push(value);
            }
            Err(error) => {
                results
                    .push(json!({"name": name, "verdict": "failed", "error": error.to_string()}));
            }
        }
    }
    let verdict = if results.iter().all(|result| result["verdict"] == "passed") {
        "passed"
    } else {
        "failed"
    };
    let evidence = json!({
        "verdict": verdict,
        "scenario_count": results.len(),
        "scenarios": results,
    });
    fs::write(
        evidence_dir.join("replay-result.json"),
        serde_json::to_string_pretty(&evidence)? + "\n",
    )?;
    if verdict == "passed" {
        if args.format == "json" {
            println!("{}", serde_json::to_string_pretty(&evidence)?);
        } else {
            println!(
                "replay evidence: {}",
                evidence_dir.join("replay-result.json").display()
            );
        }
        Ok(())
    } else {
        Err("one or more replay scenarios failed".into())
    }
}

pub(crate) type Scenario = fn(&Path) -> Result<Value>;

pub(crate) fn scenario_map() -> BTreeMap<String, Scenario> {
    let mut map: BTreeMap<String, Scenario> = BTreeMap::new();
    map.insert(
        "action_static_contract".to_string(),
        scenario_action_static_contract,
    );
    map.insert(
        "consumer_degraded_required_fails".to_string(),
        scenario_consumer_degraded_required_fails,
    );
    map.insert(
        "degraded-required-fails".to_string(),
        scenario_consumer_degraded_required_fails,
    );
    map.insert(
        "consumer_floating_tag_behavior".to_string(),
        scenario_consumer_floating_tag_behavior,
    );
    map.insert(
        "consumer_full_mode_success".to_string(),
        scenario_consumer_full_mode_success,
    );
    map.insert(
        "full-semantic-release".to_string(),
        scenario_consumer_full_mode_success,
    );
    map.insert(
        "consumer_release_update_failure".to_string(),
        scenario_consumer_release_update_failure,
    );
    map.insert(
        "release-body-fallback".to_string(),
        scenario_consumer_release_update_failure,
    );
    map.insert(
        "consumer_synthesis_only_success".to_string(),
        scenario_consumer_synthesis_only_success,
    );
    map.insert(
        "manifest_defaults_and_overrides".to_string(),
        scenario_manifest_defaults_and_overrides,
    );
    map.insert(
        "action_manifest_defaults_precedence".to_string(),
        scenario_action_manifest_defaults_precedence,
    );
    map.insert(
        "local_provider_run".to_string(),
        scenario_local_provider_run,
    );
    map.insert(
        "misty_step_landmark_social_draft".to_string(),
        scenario_misty_step_landmark_social_draft,
    );
    map.insert(
        "release_kit_classification_uses_structured_commits".to_string(),
        scenario_release_kit_classification_uses_structured_commits,
    );
    map.insert(
        "release_grounding_unified_path".to_string(),
        scenario_release_grounding_unified_path,
    );
    map.insert(
        "synthesis_fabrication_gate".to_string(),
        scenario_synthesis_fabrication_gate,
    );
    map.insert(
        "canary-v1-14-0-fabrication".to_string(),
        scenario_synthesis_fabrication_gate,
    );
    map.insert(
        "semver_evidence_agrees".to_string(),
        scenario_semver_evidence_agrees,
    );
    map.insert(
        "semver_evidence_upgrades".to_string(),
        scenario_semver_evidence_upgrades,
    );
    map.insert(
        "semver_evidence_absent".to_string(),
        scenario_semver_evidence_absent,
    );
    map.insert(
        "semver_evidence_tool_failure".to_string(),
        scenario_semver_evidence_tool_failure,
    );
    map.insert(
        "first_run_local_preview".to_string(),
        scenario_first_run_local_preview,
    );
    map.insert(
        "prestable_breaking_stays_below_one".to_string(),
        scenario_prestable_breaking_stays_below_one,
    );
    map.insert(
        "prestable_untagged_first_release_below_one".to_string(),
        scenario_prestable_untagged_first_release_below_one,
    );
    map.insert(
        "stable_line_still_majors".to_string(),
        scenario_stable_line_still_majors,
    );
    map.insert(
        "github_provider_run".to_string(),
        scenario_github_provider_run,
    );
    map.insert(
        "provider_run_parity".to_string(),
        scenario_provider_run_parity,
    );
    map.insert(
        "fleet_adoption_planner".to_string(),
        scenario_fleet_adoption_planner,
    );
    map.insert(
        "self_release_pr_path".to_string(),
        scenario_self_release_pr_path,
    );
    map.insert(
        "synthesis_cost_policy".to_string(),
        scenario_synthesis_cost_policy,
    );
    map.insert(
        "extract_prs_scoped_to_release_range".to_string(),
        scenario_extract_prs_scoped_to_release_range,
    );
    map.insert(
        "release_body_idempotent_across_reruns".to_string(),
        scenario_release_body_idempotent_across_reruns,
    );
    map.insert(
        "pr_fetch_paginates_past_first_page".to_string(),
        scenario_pr_fetch_paginates_past_first_page,
    );
    map.insert(
        "backfill_release_history".to_string(),
        scenario_backfill_release_history,
    );
    map.insert(
        "agent_native_contracts".to_string(),
        scenario_agent_native_contracts,
    );
    map.insert(
        "http_resilience_policy".to_string(),
        scenario_http_resilience_policy,
    );
    map.insert(
        "action_side_effect_coverage".to_string(),
        scenario_action_side_effect_coverage,
    );
    map.insert(
        "release_feed_adapter".to_string(),
        scenario_release_feed_adapter,
    );
    map.insert(
        "synthesis-only-success".to_string(),
        scenario_consumer_synthesis_only_success,
    );
    map.insert(
        "publication_degraded_optional".to_string(),
        scenario_publication_degraded_optional,
    );
    map.insert(
        "publication_degraded_required".to_string(),
        scenario_publication_degraded_required,
    );
    map.insert(
        "summary_artifact_failed".to_string(),
        scenario_summary_artifact_failed,
    );
    map.insert(
        "summary_release_update_failed".to_string(),
        scenario_summary_release_update_failed,
    );
    map.insert(
        "summary_rss_failed".to_string(),
        scenario_summary_rss_failed,
    );
    map
}

pub(crate) fn canonical_scenarios() -> Vec<&'static str> {
    vec![
        "action_static_contract",
        "action_manifest_defaults_precedence",
        "consumer_degraded_required_fails",
        "consumer_floating_tag_behavior",
        "consumer_full_mode_success",
        "fleet_adoption_planner",
        "first_run_local_preview",
        "github_provider_run",
        "local_provider_run",
        // Temporarily de-canonicalized 2026-07-08 (landmark-017 version reset):
        // every surviving tag points at one commit, so all dogfood ranges are
        // zero-diff and the rich-artifact gate (social drafts) can never fire.
        // Re-canonicalize by re-anchoring on the first post-reset release
        // whose importance triggers rich artifacts — tracked by Powder
        // landmark-020. The scenario itself stays registered in
        // scenario_map() and runnable explicitly by name.
        // "misty_step_landmark_social_draft",
        "release_kit_classification_uses_structured_commits",
        "release_grounding_unified_path",
        "synthesis_fabrication_gate",
        "semver_evidence_agrees",
        "semver_evidence_upgrades",
        "semver_evidence_absent",
        "semver_evidence_tool_failure",
        "prestable_breaking_stays_below_one",
        "prestable_untagged_first_release_below_one",
        "stable_line_still_majors",
        "provider_run_parity",
        "manifest_defaults_and_overrides",
        "consumer_release_update_failure",
        "consumer_synthesis_only_success",
        "self_release_pr_path",
        "synthesis_cost_policy",
        "extract_prs_scoped_to_release_range",
        "release_body_idempotent_across_reruns",
        "pr_fetch_paginates_past_first_page",
        "backfill_release_history",
        "publication_degraded_optional",
        "publication_degraded_required",
        "summary_artifact_failed",
        "summary_release_update_failed",
        "summary_rss_failed",
        "agent_native_contracts",
        "http_resilience_policy",
        "action_side_effect_coverage",
        "release_feed_adapter",
    ]
}
