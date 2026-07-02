use crate::*;
pub(crate) fn parse_major_tag(release_tag: &str) -> Option<String> {
    let re = Regex::new(r"^v?([0-9]+)\.[0-9]+\.[0-9]+$").unwrap();
    let major = re.captures(release_tag)?.get(1)?.as_str();
    Some(format!("v{major}"))
}

pub(crate) fn close_resolved_failures(args: FailureLifecycleArgs) -> Result<()> {
    let provider = GitHubProvider::required(&args.api_base_url, &args.github_token);
    let issues = provider.find_failure_issues(&args.repository, &args.release_tag)?;
    for issue in issues {
        let number = issue["number"].as_i64().unwrap_or_default();
        provider.comment_issue(
            &args.repository,
            number,
            &format!("Landmark synthesis recovered for {}.", args.release_tag),
        )?;
        provider.close_issue(&args.repository, number)?;
    }
    Ok(())
}

pub(crate) fn report_synthesis_failure(args: ReportFailureArgs) -> Result<()> {
    validate_url(&args.workflow_run_url)?;
    let provider = GitHubProvider::required(&args.api_base_url, &args.github_token);
    if !provider
        .find_failure_issues(&args.repository, &args.release_tag)?
        .is_empty()
    {
        return Ok(());
    }
    let title = failure_issue_title(&args.release_tag);
    let body = format!(
        "Landmark could not synthesize user-facing release notes for `{}`.\n\n- Workflow: {}\n- Run: {}\n- Stage: {}\n- Message: {}\n",
        args.release_tag,
        args.workflow_name,
        args.workflow_run_url,
        args.failure_stage,
        args.failure_message
    );
    provider.create_failure_issue(&args.repository, &title, &body)
}

pub(crate) fn failure_issue_title(release_tag: &str) -> String {
    format!("Landmark release-note synthesis failed for {release_tag}")
}

pub(crate) fn update_version_metadata(args: UpdateVersionArgs) -> Result<()> {
    let version = normalize_version(&args.version)?;
    let package_path = args.repo_root.join("package.json");
    if package_path.is_file() {
        let mut package: Value = serde_json::from_str(&fs::read_to_string(&package_path)?)?;
        package["version"] = Value::String(version.clone());
        fs::write(
            &package_path,
            serde_json::to_string_pretty(&package)? + "\n",
        )?;
    }
    let cargo_path = args.repo_root.join("crates/landmark/Cargo.toml");
    if cargo_path.is_file() {
        replace_toml_version(&cargo_path, &version)?;
    }
    Ok(())
}

pub(crate) fn replace_toml_version(path: &Path, version: &str) -> Result<()> {
    let text = fs::read_to_string(path)?;
    let replaced = Regex::new(r#"(?m)^version = "[^"]+""#)
        .unwrap()
        .replacen(&text, 1, format!("version = \"{version}\""))
        .to_string();
    fs::write(path, replaced)?;
    Ok(())
}

pub(crate) fn check_version_sync(args: CheckVersionArgs) -> Result<()> {
    let tags = run_ok("git", ["tag", "--merged", &args.reference], &args.repo_root)?;
    let latest = latest_semver_version(tags.lines()).ok_or("no semver tags found")?;
    let package: Value =
        serde_json::from_str(&fs::read_to_string(args.repo_root.join("package.json"))?)?;
    let package_version = package["version"].as_str().unwrap_or("");
    let cargo_version =
        cargo_version(&args.repo_root.join("crates/landmark/Cargo.toml")).unwrap_or_default();
    let mut drift = Vec::new();
    if package_version != latest {
        drift.push(format!(
            "package.json has {package_version}, expected {latest}"
        ));
    }
    if !cargo_version.is_empty() && cargo_version != latest {
        drift.push(format!(
            "crates/landmark/Cargo.toml has {cargo_version}, expected {latest}"
        ));
    }
    if drift.is_empty() {
        println!("metadata matches latest tag version {latest}");
        Ok(())
    } else if args.allow_release_candidate
        && package_version == cargo_version
        && semver_key(package_version)? > semver_key(&latest)?
        && release_candidate_changelog_exists(&args.repo_root.join("CHANGELOG.md"), package_version)
    {
        println!("metadata is valid release candidate {package_version} above latest tag {latest}");
        Ok(())
    } else {
        Err(drift.join("\n").into())
    }
}

pub(crate) fn cargo_version(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    Regex::new(r#"(?m)^version = "([^"]+)""#)
        .ok()?
        .captures(&text)?
        .get(1)
        .map(|m| m.as_str().to_string())
}

pub(crate) fn latest_semver_version<'a>(tags: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut versions: Vec<_> = tags.filter_map(semver_from_tag).collect();
    versions.sort();
    versions.pop().map(|(_, value)| value)
}

pub(crate) fn semver_from_tag(tag: &str) -> Option<((u64, u64, u64), String)> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^v?([0-9]+)\.([0-9]+)\.([0-9]+)$").unwrap());
    let caps = re.captures(tag.trim())?;
    let major = caps.get(1)?.as_str().parse().ok()?;
    let minor = caps.get(2)?.as_str().parse().ok()?;
    let patch = caps.get(3)?.as_str().parse().ok()?;
    Some(((major, minor, patch), format!("{major}.{minor}.{patch}")))
}

