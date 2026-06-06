use std::path::{Path, PathBuf};

use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolContext, ToolModeGate, ToolPack,
    ToolPackPolicy, ToolPolicy, ToolResult, ToolSummaryArgPath, ToolSummaryOutputPaths,
    ToolSummaryPolicy,
};
use agent_sandbox::PathPolicy;
use async_trait::async_trait;
use regex::Regex;
use serde_json::{Value, json};

use crate::tool::{AgentTool, default_finish_summary};

pub struct InspectGradleDependenciesTool;

#[async_trait]
impl AgentTool for InspectGradleDependenciesTool {
    fn name(&self) -> &'static str {
        "inspect_gradle_dependencies"
    }

    fn description(&self) -> &'static str {
        "Inspect Gradle dependency files deterministically. Returns structured dependency declarations, version-catalog presence, and relevant file ranges without loading broad project context."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Optional relative subdirectory to inspect; defaults to the project root."
                },
                "max_files": {
                    "type": "integer",
                    "default": 24,
                    "description": "Maximum Gradle/catalog files to inspect."
                }
            }
        })
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy {
            mode_gate: ToolModeGate::ReadFiles,
            pack_policy: ToolPackPolicy::Only(vec![ToolPack::Dependency, ToolPack::General]),
            summary: ToolSummaryPolicy {
                arg_paths: vec![ToolSummaryArgPath {
                    field: "path".into(),
                    ..ToolSummaryArgPath::default()
                }],
                output_paths: Some(ToolSummaryOutputPaths {
                    array_field: "files".into(),
                    path_field: "path".into(),
                }),
                ..ToolSummaryPolicy::default()
            },
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::Checklist
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Inspecting Gradle dependencies".into()
        } else {
            "Inspect Gradle dependencies".into()
        }
    }

    fn row_label(&self, command: Option<&str>, running: bool) -> String {
        let label = self.label(running);
        command
            .filter(|c| !c.is_empty() && *c != "{}")
            .map(|path| format!("{label} in {path}"))
            .unwrap_or(label)
    }

    fn args_preview(&self, args: &Value) -> String {
        args.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    }

    fn finish_summary(&self, _args: &Value, output: &str, is_error: bool) -> String {
        if is_error {
            return default_finish_summary(&self.label(false), output, true);
        }
        match serde_json::from_str::<Value>(output) {
            Ok(value) => {
                let file_count = value
                    .get("files")
                    .and_then(|v| v.as_array())
                    .map(|v| v.len())
                    .unwrap_or(0);
                let dep_count = value
                    .get("dependencies")
                    .and_then(|v| v.as_array())
                    .map(|v| v.len())
                    .unwrap_or(0);
                format!(
                    "Inspected {file_count} Gradle files and found {dep_count} dependency declarations"
                )
            }
            Err(_) => "Inspected Gradle dependencies".into(),
        }
    }

    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        let base = args
            .get("path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let policy = PathPolicy::new(&ctx.project_root);
        let resolved = policy.validate_read(&base)?;
        Ok(ToolAssessment {
            risk: RiskLevel::SafeRead,
            requires_approval: false,
            reason: "read-only Gradle dependency inspection".into(),
            affected_paths: vec![resolved],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        let base = args
            .get("path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let max_files = args
            .get("max_files")
            .and_then(|v| v.as_u64())
            .unwrap_or(24)
            .clamp(1, 100) as usize;
        let policy = PathPolicy::new(&ctx.project_root);
        let resolved = policy.validate_read(&base)?;
        let mut files = Vec::new();
        collect_gradle_files(&ctx.project_root, &resolved, &mut files, 0);
        files.sort();
        files.dedup();
        files.truncate(max_files);

        let mut file_rows = Vec::new();
        let mut deps = Vec::new();
        let mut version_catalog_present = false;
        for path in &files {
            let rel = path
                .strip_prefix(&ctx.project_root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            let lines = content.lines().count();
            if rel.ends_with("libs.versions.toml") {
                version_catalog_present = true;
            }
            file_rows.push(json!({
                "path": rel,
                "lines": lines,
                "kind": gradle_file_kind(path),
            }));
            deps.extend(extract_dependencies(&rel, &content));
        }

        let output = json!({
            "build_system": "gradle",
            "version_catalog_present": version_catalog_present,
            "files": file_rows,
            "dependencies": deps,
            "truncated": files.len() >= max_files,
        });
        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output: serde_json::to_string_pretty(&output).map_err(|e| e.to_string())?,
            is_error: false,
        })
    }
}

fn collect_gradle_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 6 || out.len() >= 128 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.starts_with(root) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.')
            || matches!(
                name.as_str(),
                "target" | "build" | "node_modules" | ".gradle" | ".idea"
            )
        {
            continue;
        }
        if path.is_dir() {
            collect_gradle_files(root, &path, out, depth + 1);
            continue;
        }
        if is_gradle_context_file(&name) {
            out.push(path);
        }
    }
}

