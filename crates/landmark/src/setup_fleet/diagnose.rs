use crate::*;

pub(crate) fn diagnose_setup(root: &Path) -> SetupDiagnosis {
    let mut signals = Vec::new();
    let package = read_package_json(root);
    let release_tool = detect_release_tool(root, package.as_ref(), &mut signals);
    let packages = detect_packages(root, package.as_ref(), &mut signals);
    let monorepo = packages.len() > 1 || root.join(".changeset").is_dir();
    if root.join(".changeset").is_dir() {
        signals.push(".changeset directory present".into());
    }
    SetupDiagnosis {
        release_tool,
        default_branch: detect_default_branch(root),
        tag_format: detect_tag_format(root, &packages),
        conventional_commits: diagnose_conventional_commits(root),
        monorepo,
        packages,
        signals,
    }
}

pub(crate) fn read_package_json(root: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(root.join("package.json")).ok()?).ok()
}

pub(crate) fn detect_release_tool(
    root: &Path,
    package: Option<&Value>,
    signals: &mut Vec<String>,
) -> String {
    let workflows = read_dir_text(root.join(".github/workflows"));
    if workflows.contains("googleapis/release-please-action")
        || root.join("release-please-config.json").is_file()
    {
        signals.push("release-please workflow or config present".into());
        return "release-please".into();
    }
    if root.join(".changeset").is_dir() || workflows.contains("changesets/action") {
        signals.push("changesets workflow or .changeset directory present".into());
        return "changesets".into();
    }
    if root.join(".releaserc").is_file()
        || root.join(".releaserc.json").is_file()
        || package_has_dependency(package, "semantic-release")
    {
        signals.push("semantic-release config or dependency present".into());
        return "semantic-release".into();
    }
    if workflows.contains("gh release create") {
        signals.push("manual GitHub release command detected".into());
    }
    "manual-tag".into()
}

pub(crate) fn read_dir_text(path: PathBuf) -> String {
    let mut text = String::new();
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.path().is_file()
                && let Ok(file_text) = fs::read_to_string(entry.path())
            {
                text.push_str(&file_text);
                text.push('\n');
            }
        }
    }
    text
}

pub(crate) fn package_has_dependency(package: Option<&Value>, name: &str) -> bool {
    let Some(package) = package else {
        return false;
    };
    ["dependencies", "devDependencies"]
        .iter()
        .any(|key| package[*key].get(name).is_some())
}

pub(crate) fn detect_packages(
    root: &Path,
    package: Option<&Value>,
    signals: &mut Vec<String>,
) -> Vec<String> {
    let mut packages = Vec::new();
    if let Some(name) = package.and_then(|value| value["name"].as_str()) {
        packages.push(name.to_string());
    }
    if let Some(workspaces) = package.and_then(|value| value["workspaces"].as_array()) {
        signals.push("package.json workspaces present".into());
        packages.extend(
            workspaces
                .iter()
                .filter_map(|value| value.as_str())
                .map(str::to_string),
        );
    }
    for manifest in ["Cargo.toml", "pyproject.toml", "go.mod"] {
        if root.join(manifest).is_file() {
            signals.push(format!("{manifest} present"));
        }
    }
    packages.sort();
    packages.dedup();
    packages
}

