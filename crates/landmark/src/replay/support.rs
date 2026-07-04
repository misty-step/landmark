use crate::*;

pub(crate) fn init_fixture_repo(path: &Path, release_tag: &str) -> Result<()> {
    fs::create_dir_all(path)?;
    run_ok("git", ["init", "-q"], path)?;
    run_ok("git", ["config", "user.name", "Landmark Replay"], path)?;
    run_ok(
        "git",
        ["config", "user.email", "replay@example.invalid"],
        path,
    )?;
    fs::write(path.join("README.md"), "# Fixture\n")?;
    fs::write(
        path.join("CHANGELOG.md"),
        format!("## {release_tag}\n\n- feat: replay fixture\n"),
    )?;
    run_ok("git", ["add", "."], path)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat: seed replay fixture"],
        path,
    )?;
    run_ok("git", ["tag", release_tag], path)?;
    Ok(())
}

pub(crate) fn init_rust_fixture_repo(path: &Path, release_tag: &str) -> Result<()> {
    fs::create_dir_all(path.join("src"))?;
    run_ok("git", ["init", "-q"], path)?;
    run_ok("git", ["config", "user.name", "Landmark Replay"], path)?;
    run_ok(
        "git",
        ["config", "user.email", "replay@example.invalid"],
        path,
    )?;
    fs::write(path.join("README.md"), "# Rust Fixture\n")?;
    fs::write(
        path.join("Cargo.toml"),
        "[package]\nname = \"landmark-replay-fixture\"\nversion = \"1.0.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n",
    )?;
    fs::write(path.join("src/lib.rs"), "pub fn stable_api() {}\n")?;
    fs::write(
        path.join("CHANGELOG.md"),
        format!("## {release_tag}\n\n- feat: replay Rust fixture\n"),
    )?;
    run_ok("git", ["add", "."], path)?;
    run_ok(
        "git",
        ["commit", "-q", "-m", "feat: seed Rust replay fixture"],
        path,
    )?;
    run_ok("git", ["tag", release_tag], path)?;
    Ok(())
}

pub(crate) fn git_tags(path: &Path) -> Result<Vec<String>> {
    Ok(run_ok("git", ["tag", "--list", "--sort=refname"], path)?
        .lines()
        .map(str::to_string)
        .collect())
}

pub(crate) fn current_exe() -> PathBuf {
    env::current_exe().expect("current executable")
}

pub(crate) fn temp_file(prefix: &str) -> Result<PathBuf> {
    let path = env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    fs::write(&path, "")?;
    Ok(path)
}

pub(crate) fn parse_outputs(path: &Path) -> Result<BTreeMap<String, String>> {
    let mut outputs = BTreeMap::new();
    for line in fs::read_to_string(path)?.lines() {
        if let Some((key, value)) = line.split_once('=') {
            outputs.insert(key.to_string(), value.to_string());
        }
    }
    Ok(outputs)
}

pub(crate) fn read_nonempty(path: &Path) -> Result<String> {
    let text = fs::read_to_string(path)?;
    if text.trim().is_empty() {
        Err(format!("{} is empty", path.display()).into())
    } else {
        Ok(text)
    }
}

pub(crate) fn write_json_if_requested<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if !is_requested_path(path) {
        return Ok(());
    }
    ensure_parent(path)?;
    fs::write(path, serde_json::to_string_pretty(value)? + "\n")?;
    Ok(())
}

pub(crate) fn read_json_array_if_requested(path: &Path) -> Result<Vec<Value>> {
    if !is_requested_path(path) || !path.is_file() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub(crate) fn read_json_value_if_requested(path: &Path) -> Result<Value> {
    if !is_requested_path(path) || !path.is_file() {
        return Ok(json!({}));
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

pub(crate) fn is_requested_path(path: &Path) -> bool {
    !path.as_os_str().is_empty() && path != Path::new(".")
}

pub(crate) fn validate_nonblank(value: &str, name: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(format!("{name} must not be blank").into())
    } else {
        Ok(())
    }
}

pub(crate) fn validate_repo(repository: &str) -> Result<()> {
    Regex::new(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")?
        .is_match(repository)
        .then_some(())
        .ok_or_else(|| format!("invalid repository {repository}").into())
}

pub(crate) fn validate_url(url: &str) -> Result<()> {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        Ok(())
    } else {
        Err(format!("invalid URL {url}").into())
    }
}
