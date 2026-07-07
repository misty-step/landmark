#![forbid(unsafe_code)]

//! MCP server for Landmark's own core read-only verbs (landmark-920).
//!
//! Landmark's SKILL.md used to admit "does not currently expose an MCP
//! server." This crate is the thin wrap the agent-native `--json`/
//! `--error-format json` CLI contract already made cheap: every tool here
//! shells out to the real `landmark` binary (matching `bastion-mcp`'s
//! established pattern of wrapping an existing CLI rather than
//! re-implementing its logic) and passes its stdout straight through.
//!
//! Only read-only, side-effect-free verbs are exposed. `run_dry_run` always
//! forces `--provider local --dry-run` -- no GitHub calls, no files written
//! -- which the CLI's own dry-run mode already guarantees; there is no way
//! to reach `provider=github` or the mutating pipeline through this server.
//! `synthesize` (the LLM-calling changelog step) is a deliberate exclusion,
//! not an oversight: it requires an API key and spends real money per call,
//! which does not belong behind an MCP tool argument any agent can invoke.

use std::env;
use std::process::Command;

use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: &'static str,
}

pub const TOOLS: &[ToolDef] = &[
    ToolDef {
        name: "describe",
        description: "Landmark's agent-native self-description: every command, its schema, examples, and the failure taxonomy. Start here.",
        input_schema: r#"{"type":"object","properties":{"landmark_bin":{"type":"string"}}}"#,
    },
    ToolDef {
        name: "run_dry_run",
        description: "Compute the release decision (version bump + evidence) and the release-kit plan for a repo, without writing any files or calling GitHub. Always runs with provider=local and --dry-run.",
        input_schema: r#"{"type":"object","properties":{"repo_root":{"type":"string","description":"Path to the repo to analyze, default \".\""},"release_tag":{"type":"string","description":"Explicit release tag instead of computing one from commits"},"previous_tag":{"type":"string","description":"Previous tag to diff against instead of the latest matching tag"},"landmark_bin":{"type":"string"}}}"#,
    },
    ToolDef {
        name: "doctor",
        description: "Validate a repo's .landmark.yml manifest and repo signals before a release run.",
        input_schema: r#"{"type":"object","properties":{"repo_root":{"type":"string","description":"Path to the repo to validate, default \".\""},"landmark_bin":{"type":"string"}}}"#,
    },
];

pub fn tool_defs_json() -> Value {
    Value::Array(
        TOOLS
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "inputSchema": serde_json::from_str::<Value>(tool.input_schema)
                        .expect("tool schema is valid json"),
                })
            })
            .collect(),
    )
}

pub fn handle_json_rpc(request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");

    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": request["params"]["protocolVersion"]
                .as_str()
                .unwrap_or("2024-11-05"),
            "serverInfo": {"name": "landmark", "version": env!("CARGO_PKG_VERSION")},
            "capabilities": {"tools": {"listChanged": false}},
        })),
        "tools/list" => Ok(json!({ "tools": tool_defs_json() })),
        "tools/call" => {
            let params = &request["params"];
            let name = params["name"].as_str().unwrap_or("");
            Ok(match call_tool(name, &params["arguments"]) {
                Ok(value) => value,
                Err(message) => tool_error(message),
            })
        }
        "ping" => Ok(json!({})),
        other => Err(format!("method not found: {other}")),
    };

    id.map(|id| match result {
        Ok(value) => json!({"jsonrpc": "2.0", "id": id, "result": value}),
        Err(message) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32603, "message": message},
        }),
    })
}

pub fn call_tool(name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "describe" => describe(args),
        "run_dry_run" => run_dry_run(args),
        "doctor" => doctor(args),
        other => Err(format!("unknown tool: {other}")),
    }
}

fn landmark_bin(args: &Value) -> String {
    args["landmark_bin"]
        .as_str()
        .map(ToOwned::to_owned)
        .or_else(|| env::var("LANDMARK_BIN").ok())
        .unwrap_or_else(|| "landmark".to_owned())
}

fn repo_root(args: &Value) -> String {
    args["repo_root"].as_str().unwrap_or(".").to_owned()
}

fn describe(args: &Value) -> Result<Value, String> {
    run_landmark(&landmark_bin(args), &["describe", "--json"])
}

