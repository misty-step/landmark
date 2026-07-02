use super::Result;
use super::release_kit::SCHEMA_VERSION;
use serde_json::Value;
use std::collections::BTreeSet;
use std::env;
use std::fs;

fn assert_json_eq(value: &Value, pointer: &str, expected: &str, label: &str) -> Result<()> {
    let actual = value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{label} missing JSON string at {pointer}"))?;
    if actual != expected {
        return Err(format!("{label} expected `{expected}`, got `{actual}`").into());
    }
    Ok(())
}

pub(crate) fn assert_contract(value: &Value, label: &str) -> Result<()> {
    assert_schema_contract(value, label)?;
    assert_json_eq(value, "/schema_version", SCHEMA_VERSION, label)?;
    for pointer in [
        "/generated_at",
        "/product/name",
        "/release/tag",
        "/classification/importance",
        "/classification/why_it_matters",
        "/status/summary",
    ] {
        if value
            .pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            return Err(format!("{label} missing required string at {pointer}").into());
        }
    }
    for pointer in [
        "/classification/audiences",
        "/artifacts",
        "/provenance",
        "/approvals",
    ] {
        if value
            .pointer(pointer)
            .and_then(Value::as_array)
            .is_none_or(|items| items.is_empty())
        {
            return Err(format!("{label} missing non-empty array at {pointer}").into());
        }
    }
    if !value["producer_contracts"].is_array() {
        return Err(format!("{label} producer_contracts must be an array").into());
    }
    let artifacts = value["artifacts"]
        .as_array()
        .ok_or_else(|| format!("{label} artifacts must be an array"))?;
    let artifact_ids: BTreeSet<String> = artifacts
        .iter()
        .filter_map(|artifact| artifact["id"].as_str().map(str::to_string))
        .collect();
    for artifact in artifacts {
        for pointer in ["/id", "/kind", "/audience", "/owner", "/status"] {
            if artifact
                .pointer(pointer)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                return Err(format!("{label} artifact missing string at {pointer}").into());
            }
        }
        if artifact["acceptance"]
            .as_array()
            .is_none_or(|items| items.is_empty())
        {
            return Err(format!("{label} artifact missing acceptance checks").into());
        }
    }
    for provenance in value["provenance"]
        .as_array()
        .ok_or_else(|| format!("{label} provenance must be an array"))?
    {
        let artifact_id = provenance["artifact_id"].as_str().unwrap_or_default();
        if !artifact_ids.contains(artifact_id) {
            return Err(
                format!("{label} provenance references unknown artifact `{artifact_id}`").into(),
            );
        }
        if provenance["sources"]
            .as_array()
            .is_none_or(|items| items.is_empty())
        {
            return Err(format!("{label} provenance missing sources").into());
        }
    }
    for approval in value["approvals"]
        .as_array()
        .ok_or_else(|| format!("{label} approvals must be an array"))?
    {
        let artifact_id = approval["artifact_id"].as_str().unwrap_or_default();
        if !artifact_ids.contains(artifact_id) {
            return Err(
                format!("{label} approval references unknown artifact `{artifact_id}`").into(),
            );
        }
        if approval["state"].as_str().unwrap_or_default().is_empty() {
            return Err(format!("{label} approval missing state").into());
        }
    }
    for contract in value["producer_contracts"]
        .as_array()
        .ok_or_else(|| format!("{label} producer_contracts must be an array"))?
    {
        for pointer in [
            "/id",
            "/producer",
            "/adapter_kind",
            "/command",
            "/evidence_path",
        ] {
            if contract
                .pointer(pointer)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                return Err(
                    format!("{label} producer contract missing string at {pointer}").into(),
                );
            }
        }
        if !contract["mutates"].is_boolean() {
            return Err(format!("{label} producer contract missing mutates boolean").into());
        }
        for pointer in ["/input_artifacts", "/output_artifacts", "/acceptance"] {
            if contract
                .pointer(pointer)
                .and_then(Value::as_array)
                .is_none_or(|items| items.is_empty())
            {
                return Err(format!(
                    "{label} producer contract missing non-empty array at {pointer}"
                )
                .into());
            }
        }
        for pointer in ["/input_artifacts", "/output_artifacts"] {
            for artifact_id in contract
                .pointer(pointer)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if !artifact_ids.contains(artifact_id) {
                    return Err(format!(
                        "{label} producer contract references unknown artifact `{artifact_id}`"
                    )
                    .into());
                }
            }
        }
    }
    Ok(())
}