pub(crate) fn normalize_version(version: &str) -> Result<String> {
    let value = version.trim().trim_start_matches('v');
    if semver_from_tag(value).is_none() {
        return Err(format!("invalid semver version {version}").into());
    }
    Ok(value.to_string())
}

pub(crate) fn check_action_contract(args: CheckActionContractArgs) -> Result<()> {
    let action_path = args.repo_root.join("action.yml");
    let readme_path = args.repo_root.join("README.md");
    let action: serde_yaml::Value = serde_yaml::from_str(&fs::read_to_string(&action_path)?)?;
    let inputs = action["inputs"]
        .as_mapping()
        .ok_or("action.yml missing inputs")?;
    let readme = fs::read_to_string(&readme_path)?;
    let mut errors = Vec::new();
    for (name, spec) in inputs {
        let name = name.as_str().unwrap_or_default();
        if !readme.contains(&format!("`{name}`")) {
            errors.push(format!("README missing input `{name}`"));
        }
        let default = spec["default"].as_str().unwrap_or("");
        if !default.is_empty() && !readme.contains(&format!("| `{name}` |")) {
            errors.push(format!("README input table missing `{name}`"));
        }
    }
    let known: BTreeSet<_> = inputs.keys().filter_map(|key| key.as_str()).collect();
    for path in default_contract_scan_paths(&args.repo_root) {
        if !path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        errors.extend(validate_landmark_usage_inputs(&path, &text, &known));
    }
    errors.extend(validate_manifest_schema_contract(&readme));
    errors.extend(validate_manifest_action_precedence_contract(
        &fs::read_to_string(&action_path)?,
    ));
    errors.extend(validate_self_release_workflow_contract(&args.repo_root)?);
    errors.extend(validate_agent_native_contracts(&args.repo_root)?);
    errors.extend(validate_release_integrity_contract(&readme));
    errors.extend(validate_first_run_adoption_contract(&args.repo_root)?);
    if errors.is_empty() {
        println!("action contract ok");
        Ok(())
    } else {
        Err(errors.join("\n").into())
    }
}