fn run_dry_run(args: &Value) -> Result<Value, String> {
    let bin = landmark_bin(args);
    let root = repo_root(args);
    let mut cmd_args = vec![
        "run".to_owned(),
        "--provider".to_owned(),
        "local".to_owned(),
        "--dry-run".to_owned(),
        "--repo-root".to_owned(),
        root,
        "--error-format".to_owned(),
        "json".to_owned(),
    ];
    if let Some(tag) = args["release_tag"].as_str().filter(|s| !s.is_empty()) {
        cmd_args.push("--release-tag".to_owned());
        cmd_args.push(tag.to_owned());
    }
    if let Some(tag) = args["previous_tag"].as_str().filter(|s| !s.is_empty()) {
        cmd_args.push("--previous-tag".to_owned());
        cmd_args.push(tag.to_owned());
    }
    let refs: Vec<&str> = cmd_args.iter().map(String::as_str).collect();
    run_landmark(&bin, &refs)
}

fn doctor(args: &Value) -> Result<Value, String> {
    let root = repo_root(args);
    run_landmark(
        &landmark_bin(args),
        &[
            "doctor",
            "--format",
            "json",
            "--repo-root",
            &root,
            "--error-format",
            "json",
        ],
    )
}

/// Runs `landmark <args>`, returning `tool_result` on exit 0. On a nonzero
/// exit, stderr already carries a structured `--error-format json` envelope
/// (see `landmark::errors::structured_error_json`); that becomes the tool
/// error message verbatim so a caller sees the exact same code/stage/
/// retryable/user_action fields the CLI itself would print.
fn run_landmark(bin: &str, args: &[&str]) -> Result<Value, String> {
    let output = Command::new(bin)
        .args(args)
        .output()
        .map_err(|error| format!("failed to run `{bin} {}`: {error}", args.join(" ")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(if stderr.trim().is_empty() {
            format!("`{bin} {}` exited with {}", args.join(" "), output.status)
        } else {
            stderr
        });
    }

    let structured = serde_json::from_str::<Value>(&stdout).unwrap_or(Value::Null);
    Ok(tool_result(stdout, structured))
}

fn tool_result(text: String, structured_content: Value) -> Value {
    json!({
        "content": [{"type": "text", "text": text}],
        "structuredContent": structured_content,
        "isError": false,
    })
}

fn tool_error(message: String) -> Value {
    json!({
        "content": [{"type": "text", "text": message}],
        "isError": true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_list_exposes_landmarks_core_verbs() {
        let tools = tool_defs_json();
        let names = tools
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, ["describe", "run_dry_run", "doctor"]);
    }

    #[test]
    fn unknown_tool_is_rejected() {
        let error = call_tool("synthesize", &json!({})).unwrap_err();
        assert!(error.contains("unknown tool"), "{error}");
    }

    #[test]
    fn missing_binary_reports_a_clear_error() {
        let error = call_tool(
            "describe",
            &json!({"landmark_bin": "definitely-not-a-real-binary-on-this-machine"}),
        )
        .unwrap_err();
        assert!(error.contains("failed to run"), "{error}");
    }

    #[test]
    fn json_rpc_wraps_success_and_error_outputs() {
        let list = handle_json_rpc(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
        }))
        .expect("tools/list response");
        assert_eq!(list["result"]["tools"][0]["name"], "describe");

        let error = handle_json_rpc(&json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {"name": "unknown", "arguments": {}},
        }))
        .expect("tool error response");
        assert_eq!(error["result"]["isError"], true);

        let protocol_error = handle_json_rpc(&json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "missing/method",
        }))
        .expect("protocol error response");
        assert_eq!(protocol_error["error"]["code"], -32603);
    }

    #[test]
    fn run_dry_run_against_the_real_workspace_binary_returns_a_version_decision() {
        // Exercises the actual compiled `landmark` binary, not a mock or an
        // assumption about its output shape. Looks for a sibling binary next
        // to this test binary's own `target/{debug,release}` dir (built by
        // `cargo build -p landmark` or `cargo test --workspace` beforehand);
        // skips cleanly rather than failing the suite when it isn't there.
        let Some(bin) = sibling_landmark_binary() else {
            eprintln!(
                "skipping: no built `landmark` binary found alongside landmark-mcp's own target dir"
            );
            return;
        };
        let repo_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let result = run_dry_run(&json!({
            "repo_root": repo_root,
            "landmark_bin": bin.to_string_lossy(),
        }));
        let value = result.expect("run --dry-run should succeed against this repo");
        let structured = &value["structuredContent"];
        assert!(structured["version_decision"]["bump"].is_string());
        assert!(structured["release_kit"].is_object());
    }

    fn sibling_landmark_binary() -> Option<std::path::PathBuf> {
        let mut dir = std::env::current_exe().ok()?;
        for _ in 0..6 {
            dir.pop();
            for profile in ["release", "debug"] {
                if dir.ends_with(profile) {
                    let candidate = dir.join("landmark");
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }
}