fn is_gradle_context_file(name: &str) -> bool {
    matches!(
        name,
        "build.gradle"
            | "build.gradle.kts"
            | "settings.gradle"
            | "settings.gradle.kts"
            | "gradle.properties"
            | "libs.versions.toml"
    )
}

fn gradle_file_kind(path: &Path) -> &'static str {
    match path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
    {
        "libs.versions.toml" => "version_catalog",
        "gradle.properties" => "properties",
        "settings.gradle" | "settings.gradle.kts" => "settings",
        _ => "build_script",
    }
}

fn extract_dependencies(path: &str, content: &str) -> Vec<Value> {
    let notation = Regex::new(
        r#"(implementation|api|compileOnly|runtimeOnly|testImplementation|androidTestImplementation|kapt|ksp)\s*\(?\s*["']([^:"']+):([^:"']+):([^"']+)["']"#,
    )
    .expect("valid dependency regex");
    let catalog =
        Regex::new(r#"^\s*([A-Za-z0-9_.-]+)\s*=\s*\{\s*module\s*=\s*["']([^:"']+):([^"']+)["']"#)
            .expect("valid catalog regex");
    let alias = Regex::new(
        r#"(implementation|api|compileOnly|runtimeOnly|testImplementation|androidTestImplementation|kapt|ksp)\s*\(?\s*libs\.([A-Za-z0-9_.]+)"#,
    )
    .expect("valid alias regex");

    let mut deps = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let line_number = idx + 1;
        if let Some(caps) = notation.captures(line) {
            deps.push(json!({
                "file": path,
                "line": line_number,
                "configuration": caps.get(1).map(|m| m.as_str()).unwrap_or_default(),
                "group": caps.get(2).map(|m| m.as_str()).unwrap_or_default(),
                "name": caps.get(3).map(|m| m.as_str()).unwrap_or_default(),
                "version": caps.get(4).map(|m| m.as_str()).unwrap_or_default(),
                "source": "inline_notation",
            }));
            continue;
        }
        if let Some(caps) = catalog.captures(line) {
            deps.push(json!({
                "file": path,
                "line": line_number,
                "alias": caps.get(1).map(|m| m.as_str()).unwrap_or_default(),
                "group": caps.get(2).map(|m| m.as_str()).unwrap_or_default(),
                "name": caps.get(3).map(|m| m.as_str()).unwrap_or_default(),
                "source": "version_catalog",
            }));
            continue;
        }
        if let Some(caps) = alias.captures(line) {
            deps.push(json!({
                "file": path,
                "line": line_number,
                "configuration": caps.get(1).map(|m| m.as_str()).unwrap_or_default(),
                "alias": caps.get(2).map(|m| m.as_str()).unwrap_or_default(),
                "source": "catalog_reference",
            }));
        }
    }
    deps
}

#[cfg(test)]
mod tests {
    use super::extract_dependencies;

    #[test]
    fn extracts_inline_and_catalog_dependencies() {
        let src = r#"
dependencies {
    implementation("com.squareup.okio:okio:3.9.0")
    testImplementation(libs.junit)
}
junit = { module = "junit:junit", version.ref = "junit" }
"#;
        let deps = extract_dependencies("build.gradle.kts", src);
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0]["group"], "com.squareup.okio");
        assert_eq!(deps[1]["alias"], "junit");
        assert_eq!(deps[2]["source"], "version_catalog");
    }
}
