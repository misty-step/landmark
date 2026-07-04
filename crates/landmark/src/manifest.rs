use crate::*;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct LandmarkManifest {
    pub(crate) product: ProductManifest,
    pub(crate) audience: Option<String>,
    pub(crate) voice: Option<String>,
    pub(crate) changelog: ChangelogManifest,
    pub(crate) artifacts: ArtifactManifest,
    pub(crate) release: ReleaseManifest,
    pub(crate) model: ModelManifest,
    pub(crate) budget: BudgetManifest,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct ProductManifest {
    pub(crate) name: Option<String>,
    pub(crate) description: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct ChangelogManifest {
    pub(crate) source: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct ArtifactManifest {
    pub(crate) markdown: Option<String>,
    pub(crate) plaintext: Option<String>,
    pub(crate) html: Option<String>,
    pub(crate) json: Option<String>,
    pub(crate) rss: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct ReleaseManifest {
    pub(crate) profile: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct ModelManifest {
    pub(crate) policy: Option<String>,
    pub(crate) primary: Option<String>,
    pub(crate) fallbacks: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct BudgetManifest {
    pub(crate) max_input_tokens: Option<u64>,
    pub(crate) max_output_tokens: Option<u64>,
    pub(crate) max_usd: Option<f64>,
}

#[derive(Clone, Debug)]
pub(crate) struct EffectiveSynthesisConfig {
    pub(crate) product_name: String,
    pub(crate) product_description: String,
    pub(crate) voice_guide: String,
    pub(crate) audience: String,
    pub(crate) changelog_source: String,
    pub(crate) model_policy: String,
    pub(crate) model: String,
    pub(crate) fallback_models: String,
    pub(crate) max_input_tokens: Option<u64>,
    pub(crate) max_output_tokens: Option<u64>,
    pub(crate) max_usd: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SynthesisContextPacket {
    pub(crate) product: ContextProduct,
    pub(crate) release: ContextRelease,
    pub(crate) grounding: ReleaseGroundingMetadata,
    pub(crate) deterministic: DeterministicReleaseContext,
    pub(crate) sources: Vec<ContextSource>,
    pub(crate) classification: ReleaseClassification,
    pub(crate) cost: CostEstimate,
    pub(crate) decision: SynthesisDecision,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ContextProduct {
    pub(crate) name: String,
    pub(crate) audience: String,
    pub(crate) description: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ContextRelease {
    pub(crate) version: String,
    pub(crate) changelog_source: String,
    pub(crate) model_policy: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct ReleaseGroundingMetadata {
    pub(crate) selected_source: String,
    pub(crate) selected_source_status: String,
    pub(crate) warnings: Vec<String>,
    pub(crate) commit_count: usize,
    pub(crate) diff_stat_count: usize,
    pub(crate) changelog_section: ContextOptionalSource,
    pub(crate) release_body: ContextOptionalSource,
    pub(crate) pull_requests: ContextOptionalSource,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ContextSource {
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) bytes: usize,
    pub(crate) estimated_tokens: u64,
    pub(crate) included: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct DeterministicReleaseContext {
    pub(crate) commits: Vec<ContextCommit>,
    pub(crate) tags: Vec<String>,
    pub(crate) changed_files: Vec<String>,
    pub(crate) diff_stats: Vec<ContextDiffStat>,
    pub(crate) manifest: ContextManifestSummary,
    pub(crate) docs: Vec<ContextDocument>,
    pub(crate) package: Option<ContextPackage>,
    pub(crate) prior_releases: Vec<String>,
    pub(crate) pr_metadata: ContextOptionalSource,
    pub(crate) release_body: ContextOptionalSource,
    pub(crate) artifacts: ContextArtifactAudiences,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ContextCommit {
    pub(crate) subject: String,
    pub(crate) body: String,
    pub(crate) short_hash: String,
    pub(crate) conventional_type: String,
    pub(crate) breaking: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ContextDiffStat {
    pub(crate) path: String,
    pub(crate) additions: u64,
    pub(crate) deletions: u64,
    pub(crate) binary: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct ContextManifestSummary {
    pub(crate) present: bool,
    pub(crate) product_name: String,
    pub(crate) audience: String,
    pub(crate) model_policy: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ContextDocument {
    pub(crate) path: String,
    pub(crate) title: String,
    pub(crate) estimated_tokens: u64,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ContextPackage {
    pub(crate) manager: String,
    pub(crate) name: String,
    pub(crate) description: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct ContextOptionalSource {
    pub(crate) present: bool,
    pub(crate) estimated_tokens: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct ContextArtifactAudiences {
    pub(crate) internal_technical_changelog: String,
    pub(crate) public_release_notes: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ReleaseClassification {
    pub(crate) categories: Vec<String>,
    pub(crate) significance: String,
    pub(crate) user_visible: bool,
    pub(crate) breaking: bool,
    pub(crate) security: bool,
    pub(crate) migration_heavy: bool,
    pub(crate) source: String,
    pub(crate) model: String,
    pub(crate) deterministic_signals: Vec<String>,
    pub(crate) disagreements: Vec<String>,
    pub(crate) reasons: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CostEstimate {
    pub(crate) input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) model_tier: String,
    pub(crate) model: String,
    pub(crate) estimated_usd: f64,
    pub(crate) skip: bool,
    pub(crate) skip_reason: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SynthesisDecision {
    pub(crate) action: String,
    pub(crate) reason: String,
    pub(crate) llm_required: bool,
    pub(crate) model_tier: String,
}

pub(crate) fn init(args: InitArgs) -> Result<()> {
    let manifest = infer_manifest(&args.repo_root);
    let rendered = render_manifest_yaml(&manifest)?;
    if args.dry_run {
        print!("{rendered}");
        return Ok(());
    }
    let output = args.repo_root.join(args.output);
    ensure_parent(&output)?;
    fs::write(output, rendered)?;
    Ok(())
}

pub(crate) fn doctor(args: DoctorArgs) -> Result<()> {
    if !matches!(args.format.as_str(), "text" | "json") {
        return Err("--format must be text or json".into());
    }
    let manifest = load_manifest(&args.repo_root)?.ok_or(".landmark.yml is missing")?;
    let mut errors = validate_manifest(&manifest);
    errors.extend(validate_manifest_completeness(&manifest));
    if errors.is_empty() {
        if args.format == "json" {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "verdict": "passed",
                    "schema": "schemas/landmark-manifest.v1.schema.json",
                    "manifest": ".landmark.yml"
                }))?
            );
        } else {
            println!("manifest ok (schema schemas/landmark-manifest.v1.schema.json)");
        }
        Ok(())
    } else {
        Err(errors.join("\n").into())
    }
}

pub(crate) fn manifest_defaults(args: ManifestDefaultsArgs) -> Result<()> {
    let manifest = load_manifest(&args.repo_root)?.unwrap_or_default();
    let mut values: Vec<(&str, String)> = Vec::new();
    if let Some(value) = manifest.product.name.as_deref().and_then(trimmed_option) {
        values.push(("product_name", sanitize_text(&value)));
    }
    if let Some(value) = manifest
        .product
        .description
        .as_deref()
        .and_then(trimmed_option)
    {
        values.push(("product_description", sanitize_text(&value)));
    }
    if let Some(value) = manifest.audience.as_deref().and_then(trimmed_option) {
        values.push(("audience", sanitize_text(&value)));
    }
    if let Some(value) = manifest.voice.as_deref().and_then(trimmed_option) {
        values.push(("voice_guide", sanitize_text(&value)));
    }
    if let Some(value) = manifest
        .changelog
        .source
        .as_deref()
        .and_then(trimmed_option)
    {
        values.push(("changelog_source", sanitize_text(&value)));
    }
    if let Some(value) = manifest
        .artifacts
        .markdown
        .as_deref()
        .and_then(trimmed_option)
    {
        values.push(("notes_output_file", sanitize_text(&value)));
    }
    if let Some(value) = manifest
        .artifacts
        .plaintext
        .as_deref()
        .and_then(trimmed_option)
    {
        values.push(("notes_output_text_file", sanitize_text(&value)));
    }
    if let Some(value) = manifest.artifacts.html.as_deref().and_then(trimmed_option) {
        values.push(("notes_output_html_file", sanitize_text(&value)));
    }
    if let Some(value) = manifest.artifacts.json.as_deref().and_then(trimmed_option) {
        values.push(("notes_output_json", sanitize_text(&value)));
    }
    if let Some(value) = manifest.artifacts.rss.as_deref().and_then(trimmed_option) {
        values.push(("rss_feed_file", sanitize_text(&value)));
    }
    if let Some(value) = manifest.model.policy.as_deref().and_then(trimmed_option) {
        values.push(("model_policy", sanitize_text(&value)));
    }
    if let Some(value) = manifest
        .model
        .primary
        .as_deref()
        .and_then(trimmed_option)
        .or_else(|| policy_default_model(manifest.model.policy.as_deref()))
    {
        values.push(("llm_model", sanitize_text(&value)));
    }
    if !manifest.model.fallbacks.is_empty() {
        values.push((
            "llm_fallback_models",
            sanitize_text(&manifest.model.fallbacks.join(",")),
        ));
    }
    if is_requested_path(Path::new(&args.github_output)) {
        write_outputs(Path::new(&args.github_output), &values)?;
    } else {
        let json: BTreeMap<_, _> = values.into_iter().collect();
        println!("{}", serde_json::to_string_pretty(&json)?);
    }
    Ok(())
}

pub(crate) fn infer_manifest(root: &Path) -> LandmarkManifest {
    let package = read_package_json(root);
    let package_name = package
        .as_ref()
        .and_then(|value| value["name"].as_str())
        .map(display_name_from_package);
    let readme_name = readme_title(root);
    let product_name = readme_name.or(package_name).or_else(|| {
        root.file_name()
            .and_then(|name| name.to_str())
            .map(display_name_from_package)
    });
    let description = package
        .as_ref()
        .and_then(|value| value["description"].as_str())
        .and_then(trimmed_option)
        .or_else(|| readme_description(root));
    let mut signals = Vec::new();
    let release_tool = detect_release_tool(root, package.as_ref(), &mut signals);
    LandmarkManifest {
        product: ProductManifest {
            name: product_name,
            description,
        },
        audience: Some(infer_audience(root, package.as_ref()).into()),
        voice: Some("clear, specific, user-facing".into()),
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
            profile: Some(if release_tool == "semantic-release" {
                "full".into()
            } else {
                "synthesis-only".into()
            }),
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

pub(crate) fn render_manifest_yaml(manifest: &LandmarkManifest) -> Result<String> {
    Ok(serde_yaml::to_string(manifest)?)
}

pub(crate) fn load_manifest(root: &Path) -> Result<Option<LandmarkManifest>> {
    let path = root.join(".landmark.yml");
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    let raw: serde_yaml::Value = serde_yaml::from_str(&text)?;
    let shape_errors = validate_manifest_yaml_shape(&raw);
    if !shape_errors.is_empty() {
        return Err(shape_errors.join("\n").into());
    }
    let manifest: LandmarkManifest = serde_yaml::from_str(&text)?;
    let errors = validate_manifest(&manifest);
    if errors.is_empty() {
        Ok(Some(manifest))
    } else {
        Err(errors.join("\n").into())
    }
}

pub(crate) fn validate_manifest_yaml_shape(raw: &serde_yaml::Value) -> Vec<String> {
    let mut errors = Vec::new();
    for (label, _, allowed) in manifest_schema_key_contracts() {
        validate_yaml_mapping_keys(yaml_value_at_label(raw, label), label, allowed, &mut errors);
    }
    errors
}

pub(crate) fn manifest_schema_key_contracts()
-> Vec<(&'static str, &'static str, &'static [&'static str])> {
    vec![
        (
            "manifest",
            "/properties",
            &[
                "product",
                "audience",
                "voice",
                "changelog",
                "artifacts",
                "release",
                "model",
                "budget",
            ],
        ),
        (
            "manifest.product",
            "/properties/product/properties",
            &["name", "description"],
        ),
        (
            "manifest.changelog",
            "/properties/changelog/properties",
            &["source"],
        ),
        (
            "manifest.artifacts",
            "/properties/artifacts/properties",
            &["markdown", "plaintext", "html", "json", "rss"],
        ),
        (
            "manifest.release",
            "/properties/release/properties",
            &["profile"],
        ),
        (
            "manifest.model",
            "/properties/model/properties",
            &["policy", "primary", "fallbacks"],
        ),
        (
            "manifest.budget",
            "/properties/budget/properties",
            &["max_input_tokens", "max_output_tokens", "max_usd"],
        ),
    ]
}

pub(crate) fn release_context_schema_key_contracts()
-> Vec<(&'static str, &'static str, &'static [&'static str])> {
    vec![
        (
            "release-context.classification",
            "/properties/classification/properties",
            &[
                "categories",
                "significance",
                "user_visible",
                "breaking",
                "security",
                "migration_heavy",
                "source",
                "model",
                "deterministic_signals",
                "disagreements",
                "reasons",
            ],
        ),
        (
            "release-context.cost",
            "/properties/cost/properties",
            &[
                "input_tokens",
                "output_tokens",
                "model_tier",
                "model",
                "estimated_usd",
                "skip",
                "skip_reason",
            ],
        ),
    ]
}

pub(crate) fn yaml_value_at_label<'a>(
    raw: &'a serde_yaml::Value,
    label: &str,
) -> &'a serde_yaml::Value {
    match label {
        "manifest.product" => &raw["product"],
        "manifest.changelog" => &raw["changelog"],
        "manifest.artifacts" => &raw["artifacts"],
        "manifest.release" => &raw["release"],
        "manifest.model" => &raw["model"],
        "manifest.budget" => &raw["budget"],
        _ => raw,
    }
}

pub(crate) fn validate_yaml_mapping_keys(
    raw: &serde_yaml::Value,
    label: &str,
    allowed: &[&str],
    errors: &mut Vec<String>,
) {
    if raw.is_null() {
        return;
    }
    let Some(mapping) = raw.as_mapping() else {
        errors.push(format!("{label} must be a mapping"));
        return;
    };
    for key in mapping.keys() {
        let Some(key) = key.as_str() else {
            errors.push(format!("{label} keys must be strings"));
            continue;
        };
        if !allowed.contains(&key) {
            errors.push(format!("{label} contains unknown key `{key}`"));
        }
    }
}

pub(crate) fn validate_manifest(manifest: &LandmarkManifest) -> Vec<String> {
    let mut errors = Vec::new();
    for (name, value) in manifest_scalar_fields(manifest) {
        if value.contains('\n') || value.contains('\r') {
            errors.push(format!("manifest {name} must be a single-line scalar"));
        }
    }
    if let Some(audience) = manifest.audience.as_deref().and_then(trimmed_option)
        && !matches!(
            audience.as_str(),
            "general" | "developer" | "end-user" | "enterprise"
        )
    {
        errors.push(format!(
            "manifest audience must be general, developer, end-user, or enterprise; got {audience}"
        ));
    }
    if let Some(source) = manifest
        .changelog
        .source
        .as_deref()
        .and_then(trimmed_option)
        && !matches!(
            source.as_str(),
            "auto" | "changelog" | "release-body" | "prs"
        )
    {
        errors.push(format!(
            "manifest changelog.source must be auto, changelog, release-body, or prs; got {source}"
        ));
    }
    if let Some(profile) = manifest.release.profile.as_deref().and_then(trimmed_option)
        && !matches!(profile.as_str(), "full" | "synthesis-only")
    {
        errors.push(format!(
            "manifest release.profile must be full or synthesis-only; got {profile}"
        ));
    }
    if let Some(policy) = manifest.model.policy.as_deref().and_then(trimmed_option)
        && !matches!(policy.as_str(), "cheap" | "balanced" | "rich" | "off")
    {
        errors.push(format!(
            "manifest model.policy must be cheap, balanced, rich, or off; got {policy}"
        ));
    }
    errors
}

pub(crate) fn validate_manifest_completeness(manifest: &LandmarkManifest) -> Vec<String> {
    let mut errors = Vec::new();
    if manifest
        .product
        .name
        .as_deref()
        .and_then(trimmed_option)
        .is_none()
    {
        errors.push("manifest product.name is required for a complete Landmark manifest".into());
    }
    if manifest
        .product
        .description
        .as_deref()
        .and_then(trimmed_option)
        .is_none()
    {
        errors.push("manifest product.description is required for contextual release notes".into());
    }
    errors
}

pub(crate) fn manifest_scalar_fields(manifest: &LandmarkManifest) -> Vec<(&'static str, &str)> {
    let mut fields = Vec::new();
    push_scalar(
        &mut fields,
        "product.name",
        manifest.product.name.as_deref(),
    );
    push_scalar(
        &mut fields,
        "product.description",
        manifest.product.description.as_deref(),
    );
    push_scalar(&mut fields, "audience", manifest.audience.as_deref());
    push_scalar(&mut fields, "voice", manifest.voice.as_deref());
    push_scalar(
        &mut fields,
        "changelog.source",
        manifest.changelog.source.as_deref(),
    );
    push_scalar(
        &mut fields,
        "artifacts.markdown",
        manifest.artifacts.markdown.as_deref(),
    );
    push_scalar(
        &mut fields,
        "artifacts.plaintext",
        manifest.artifacts.plaintext.as_deref(),
    );
    push_scalar(
        &mut fields,
        "artifacts.html",
        manifest.artifacts.html.as_deref(),
    );
    push_scalar(
        &mut fields,
        "artifacts.json",
        manifest.artifacts.json.as_deref(),
    );
    push_scalar(
        &mut fields,
        "artifacts.rss",
        manifest.artifacts.rss.as_deref(),
    );
    push_scalar(
        &mut fields,
        "release.profile",
        manifest.release.profile.as_deref(),
    );
    push_scalar(
        &mut fields,
        "model.policy",
        manifest.model.policy.as_deref(),
    );
    push_scalar(
        &mut fields,
        "model.primary",
        manifest.model.primary.as_deref(),
    );
    for fallback in &manifest.model.fallbacks {
        fields.push(("model.fallbacks[]", fallback.as_str()));
    }
    fields
}

pub(crate) fn push_scalar<'a>(
    fields: &mut Vec<(&'static str, &'a str)>,
    name: &'static str,
    value: Option<&'a str>,
) {
    if let Some(value) = value {
        fields.push((name, value));
    }
}

pub(crate) fn readme_title(root: &Path) -> Option<String> {
    let readme = fs::read_to_string(root.join("README.md")).ok()?;
    readme
        .lines()
        .find_map(|line| line.trim().strip_prefix("# "))
        .and_then(trimmed_option)
}

pub(crate) fn readme_description(root: &Path) -> Option<String> {
    let readme = fs::read_to_string(root.join("README.md")).ok()?;
    readme
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .and_then(trimmed_option)
}

pub(crate) fn display_name_from_package(name: &str) -> String {
    let name = name.rsplit('/').next().unwrap_or(name);
    name.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn infer_audience(root: &Path, package: Option<&Value>) -> &'static str {
    if root.join("Cargo.toml").is_file()
        || root.join("pyproject.toml").is_file()
        || root.join("go.mod").is_file()
        || package.is_some()
    {
        "developer"
    } else {
        "general"
    }
}

pub(crate) fn trimmed_option(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