pub(crate) fn validate_agent_native_contracts(repo_root: &Path) -> Result<Vec<String>> {
    let mut errors = Vec::new();
    let readme = fs::read_to_string(repo_root.join("README.md")).unwrap_or_default();
    let guide = fs::read_to_string(repo_root.join("docs/agent-integration.md")).unwrap_or_default();
    for descriptor in schema_descriptors() {
        let path = repo_root.join(descriptor.path);
        if !path.is_file() {
            errors.push(format!("missing schema `{}`", descriptor.path));
            continue;
        }
        let schema: Value = match serde_json::from_str(&fs::read_to_string(&path)?) {
            Ok(schema) => schema,
            Err(error) => {
                errors.push(format!(
                    "schema `{}` is invalid JSON: {error}",
                    descriptor.path
                ));
                continue;
            }
        };
        if schema["$id"].as_str() != Some(descriptor.id) {
            errors.push(format!("schema `{}` has wrong $id", descriptor.path));
        }
        if schema["x-landmark-artifact"].as_str() != Some(descriptor.artifact) {
            errors.push(format!(
                "schema `{}` has wrong x-landmark-artifact",
                descriptor.path
            ));
        }
        if !readme.contains(descriptor.path) {
            errors.push(format!("README missing schema path `{}`", descriptor.path));
        }
        if !guide.contains(descriptor.path) {
            errors.push(format!(
                "agent integration guide missing schema path `{}`",
                descriptor.path
            ));
        }
    }
    errors.extend(validate_command_contract_coverage());
    errors.extend(validate_schema_key_alignment(
        &repo_root.join("schemas/landmark-manifest.v1.schema.json"),
        manifest_schema_key_contracts(),
    )?);
    errors.extend(validate_schema_key_alignment(
        &repo_root.join("schemas/release-context.v1.schema.json"),
        release_context_schema_key_contracts(),
    )?);
    for required in [
        "landmark describe --json",
        "--error-format json",
        "replay-action --scenario agent_native_contracts",
        "stdout carries JSON payloads",
        "stderr carries logs and errors",
    ] {
        if !readme.contains(required) {
            errors.push(format!("README missing agent contract token `{required}`"));
        }
        if !guide.contains(required) {
            errors.push(format!(
                "agent integration guide missing contract token `{required}`"
            ));
        }
    }
    Ok(errors)
}

pub(crate) fn validate_command_contract_coverage() -> Vec<String> {
    let commands: BTreeSet<String> = Cli::command()
        .get_subcommands()
        .map(|command| command.get_name().to_string())
        .collect();
    let contracts: BTreeSet<String> = command_contracts()
        .into_iter()
        .filter_map(|contract| {
            contract
                .command
                .split_whitespace()
                .next()
                .map(str::to_string)
        })
        .collect();
    commands
        .difference(&contracts)
        .map(|command| format!("describe contract missing command `{command}`"))
        .chain(
            contracts
                .difference(&commands)
                .map(|command| format!("describe contract references unknown command `{command}`")),
        )
        .collect()
}

pub(crate) fn validate_release_integrity_contract(readme: &str) -> Vec<String> {
    [
        "--connect-timeout",
        "--max-time",
        "http_resilience_policy",
        "action_side_effect_coverage",
        "webhook",
        "Slack",
    ]
    .iter()
    .filter(|required| !readme.contains(**required))
    .map(|required| format!("README missing release integrity token `{required}`"))
    .collect()
}

