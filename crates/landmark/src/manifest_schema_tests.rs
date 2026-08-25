use super::*;
use std::fs;

#[test]
fn init_manifest_infers_product_context_from_repo_metadata() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{"name":"@mistystep/atlas","description":"Release operations for app fleets."}"#,
    )
    .unwrap();
    fs::write(
        repo.path().join("README.md"),
        "# Atlas\n\nLandmark-managed release automation.\n",
    )
    .unwrap();

    let manifest = infer_manifest(repo.path());
    assert_eq!(manifest.product.name.as_deref(), Some("Atlas"));
    assert_eq!(
        manifest.product.description.as_deref(),
        Some("Release operations for app fleets.")
    );
    assert_eq!(manifest.audience.as_deref(), Some("developer"));
    assert_eq!(manifest.changelog.source.as_deref(), Some("auto"));

    let rendered = render_manifest_yaml(&manifest).unwrap();
    assert!(
        !rendered.contains("null"),
        "init YAML must omit unset optional fields, got:\n{rendered}"
    );
    let parsed: serde_norway::Value = serde_norway::from_str(&rendered).unwrap();
    assert_eq!(parsed["product"]["name"], "Atlas");
    assert_eq!(parsed["model"]["policy"], "balanced");
    assert!(parsed["artifacts"].get("plaintext").is_none());
    assert!(parsed["artifacts"].get("html").is_none());
    assert!(parsed["artifacts"].get("rss").is_none());
    assert!(parsed["model"].get("primary").is_none());
    assert!(parsed["model"].get("fallbacks").is_none());
    assert!(parsed["budget"].get("max_usd").is_none());
    assert_manifest_yaml_matches_published_schema(&rendered);
}

#[test]
fn rendered_init_manifest_matches_published_json_schema() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("README.md"),
        "# Schema Fixture\n\nValidates init YAML.\n",
    )
    .unwrap();
    let rendered = render_manifest_yaml(&infer_manifest(repo.path())).unwrap();
    assert_manifest_yaml_matches_published_schema(&rendered);
}

#[test]
fn readme_description_joins_wrapped_paragraph() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("README.md"),
        "# Habitat\n\nHabitat is R90's internal-first product-and-engineering delivery platform for\nteams.\n",
    )
    .unwrap();
    assert_eq!(
        readme_description(repo.path()).as_deref(),
        Some(
            "Habitat is R90's internal-first product-and-engineering delivery platform for teams."
        )
    );
}

fn assert_manifest_yaml_matches_published_schema(rendered: &str) {
    let schema_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/landmark-manifest.v1.schema.json"
    );
    let schema: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(schema_path).unwrap()).unwrap();
    let yaml: serde_norway::Value = serde_norway::from_str(rendered).unwrap();
    let json = serde_json::to_value(&yaml).expect("manifest YAML must convert to JSON");
    if let Err(error) = json_conforms_to_schema(&json, &schema, "$") {
        panic!("{error}\nrendered YAML:\n{rendered}");
    }
}

fn json_conforms_to_schema(
    value: &serde_json::Value,
    schema: &serde_json::Value,
    path: &str,
) -> std::result::Result<(), String> {
    if value.is_null() {
        return Err(format!("{path}: schema does not allow null"));
    }
    let expected = schema.get("type").and_then(serde_json::Value::as_str);
    match expected {
        Some("object") => {
            let object = value
                .as_object()
                .ok_or_else(|| format!("{path}: expected object"))?;
            if schema.get("additionalProperties") == Some(&serde_json::Value::Bool(false)) {
                let allowed = schema
                    .get("properties")
                    .and_then(serde_json::Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                for key in object.keys() {
                    if !allowed.contains_key(key) {
                        return Err(format!("{path}: additional property {key}"));
                    }
                }
            }
            if let Some(properties) = schema
                .get("properties")
                .and_then(serde_json::Value::as_object)
            {
                for (key, nested) in properties {
                    if let Some(child) = object.get(key) {
                        json_conforms_to_schema(child, nested, &format!("{path}.{key}"))?;
                    }
                }
            }
        }
        Some("string") => {
            value
                .as_str()
                .ok_or_else(|| format!("{path}: expected string"))?;
        }
        Some("integer") => {
            if value.as_u64().is_none() && value.as_i64().is_none() {
                return Err(format!("{path}: expected integer"));
            }
        }
        Some("number") => {
            value
                .as_f64()
                .ok_or_else(|| format!("{path}: expected number"))?;
        }
        Some("array") => {
            let items = schema
                .get("items")
                .ok_or_else(|| format!("{path}: array schema missing items"))?;
            let array = value
                .as_array()
                .ok_or_else(|| format!("{path}: expected array"))?;
            for (index, child) in array.iter().enumerate() {
                json_conforms_to_schema(child, items, &format!("{path}[{index}]"))?;
            }
        }
        Some(other) => return Err(format!("{path}: unsupported schema type {other}")),
        None => {}
    }
    Ok(())
}