fn assert_schema_contract(value: &Value, label: &str) -> Result<()> {
    let schema_path = env::current_dir()?.join("schemas/release-kit.v1.schema.json");
    let schema: Value = serde_json::from_str(&fs::read_to_string(&schema_path)?)?;
    assert_supported_schema_keywords(&schema, &schema_path)?;
    let mut errors = Vec::new();
    validate_contract_schema_node(&schema, value, "$", &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{label} does not validate against {}:\n{}",
            schema_path.display(),
            errors.join("\n")
        )
        .into())
    }
}

fn assert_supported_schema_keywords(schema: &Value, schema_path: &std::path::Path) -> Result<()> {
    let mut unsupported = Vec::new();
    collect_unsupported_schema_keywords(schema, "$", &mut unsupported);
    if unsupported.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} uses unsupported JSON Schema keywords for Landmark's replay contract checker:\n{}",
            schema_path.display(),
            unsupported.join("\n")
        )
        .into())
    }
}

fn collect_unsupported_schema_keywords(schema: &Value, path: &str, errors: &mut Vec<String>) {
    let Some(object) = schema.as_object() else {
        return;
    };
    for key in object.keys() {
        if key.starts_with("x-") || supported_schema_keyword(key) {
            continue;
        }
        errors.push(format!("{path} unsupported keyword `{key}`"));
    }
    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for (property, property_schema) in properties {
            collect_unsupported_schema_keywords(
                property_schema,
                &format!("{path}/properties/{property}"),
                errors,
            );
        }
    }
    if let Some(item_schema) = object.get("items") {
        collect_unsupported_schema_keywords(item_schema, &format!("{path}/items"), errors);
    }
}

fn supported_schema_keyword(keyword: &str) -> bool {
    matches!(
        keyword,
        "$schema"
            | "$id"
            | "title"
            | "type"
            | "additionalProperties"
            | "required"
            | "properties"
            | "const"
            | "enum"
            | "items"
    )
}

fn validate_contract_schema_node(
    schema: &Value,
    value: &Value,
    path: &str,
    errors: &mut Vec<String>,
) {
    if let Some(expected) = schema.get("const")
        && value != expected
    {
        errors.push(format!("{path} expected const {expected}, got {value}"));
    }
    if let Some(variants) = schema.get("enum").and_then(Value::as_array)
        && !variants.iter().any(|variant| variant == value)
    {
        errors.push(format!("{path} expected one of {variants:?}, got {value}"));
    }
    let Some(schema_type) = schema.get("type").and_then(Value::as_str) else {
        return;
    };
    match schema_type {
        "object" => validate_object_schema_node(schema, value, path, errors),
        "array" => validate_array_schema_node(schema, value, path, errors),
        "string" if !value.is_string() => {
            errors.push(format!("{path} expected string, got {}", json_type(value)));
        }
        "boolean" if !value.is_boolean() => {
            errors.push(format!("{path} expected boolean, got {}", json_type(value)));
        }
        "integer" if !value.is_i64() && !value.is_u64() => {
            errors.push(format!("{path} expected integer, got {}", json_type(value)));
        }
        "number" if !value.is_number() => {
            errors.push(format!("{path} expected number, got {}", json_type(value)));
        }
        _ => {}
    }
}

fn validate_object_schema_node(
    schema: &Value,
    value: &Value,
    path: &str,
    errors: &mut Vec<String>,
) {
    let Some(object) = value.as_object() else {
        errors.push(format!("{path} expected object, got {}", json_type(value)));
        return;
    };
    let required: BTreeSet<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    for key in &required {
        if !object.contains_key(*key) {
            errors.push(format!("{path} missing required property `{key}`"));
        }
    }
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
        for key in object.keys() {
            if !properties.contains_key(key) {
                errors.push(format!("{path} has unexpected property `{key}`"));
            }
        }
    }
    for (key, property_schema) in properties {
        if let Some(child) = object.get(&key) {
            validate_contract_schema_node(
                &property_schema,
                child,
                &format!("{path}/{key}"),
                errors,
            );
        }
    }
}

fn validate_array_schema_node(schema: &Value, value: &Value, path: &str, errors: &mut Vec<String>) {
    let Some(items) = value.as_array() else {
        errors.push(format!("{path} expected array, got {}", json_type(value)));
        return;
    };
    if let Some(item_schema) = schema.get("items") {
        for (index, item) in items.iter().enumerate() {
            validate_contract_schema_node(item_schema, item, &format!("{path}/{index}"), errors);
        }
    }
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