pub(crate) fn validate_first_run_adoption_contract(repo_root: &Path) -> Result<Vec<String>> {
    let mut errors = Vec::new();
    let readme = fs::read_to_string(repo_root.join("README.md")).unwrap_or_default();
    let action = fs::read_to_string(repo_root.join("action.yml")).unwrap_or_default();
    let ci = fs::read_to_string(repo_root.join(".github/workflows/ci.yml")).unwrap_or_default();
    let manual_example =
        fs::read_to_string(repo_root.join("examples/manual-tag.yml")).unwrap_or_default();

    for required in [
        "## Adoption Modes",
        "### Local CLI Preview",
        "### Generic CI",
        "### GitHub Action Full Mode",
        "### GitHub Action Synthesis-Only Mode",
        "cargo run --locked -- run --provider local --repo-root .",
        "downloads and checksum-verifies the matching binary itself",
        "GitHub Release",
        "replay-action --scenario first_run_local_preview",
    ] {
        if !readme.contains(required) {
            errors.push(format!(
                "README missing first-run adoption token `{required}`"
            ));
        }
    }

    if action.contains("default: \"22\"") {
        errors.push("action.yml node-version default must not remain on Node 22".into());
    }
    if !action.contains("default: \"24\"") {
        errors.push("action.yml node-version default must be 24".into());
    }
    if ci.contains("node-version: \"22\"") || ci.contains("node-version: 22") {
        errors.push("CI workflow must not pin setup-node to Node 22".into());
    }
    if !ci.contains("node-version: \"24\"") || !ci.contains("node --version | grep '^v24\\.'") {
        errors.push("CI workflow must pin and verify Node 24".into());
    }

    let diagnosis = SetupDiagnosis {
        release_tool: "manual-tag".into(),
        default_branch: "main".into(),
        tag_format: "v{version}".into(),
        conventional_commits: "ready".into(),
        monorepo: false,
        packages: Vec::new(),
        signals: Vec::new(),
    };
    let workflows = setup_workflows(&diagnosis, None);
    let manual = &workflows["manual-tag"].content;
    if manual.contains("push:\n    tags:") {
        errors
            .push("setup manual-tag workflow must not include tag-push trigger by default".into());
    }
    if !manual.contains("release:\n    types: [published]") {
        errors.push("setup manual-tag workflow must use release.published trigger".into());
    }
    for candidate in workflows.values() {
        if candidate.content.contains("node-version: 22") {
            errors.push(format!(
                "{} generated workflow still pins Node 22",
                candidate.path
            ));
        }
    }

    errors.extend(validate_docs_link_targets(repo_root, &readme));
    errors.extend(validate_readme_command_names(&readme));
    for required_model in [
        "anthropic/claude-sonnet-5",
        "anthropic/claude-haiku-4.5",
        "google/gemini-2.5-flash",
    ] {
        if !readme.contains(required_model) {
            errors.push(format!(
                "README missing supported model id `{required_model}`"
            ));
        }
    }
    if readme.contains("misty-step/landmark@v2") {
        errors.push("README references nonexistent misty-step/landmark@v2 example".into());
    }
    if let Err(error) = serde_yaml::from_str::<serde_yaml::Value>(&manual_example) {
        errors.push(format!("examples/manual-tag.yml is invalid YAML: {error}"));
    }
    if manual_example.contains("push:\n    tags:") {
        errors.push("examples/manual-tag.yml must not include tag-push trigger".into());
    }
    if !manual_example.contains("release:\n    types: [published]") {
        errors.push("examples/manual-tag.yml must use release.published trigger".into());
    }
    if !manual_example.contains("release-tag: ${{ github.event.release.tag_name }}") {
        errors.push("examples/manual-tag.yml must use github.event.release.tag_name".into());
    }
    if manual_example.contains("github.ref_name") || manual_example.contains("ref_nameevent") {
        errors
            .push("examples/manual-tag.yml contains stale tag-push release-tag expression".into());
    }

    Ok(errors)
}

pub(crate) fn validate_docs_link_targets(repo_root: &Path, readme: &str) -> Vec<String> {
    let link_re = Regex::new(r"\]\((docs/[^)#]+|examples/[^)#]+|schemas/[^)#]+)\)").unwrap();
    link_re
        .captures_iter(readme)
        .filter_map(|caps| {
            let path = caps.get(1).unwrap().as_str();
            if repo_root.join(path).is_file() {
                None
            } else {
                Some(format!("README links to nonexistent file `{path}`"))
            }
        })
        .collect()
}

pub(crate) fn validate_readme_command_names(readme: &str) -> Vec<String> {
    let commands: BTreeSet<String> = Cli::command()
        .get_subcommands()
        .map(|command| command.get_name().to_string())
        .collect();
    let nested: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::from([
        ("fleet", BTreeSet::from(["scan", "plan", "open-prs"])),
        ("release-policy", BTreeSet::from(["publication", "summary"])),
    ]);
    let command_re =
        Regex::new(r"(?m)(?:^\s*|`)landmark\s+([a-z][a-z-]*)(?:\s+([a-z][a-z-]*))?").unwrap();
    let mut errors = Vec::new();
    for caps in command_re.captures_iter(readme) {
        let command = caps.get(1).unwrap().as_str();
        if !commands.contains(command) {
            errors.push(format!(
                "README references unknown landmark command `{command}`"
            ));
            continue;
        }
        if let Some(subcommands) = nested.get(command)
            && let Some(subcommand) = caps.get(2).map(|value| value.as_str())
            && !subcommands.contains(subcommand)
            && !subcommand.starts_with("--")
        {
            errors.push(format!(
                "README references unknown landmark subcommand `{command} {subcommand}`"
            ));
        }
    }
    errors
}