pub(crate) fn detect_default_branch(root: &Path) -> String {
    if let Ok(output) = run_ok(
        "git",
        ["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
        root,
    ) && let Some(branch) = output.trim().strip_prefix("origin/")
    {
        return branch.to_string();
    }
    for branch in ["main", "master"] {
        if run_ok("git", ["rev-parse", "--verify", branch], root).is_ok() {
            return branch.to_string();
        }
    }
    "main".into()
}

pub(crate) fn detect_tag_format(root: &Path, packages: &[String]) -> String {
    let tags = run_ok("git", ["tag", "--list"], root).unwrap_or_default();
    let package_re = Regex::new(r"^[A-Za-z0-9_.-]+@v?[0-9]+\.[0-9]+\.[0-9]+").unwrap();
    let v_re = Regex::new(r"^v[0-9]+\.[0-9]+\.[0-9]+").unwrap();
    let bare_re = Regex::new(r"^[0-9]+\.[0-9]+\.[0-9]+").unwrap();
    let package_tag = tags.lines().any(|tag| package_re.is_match(tag));
    let v_tag = tags.lines().any(|tag| v_re.is_match(tag));
    let bare_tag = tags.lines().any(|tag| bare_re.is_match(tag));
    if package_tag || packages.len() > 1 {
        "package@{version}".into()
    } else if v_tag || !bare_tag {
        "v{version}".into()
    } else {
        "{version}".into()
    }
}

pub(crate) fn diagnose_conventional_commits(root: &Path) -> String {
    let log = run_ok("git", ["log", "-n", "30", "--pretty=%s"], root).unwrap_or_default();
    let subjects: Vec<_> = log.lines().collect();
    if subjects.is_empty() {
        return "unknown: no git history visible".into();
    }
    let conventional =
        Regex::new(r"^(feat|fix|docs|chore|refactor|test|ci|build|perf)(\(.+\))?!?: ").unwrap();
    let matches = subjects
        .iter()
        .filter(|subject| conventional.is_match(subject))
        .count();
    if matches * 2 >= subjects.len() {
        format!(
            "ready: {matches}/{} recent commits look conventional",
            subjects.len()
        )
    } else {
        format!(
            "needs-review: {matches}/{} recent commits look conventional",
            subjects.len()
        )
    }
}

pub(crate) fn recommend_setup(
    diagnosis: &SetupDiagnosis,
    manifest: Option<&LandmarkManifest>,
) -> SetupRecommendation {
    let workflow = match diagnosis.release_tool.as_str() {
        "semantic-release" => "semantic-release",
        "release-please" => "release-please",
        "changesets" if diagnosis.monorepo => "changesets-monorepo",
        "changesets" => "changesets",
        _ => "manual-tag",
    };
    let manifest_profile = manifest
        .and_then(|manifest| manifest.release.profile.as_deref())
        .and_then(trimmed_option);
    let mode = if let Some(profile) = manifest_profile.as_deref() {
        profile
    } else if workflow == "semantic-release" {
        "full"
    } else {
        "synthesis-only"
    };
    let mut rationale = vec![format!("detected release tool: {}", diagnosis.release_tool)];
    rationale.push(format!("default branch: {}", diagnosis.default_branch));
    rationale.push(format!("tag format: {}", diagnosis.tag_format));
    if let Some(profile) = manifest_profile.as_deref() {
        rationale.push(format!("manifest release profile: {profile}"));
    }
    if diagnosis.monorepo {
        rationale.push("monorepo outputs enabled".into());
    }
    SetupRecommendation {
        mode: mode.into(),
        workflow: workflow.into(),
        rationale,
    }
}

pub(crate) fn setup_workflows(
    diagnosis: &SetupDiagnosis,
    manifest: Option<&LandmarkManifest>,
) -> BTreeMap<String, WorkflowCandidate> {
    let branch = &diagnosis.default_branch;
    let mut workflows = BTreeMap::new();
    for (name, tool, mode, content) in [
        (
            "semantic-release",
            "semantic-release",
            "full",
            workflow_semantic_release(branch, manifest),
        ),
        (
            "release-please",
            "release-please",
            "synthesis-only",
            workflow_release_please(branch, manifest),
        ),
        (
            "changesets",
            "changesets",
            "synthesis-only",
            workflow_changesets(branch, false, manifest),
        ),
        (
            "changesets-monorepo",
            "changesets",
            "synthesis-only",
            workflow_changesets(branch, true, manifest),
        ),
        (
            "manual-tag",
            "manual-tag",
            "synthesis-only",
            workflow_manual_tag(manifest),
        ),
    ] {
        workflows.insert(
            name.to_string(),
            WorkflowCandidate {
                path: format!(".github/workflows/landmark-{name}.yml"),
                release_tool: tool.into(),
                mode: mode.into(),
                rationale: vec![
                    "includes Landmark healthcheck before the release attempt".into(),
                    "declares contents/issues/pull-requests write permissions".into(),
                ],
                content,
            },
        );
    }
    workflows
}

pub(crate) fn workflow_semantic_release(
    branch: &str,
    manifest: Option<&LandmarkManifest>,
) -> String {
    let manifest_inputs = render_manifest_action_inputs(manifest, 10, None);
    format!(
        r#"name: Release

on:
  push:
    branches: [{branch}]
  workflow_dispatch:

permissions:
  contents: write
  issues: write
  pull-requests: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
          persist-credentials: false
      - uses: misty-step/landmark@v0
        with:
          mode: full
          healthcheck: 'true'
          github-token: ${{{{ github.token }}}}
          llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
    )
}

pub(crate) fn workflow_release_please(branch: &str, manifest: Option<&LandmarkManifest>) -> String {
    workflow_release_please_for_job(branch, manifest, "release-please")
}

pub(crate) fn workflow_release_please_for_job(
    branch: &str,
    manifest: Option<&LandmarkManifest>,
    release_job: &str,
) -> String {
    let manifest_inputs = render_manifest_action_inputs(manifest, 10, Some("release-body"));
    format!(
        r#"name: Release

on:
  push:
    branches: [{branch}]

permissions:
  contents: write
  issues: write
  pull-requests: write

jobs:
  {release_job}:
    runs-on: ubuntu-latest
    outputs:
      release_created: ${{{{ steps.release.outputs.release_created }}}}
      tag_name: ${{{{ steps.release.outputs.tag_name }}}}
    steps:
      - uses: googleapis/release-please-action@v4
        id: release

  synthesize:
    needs: {release_job}
    if: needs.{release_job}.outputs.release_created == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: misty-step/landmark@v0
        with:
          mode: synthesis-only
          healthcheck: 'true'
          release-tag: ${{{{ needs.{release_job}.outputs.tag_name }}}}
          github-token: ${{{{ github.token }}}}
          llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
    )
}

pub(crate) fn workflow_changesets(
    branch: &str,
    monorepo: bool,
    manifest: Option<&LandmarkManifest>,
) -> String {
    workflow_changesets_for_job(branch, monorepo, manifest, "release")
}

pub(crate) fn workflow_changesets_for_job(
    branch: &str,
    monorepo: bool,
    manifest: Option<&LandmarkManifest>,
    release_job: &str,
) -> String {
    let manifest_inputs = render_manifest_action_inputs(manifest, 10, Some("release-body"));
    if monorepo {
        format!(
            r#"name: Release

on:
  push:
    branches: [{branch}]

permissions:
  contents: write
  issues: write
  pull-requests: write

jobs:
  {release_job}:
    runs-on: ubuntu-latest
    outputs:
      published: ${{{{ steps.changesets.outputs.published }}}}
      published_packages: ${{{{ steps.changesets.outputs.publishedPackages }}}}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 24
      - run: npm ci
      - uses: changesets/action@v1
        id: changesets
        with:
          publish: npm run release
        env:
          GITHUB_TOKEN: ${{{{ github.token }}}}
          NPM_TOKEN: ${{{{ secrets.NPM_TOKEN }}}}

  synthesize:
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
      - uses: misty-step/landmark@v0
        with:
          mode: synthesis-only
          healthcheck: 'true'
          release-tag: ${{{{ matrix.package.name }}}}@${{{{ matrix.package.version }}}}
          github-token: ${{{{ github.token }}}}
          llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
        )
    } else {
        format!(
            r#"name: Release

on:
  push:
    branches: [{branch}]

permissions:
  contents: write
  issues: write
  pull-requests: write

jobs:
  {release_job}:
    runs-on: ubuntu-latest
    outputs:
      published: ${{{{ steps.changesets.outputs.published }}}}
      published_packages: ${{{{ steps.changesets.outputs.publishedPackages }}}}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 24
      - run: npm ci
      - uses: changesets/action@v1
        id: changesets
        with:
          publish: npm run release
        env:
          GITHUB_TOKEN: ${{{{ github.token }}}}
          NPM_TOKEN: ${{{{ secrets.NPM_TOKEN }}}}

  synthesize:
    needs: {release_job}
    if: needs.{release_job}.outputs.published == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: misty-step/landmark@v0
        with:
          mode: synthesis-only
          healthcheck: 'true'
          release-tag: v${{{{ fromJson(needs.{release_job}.outputs.published_packages)[0].version }}}}
          github-token: ${{{{ github.token }}}}
          llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
        )
    }
}

pub(crate) fn workflow_manual_tag(manifest: Option<&LandmarkManifest>) -> String {
    let manifest_inputs = render_manifest_action_inputs(manifest, 10, Some("auto"));
    format!(
        r#"name: Synthesize Release Notes

on:
  release:
    types: [published]

permissions:
  contents: write
  issues: write
  pull-requests: write

jobs:
  synthesize:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: misty-step/landmark@v0
        with:
          mode: synthesis-only
          healthcheck: 'true'
          release-tag: ${{{{ github.event.release.tag_name }}}}
          github-token: ${{{{ github.token }}}}
          llm-api-key: ${{{{ secrets.OPENROUTER_API_KEY }}}}
{manifest_inputs}
"#
    )
}

pub(crate) fn render_manifest_action_inputs(
    manifest: Option<&LandmarkManifest>,
    indent: usize,
    default_changelog_source: Option<&str>,
) -> String {
    let mut lines = Vec::new();
    if let Some(manifest) = manifest {
        if let Some(value) = manifest
            .product
            .description
            .as_deref()
            .and_then(trimmed_option)
        {
            lines.push(("product-description", value));
        }
        if let Some(value) = manifest.audience.as_deref().and_then(trimmed_option) {
            lines.push(("audience", value));
        }
        if let Some(value) = manifest.voice.as_deref().and_then(trimmed_option) {
            lines.push(("voice-guide", value));
        }
        if let Some(value) = manifest
            .changelog
            .source
            .as_deref()
            .and_then(trimmed_option)
            .or_else(|| default_changelog_source.map(str::to_string))
        {
            lines.push(("changelog-source", value));
        }
        if let Some(value) = manifest
            .artifacts
            .markdown
            .as_deref()
            .and_then(trimmed_option)
        {
            lines.push(("notes-output-file", value));
        }
        if let Some(value) = manifest
            .artifacts
            .plaintext
            .as_deref()
            .and_then(trimmed_option)
        {
            lines.push(("notes-output-text-file", value));
        }
        if let Some(value) = manifest.artifacts.html.as_deref().and_then(trimmed_option) {
            lines.push(("notes-output-html-file", value));
        }
        if let Some(value) = manifest.artifacts.json.as_deref().and_then(trimmed_option) {
            lines.push(("notes-output-json", value));
        }
        if let Some(value) = manifest.artifacts.rss.as_deref().and_then(trimmed_option) {
            lines.push(("rss-feed-file", value));
        }
        if let Some(value) = manifest.model.primary.as_deref().and_then(trimmed_option) {
            lines.push(("llm-model", value));
        }
        if !manifest.model.fallbacks.is_empty() {
            lines.push(("llm-fallback-models", manifest.model.fallbacks.join(",")));
        }
    } else if let Some(value) = default_changelog_source {
        lines.push(("changelog-source", value.to_string()));
    }

    let padding = " ".repeat(indent);
    lines
        .into_iter()
        .map(|(key, value)| format!("{padding}{key}: {}", yaml_scalar(&value)))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn yaml_scalar(value: &str) -> String {
    serde_yaml::to_string(value)
        .ok()
        .and_then(|rendered| {
            rendered
                .lines()
                .find(|line| !line.trim().is_empty() && !line.trim().starts_with("---"))
                .map(|line| line.trim().to_string())
        })
        .unwrap_or_else(|| format!("{value:?}"))
}
