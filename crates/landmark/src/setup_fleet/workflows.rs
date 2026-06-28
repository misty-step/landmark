use crate::*;
pub(crate) fn fleet_workflow_patches(
    repo: &FleetRepository,
    manifest: &LandmarkManifest,
    recommended_mode: &str,
    workflow: &str,
) -> Vec<FleetWorkflowPatch> {
    if recommended_mode != "github-synthesis-only" {
        return Vec::new();
    }
    match workflow {
        "release-please" => fleet_workflow_file_for_tool(repo, "release-please")
            .map(|workflow_file| FleetWorkflowPatch {
                path: workflow_file.path.clone(),
                description: "update existing release-please workflow with Landmark synthesis job"
                    .into(),
                content: patch_release_please_workflow(
                    workflow_file,
                    &repo.default_branch,
                    manifest,
                ),
            })
            .into_iter()
            .collect(),
        "changesets" => fleet_workflow_file_for_tool(repo, "changesets")
            .map(|workflow_file| FleetWorkflowPatch {
                path: workflow_file.path.clone(),
                description: "update existing changesets workflow with Landmark synthesis job"
                    .into(),
                content: patch_changesets_workflow(
                    workflow_file,
                    &repo.default_branch,
                    repo.package_topology.len() > 1,
                    manifest,
                ),
            })
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn patch_release_please_workflow(
    workflow_file: &FleetWorkflowFile,
    branch: &str,
    manifest: &LandmarkManifest,
) -> String {
    if workflow_file.content.is_empty() {
        return workflow_release_please_for_job(branch, Some(manifest), &workflow_file.release_job);
    }
    let job = release_please_synthesis_job(&workflow_file.release_job, manifest);
    workflow_with_synthesis_job(&workflow_file.content, &job).unwrap_or_else(|| {
        workflow_release_please_for_job(branch, Some(manifest), &workflow_file.release_job)
    })
}

pub(crate) fn patch_changesets_workflow(
    workflow_file: &FleetWorkflowFile,
    branch: &str,
    monorepo: bool,
    manifest: &LandmarkManifest,
) -> String {
    if workflow_file.content.is_empty() {
        return workflow_changesets_for_job(
            branch,
            monorepo,
            Some(manifest),
            &workflow_file.release_job,
        );
    }
    let job = changesets_synthesis_job(&workflow_file.release_job, monorepo, manifest);
    workflow_with_synthesis_job(&workflow_file.content, &job).unwrap_or_else(|| {
        workflow_changesets_for_job(branch, monorepo, Some(manifest), &workflow_file.release_job)
    })
}

pub(crate) fn workflow_with_synthesis_job(content: &str, synthesis_job: &str) -> Option<String> {
    let mut workflow: serde_yaml::Value = serde_yaml::from_str(content).ok()?;
    let jobs_key = serde_yaml::Value::String("jobs".into());
    let synthesize_key = serde_yaml::Value::String("synthesize".into());
    let jobs = workflow
        .as_mapping_mut()?
        .get_mut(&jobs_key)?
        .as_mapping_mut()?;
    let synthesis: serde_yaml::Value = serde_yaml::from_str(synthesis_job).ok()?;
    let synthesis = synthesis.as_mapping()?.get(&synthesize_key)?.clone();
    jobs.insert(synthesize_key, synthesis);
    let mut rendered = serde_yaml::to_string(&workflow).ok()?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Some(rendered)
}

pub(crate) fn release_please_synthesis_job(
    release_job: &str,
    manifest: &LandmarkManifest,
) -> String {
    let manifest_inputs = render_manifest_action_inputs(Some(manifest), 8, Some("release-body"));
    format!(
        r#"synthesize:
  needs: {release_job}
  if: needs.{release_job}.outputs.release_created == 'true'
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
      with:
        fetch-depth: 0
    - uses: misty-step/landmark@v1
      with:
        mode: synthesis-only
        healthcheck: 'true'
        release-tag: ${{{{ needs.{release_job}.outputs.tag_name }}}}
        github-token: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
        llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
    )
}

pub(crate) fn changesets_synthesis_job(
    release_job: &str,
    monorepo: bool,
    manifest: &LandmarkManifest,
) -> String {
    let manifest_inputs = render_manifest_action_inputs(Some(manifest), 8, Some("release-body"));
    if monorepo {
        format!(
            r#"synthesize:
  needs: {release_job}
  if: needs.{release_job}.outputs.published == 'true'
  strategy:
    matrix:
      package: ${{{{ fromJson(needs.{release_job}.outputs.published_packages) }}}}
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
      with:
        fetch-depth: 0
    - uses: misty-step/landmark@v1
      with:
        mode: synthesis-only
        healthcheck: 'true'
        release-tag: ${{{{ matrix.package.name }}}}@${{{{ matrix.package.version }}}}
        github-token: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
        llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
        )
    } else {
        format!(
            r#"synthesize:
  needs: {release_job}
  if: needs.{release_job}.outputs.published == 'true'
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
      with:
        fetch-depth: 0
    - uses: misty-step/landmark@v1
      with:
        mode: synthesis-only
        healthcheck: 'true'
        release-tag: v${{{{ fromJson(needs.{release_job}.outputs.published_packages)[0].version }}}}
        github-token: ${{{{ secrets.GH_RELEASE_TOKEN }}}}
        llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
        )
    }
}

pub(crate) fn fleet_manifest(repo: &FleetRepository, mode: &str) -> LandmarkManifest {
    LandmarkManifest {
        product: ProductManifest {
            name: Some(display_name_from_package(&repo.name)),
            description: Some(format!(
                "Release notes and changelog automation for {}.",
                repo.name_with_owner
            )),
        },
        audience: Some("developer".into()),
        voice: Some("clear, concrete, and specific to shipped behavior".into()),
        changelog: ChangelogManifest {
            source: Some("auto".into()),
        },
        artifacts: ArtifactManifest {
            markdown: Some("docs/releases/{version}.md".into()),
            plaintext: None,
            html: None,
            json: Some("docs/releases/releases.json".into()),
            rss: None,
        },
        release: ReleaseManifest {
            profile: Some(
                if mode == "full" || mode == "github-full" {
                    "full"
                } else {
                    "synthesis-only"
                }
                .into(),
            ),
        },
        model: ModelManifest {
            policy: Some("balanced".into()),
            primary: None,
            fallbacks: Vec::new(),
        },
        budget: BudgetManifest {
            max_input_tokens: Some(12000),
            max_output_tokens: Some(1200),
            max_usd: None,
        },
    }
}

pub(crate) fn render_fleet_plan_markdown(plan: &FleetPlan) -> String {
    let mut out = String::from("# Landmark Fleet Adoption Plan\n\n");
    out.push_str("## Summary\n\n");
    for (status, count) in &plan.summary {
        out.push_str(&format!("- {status}: {count}\n"));
    }
    out.push_str("\n## Repositories\n\n");
    for repo in &plan.repositories {
        out.push_str(&format!(
            "### {}\n\n- Rank: {}\n- Status: {}\n- Repository kind: {}\n- Release surface: {}\n- Integration mode: {}\n- Workflow: {}\n",
            repo.repository,
            repo.rank,
            repo.status,
            repo.repository_kind,
            repo.release_surface,
            repo.integration_mode,
            repo.workflow
        ));
        if !repo.skip_reason.is_empty() {
            out.push_str(&format!("- Skip reason: {}\n", repo.skip_reason));
        }
        if !repo.required_secrets.is_empty() {
            out.push_str(&format!(
                "- Required secrets: {}\n",
                repo.required_secrets.join(", ")
            ));
        }
        if !repo.risk_flags.is_empty() {
            out.push_str(&format!("- Risk flags: {}\n", repo.risk_flags.join("; ")));
        }
        if !repo.integration_rationale.is_empty() {
            out.push_str(&format!(
                "- Rationale: {}\n",
                repo.integration_rationale.join("; ")
            ));
        }
        if !repo.workflow_patches.is_empty() {
            out.push_str(&format!(
                "- Workflow patches: {}\n",
                repo.workflow_patches
                    .iter()
                    .map(|patch| patch.path.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !repo.initial_version_recommendation.is_empty() {
            out.push_str(&format!(
                "- Initial version recommendation: `{}`\n",
                repo.initial_version_recommendation
            ));
        }
        if !repo.initial_tag_recommendation.is_empty() {
            out.push_str(&format!(
                "- Initial tag recommendation: `{}`\n",
                repo.initial_tag_recommendation
            ));
        }
        if !repo.artifact_paths.is_empty() {
            out.push_str(&format!(
                "- Artifact paths: {}\n",
                repo.artifact_paths.join(", ")
            ));
        }
        if !repo.historical_preview_command.is_empty() {
            out.push_str(&format!(
                "- Historical preview command: `{}`\n",
                repo.historical_preview_command
            ));
        }
        if !repo.rollback_guidance.is_empty() {
            out.push_str(&format!("- Rollback: {}\n", repo.rollback_guidance));
        }
        out.push('\n');
    }
    let blocked_or_skipped = plan
        .repositories
        .iter()
        .filter(|repo| matches!(repo.status.as_str(), "blocked" | "skipped"))
        .collect::<Vec<_>>();
    if !blocked_or_skipped.is_empty() {
        out.push_str("## Blocked And Skipped Repositories\n\n");
        for repo in blocked_or_skipped {
            let reason = if repo.skip_reason.is_empty() {
                repo.status.as_str()
            } else {
                repo.skip_reason.as_str()
            };
            out.push_str(&format!(
                "- {}: {} ({})\n",
                repo.repository, reason, repo.integration_mode
            ));
        }
        out.push('\n');
    }
    out
}

pub(crate) fn fleet_workflow_for_plan(repo: &FleetRepositoryPlan) -> String {
    let diagnosis = SetupDiagnosis {
        release_tool: repo.workflow.clone(),
        default_branch: repo.default_branch.clone(),
        tag_format: "v{version}".into(),
        conventional_commits: "unknown: fleet plan generated without local git history".into(),
        monorepo: repo
            .risk_flags
            .iter()
            .any(|flag| flag.contains("multi-package")),
        packages: Vec::new(),
        signals: repo.migration_notes.clone(),
    };
    let workflows = setup_workflows(&diagnosis, Some(&repo.manifest));
    let preferred = match repo.workflow.as_str() {
        "semantic-release" => "semantic-release",
        "release-please" => "release-please",
        "changesets" => {
            if diagnosis.monorepo {
                "changesets-monorepo"
            } else {
                "changesets"
            }
        }
        _ => "manual-tag",
    };
    workflows
        .get(preferred)
        .or_else(|| workflows.values().next())
        .map(|candidate| candidate.content.clone())
        .unwrap_or_else(|| workflow_manual_tag(Some(&repo.manifest)))
}

pub(crate) fn fleet_pr_should_write_workflow(repo: &FleetRepositoryPlan) -> bool {
    if !repo.workflow_patches.is_empty() {
        return false;
    }
    matches!(
        repo.integration_mode.as_str(),
        "github-full" | "github-synthesis-only"
    ) && repo.recommended_mode != "manifest-only"
}

pub(crate) fn render_fleet_apply_markdown(
    repo: &FleetRepositoryPlan,
    branch: &str,
    title: &str,
    commit_message: &str,
    files: &[String],
) -> String {
    let mut out = format!(
        "# Apply Landmark Fleet PR\n\nRepository: `{}`\nBranch: `{}`\nBase: `{}`\nTitle: `{}`\n\n",
        repo.repository, branch, repo.default_branch, title
    );
    out.push_str(
        "Run these commands from a disposable directory after inspecting `diff.md`. They intentionally do not print secret values.\n\n",
    );
    out.push_str("```bash\n");
    out.push_str(&format!(
        "gh repo clone {} repo\n",
        shell_quote(&repo.repository)
    ));
    out.push_str("cd repo\n");
    out.push_str(&format!(
        "git checkout -b {} origin/{}\n",
        shell_quote(branch),
        shell_quote(&repo.default_branch)
    ));
    for file in files.iter().filter(|file| file.as_str() != "diff.md") {
        out.push_str(&format!("# copy rendered `{file}` into this checkout\n"));
    }
    let add_files = files
        .iter()
        .filter(|file| !matches!(file.as_str(), "diff.md" | "APPLY.md"))
        .map(|file| shell_quote(file))
        .collect::<Vec<_>>()
        .join(" ");
    out.push_str(&format!("git add {add_files}\n"));
    out.push_str(&format!("git commit -m {}\n", shell_quote(commit_message)));
    out.push_str(&format!("git push -u origin {}\n", shell_quote(branch)));
    out.push_str(&format!(
        "gh pr create --repo {} --base {} --head {} --title {} --body 'Adopt Landmark using the reviewed fleet rollout receipt. Merge this PR, monitor the downstream release run, then continue the fleet rollout.'\n",
        shell_quote(&repo.repository),
        shell_quote(&repo.default_branch),
        shell_quote(branch),
        shell_quote(title)
    ));
    out.push_str("gh pr checks --watch\n");
    out.push_str("```\n\n");
    if !repo.initial_version_recommendation.is_empty() {
        out.push_str(&format!(
            "Initial version recommendation: `{}`\n\n",
            repo.initial_version_recommendation
        ));
    }
    if !repo.initial_tag_recommendation.is_empty() {
        out.push_str(&format!(
            "Initial tag recommendation: `{}`\n\n",
            repo.initial_tag_recommendation
        ));
    }
    if !repo.historical_preview_command.is_empty() {
        out.push_str(&format!(
            "Preview command: `{}`\n\n",
            repo.historical_preview_command
        ));
    }
    if repo.rollback_guidance.is_empty() {
        out.push_str(
            "Rollback: close the PR and delete the remote branch before continuing the fleet.\n",
        );
    } else {
        out.push_str(&format!("Rollback: {}\n", repo.rollback_guidance));
    }
    out
}

pub(crate) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) fn render_fleet_pr_diff(
    repo: &FleetRepositoryPlan,
    manifest: &str,
    workflow_files: &[(String, String)],
) -> String {
    let mut out = format!(
        "# {}\n\nDry-run branch: `landmark/adopt-{}`\n\n## Files\n\n### .landmark.yml\n\n```yaml\n{}\n```\n\n",
        repo.repository,
        repo.repository.replace('/', "-"),
        manifest
    );
    for (path, workflow) in workflow_files {
        out.push_str(&format!("### {}\n\n```yaml\n{}\n```\n\n", path, workflow));
    }
    if !repo.initial_version_recommendation.is_empty()
        || !repo.initial_tag_recommendation.is_empty()
        || !repo.artifact_paths.is_empty()
        || !repo.historical_preview_command.is_empty()
        || !repo.rollback_guidance.is_empty()
    {
        out.push_str("## Operator Guidance\n\n");
        if !repo.initial_version_recommendation.is_empty() {
            out.push_str(&format!(
                "- Initial version recommendation: `{}`\n",
                repo.initial_version_recommendation
            ));
        }
        if !repo.initial_tag_recommendation.is_empty() {
            out.push_str(&format!(
                "- Initial tag recommendation: `{}`\n",
                repo.initial_tag_recommendation
            ));
        }
        if !repo.artifact_paths.is_empty() {
            out.push_str(&format!(
                "- Artifact paths: {}\n",
                repo.artifact_paths.join(", ")
            ));
        }
        if !repo.historical_preview_command.is_empty() {
            out.push_str(&format!(
                "- Historical preview command: `{}`\n",
                repo.historical_preview_command
            ));
        }
        if !repo.rollback_guidance.is_empty() {
            out.push_str(&format!("- Rollback: {}\n", repo.rollback_guidance));
        }
        out.push('\n');
    }
    out.push_str(&format!(
        "## Notes\n\n{}\n",
        repo.migration_notes
            .iter()
            .map(|note| format!("- {note}"))
            .collect::<Vec<_>>()
            .join("\n")
    ));
    out
}