pub(crate) fn validate_schema_key_alignment(
    path: &Path,
    contracts: Vec<(&'static str, &'static str, &'static [&'static str])>,
) -> Result<Vec<String>> {
    let schema: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
    let mut errors = Vec::new();
    for (label, pointer, allowed) in contracts {
        let actual = schema_property_keys(&schema, pointer);
        let expected: BTreeSet<String> = allowed.iter().map(|key| (*key).to_string()).collect();
        if actual != expected {
            errors.push(format!(
                "{label} schema keys drifted from runtime validation: expected {:?}, got {:?}",
                expected, actual
            ));
        }
    }
    Ok(errors)
}

pub(crate) fn schema_property_keys(schema: &Value, pointer: &str) -> BTreeSet<String> {
    schema
        .pointer(pointer)
        .and_then(Value::as_object)
        .map(|properties| properties.keys().cloned().collect())
        .unwrap_or_default()
}

pub(crate) fn validate_self_release_workflow_contract(repo_root: &Path) -> Result<Vec<String>> {
    let mut errors = Vec::new();
    let ci_path = repo_root.join(".github/workflows/ci.yml");
    let gate_path = repo_root.join("bin/gate");
    let release_path = repo_root.join(".github/workflows/release.yml");
    let ci = fs::read_to_string(&ci_path)?;
    let gate = fs::read_to_string(&gate_path)?;
    let release = fs::read_to_string(&release_path)?;

    if !ci.contains("run: bin/gate") {
        errors.push("CI workflow must delegate the aggregate gate to bin/gate".into());
    }
    if ci.contains("cargo run --locked -- check-version-sync") {
        errors.push("CI workflow must not duplicate version sync outside bin/gate".into());
    }
    for required in [
        "LANDMARK_CI_EVENT",
        "LANDMARK_PR_BASE_SHA",
        "LANDMARK_PR_HEAD_REF",
        "LANDMARK_PR_HEAD_REPO",
        "LANDMARK_REPOSITORY",
    ] {
        if !ci.contains(required) {
            errors.push(format!(
                "CI workflow missing bin/gate context env `{required}`"
            ));
        }
    }

    for required in [
        "git fetch --tags --force origin",
        "cargo run --locked -- check-version-sync --reference \"${tag_ref}\" \"${candidate_args[@]}\"",
        "candidate_args+=(--allow-release-candidate)",
        "LANDMARK_PR_BASE_SHA",
        "landmark/self-release",
    ] {
        if !gate.contains(required) {
            errors.push(format!(
                "bin/gate missing self-release version-sync token `{required}`"
            ));
        }
    }
    if let (Some(fetch), Some(sync)) = (
        gate.find("git fetch --tags --force origin"),
        gate.find("cargo run --locked -- check-version-sync"),
    ) && fetch > sync
    {
        errors.push("bin/gate must fetch tags before version sync".into());
    }

    if let Some(step) = release.find("name: Synthesize landed release notes") {
        let landed_synthesis = &release[step..];
        if !landed_synthesis.contains("synthesis-required: \"false\"") {
            errors.push("self-release landed synthesis must be non-blocking".into());
        }
    } else {
        errors.push("release workflow missing landed synthesis step".into());
    }

    Ok(errors)
}

pub(crate) fn validate_manifest_schema_contract(readme: &str) -> Vec<String> {
    let required = [
        ".landmark.yml",
        "product:",
        "description:",
        "audience:",
        "voice:",
        "changelog:",
        "source:",
        "artifacts:",
        "markdown:",
        "plaintext:",
        "html:",
        "json:",
        "rss:",
        "release:",
        "profile:",
        "model:",
        "policy:",
        "primary:",
        "fallbacks:",
        "budget:",
        "max_input_tokens:",
        "max_output_tokens:",
        "max_usd:",
        "landmark doctor --repo-root .",
    ];
    required
        .iter()
        .filter(|needle| !readme.contains(**needle))
        .map(|needle| format!("README missing manifest schema token `{needle}`"))
        .chain(
            [
                "cheap, balanced, rich, off",
                "full # full or synthesis-only",
                "Non-empty action inputs still win over manifest values.",
                "cheap` uses `anthropic/claude-haiku-4.5`",
                "synthesize --dry-run-cost",
            ]
            .iter()
            .filter(|needle| !readme.contains(**needle))
            .map(|needle| format!("README missing manifest contract text `{needle}`")),
        )
        .collect()
}

pub(crate) fn validate_manifest_action_precedence_contract(action: &str) -> Vec<String> {
    let required = [
        "id: manifest_defaults",
        "manifest-defaults",
        "inputs.llm-model || steps.manifest_defaults.outputs.llm_model",
        "steps.manifest_defaults.outputs.model_policy",
        "MODEL_POLICY: ${{ steps.manifest_defaults.outputs.model_policy }}",
        "Landmark LLM healthcheck skipped because model.policy=off disables synthesis.",
        "inputs.llm-fallback-models || steps.manifest_defaults.outputs.llm_fallback_models",
        "inputs.audience || steps.manifest_defaults.outputs.audience",
        "inputs.changelog-source || steps.manifest_defaults.outputs.changelog_source",
        "inputs.product-description || steps.manifest_defaults.outputs.product_description",
        "inputs.voice-guide || steps.manifest_defaults.outputs.voice_guide",
        "inputs.notes-output-file || steps.manifest_defaults.outputs.notes_output_file",
        "inputs.notes-output-text-file || steps.manifest_defaults.outputs.notes_output_text_file",
        "inputs.notes-output-html-file || steps.manifest_defaults.outputs.notes_output_html_file",
        "inputs.notes-output-json || steps.manifest_defaults.outputs.notes_output_json",
        "inputs.rss-feed-file || steps.manifest_defaults.outputs.rss_feed_file",
        "--context-metadata-file",
    ];
    required
        .iter()
        .filter(|needle| !action.contains(**needle))
        .map(|needle| format!("action.yml missing manifest precedence expression `{needle}`"))
        .collect()
}

pub(crate) fn validate_landmark_usage_inputs(
    path: &Path,
    text: &str,
    known: &BTreeSet<&str>,
) -> Vec<String> {
    let mut errors = Vec::new();
    let key_re = Regex::new(r"^\s*([A-Za-z0-9_-]+):").unwrap();
    let mut in_landmark_step = false;
    let mut in_with = false;
    let mut with_indent = 0usize;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("uses:") || trimmed.starts_with("- uses:") {
            in_landmark_step = workflow_invokes_landmark_action(trimmed) || trimmed == "uses: ./";
            in_with = false;
            continue;
        }
        if !in_landmark_step {
            continue;
        }

        let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
        if !in_with {
            if trimmed == "with:" {
                in_with = true;
                with_indent = indent;
            }
            continue;
        }
        if !trimmed.is_empty() && indent <= with_indent {
            in_with = false;
            in_landmark_step = false;
            continue;
        }
        if let Some(caps) = key_re.captures(line) {
            let key = caps.get(1).unwrap().as_str();
            if !known.contains(key) {
                errors.push(format!(
                    "{} references unknown input `{key}`",
                    path.display()
                ));
            }
        }
    }
    errors
}

pub(crate) fn default_contract_scan_paths(repo_root: &Path) -> Vec<PathBuf> {
    let mut paths = vec![repo_root.join("README.md")];
    for dir in ["examples", ".github/workflows"] {
        if let Ok(entries) = fs::read_dir(repo_root.join(dir)) {
            for entry in entries.flatten() {
                paths.push(entry.path());
            }
        }
    }
    paths
}
