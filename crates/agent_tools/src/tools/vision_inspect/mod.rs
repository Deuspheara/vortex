use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_protocol::{
    IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolContext, ToolModeGate, ToolPack,
    ToolPackPolicy, ToolPolicy, ToolResult,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::shared::provenance::{Confidence, SourceProvenance};
use crate::shared::vision::{VisionInspectRequest, VisionPort};
use crate::shared::web::structured_output;
use crate::tool::{AgentTool, default_finish_summary};

pub struct VisionInspectTool {
    pub port: Arc<dyn VisionPort>,
    pub artifact_dir: PathBuf,
}

#[async_trait]
impl AgentTool for VisionInspectTool {
    fn name(&self) -> &'static str {
        "vision_inspect"
    }

    fn description(&self) -> &'static str {
        "Inspect an existing screenshot/image through a provider-agnostic vision backend. Returns structured observations with uncertainty."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "image_path": { "type": "string" },
                "question": { "type": "string" },
                "context": {},
                "output_schema": {}
            },
            "required": ["image_path", "question"]
        })
    }

    fn policy(&self) -> ToolPolicy {
        ToolPolicy {
            mode_gate: ToolModeGate::ReadFiles,
            pack_policy: ToolPackPolicy::Only(vec![ToolPack::UiBrowser, ToolPack::General]),
            ..ToolPolicy::default()
        }
    }

    fn icon(&self) -> IconToken {
        IconToken::Globe
    }

    fn label(&self, running: bool) -> String {
        if running {
            "Inspecting image".into()
        } else {
            "Inspected image".into()
        }
    }

    fn args_preview(&self, args: &Value) -> String {
        let path = args
            .get("image_path")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let question = args
            .get("question")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        format!("{path} {question}").chars().take(120).collect()
    }

    fn finish_summary(&self, _args: &Value, output: &str, is_error: bool) -> String {
        if is_error {
            return default_finish_summary(&self.label(false), output, true);
        }
        "Inspected image".into()
    }

    async fn assess(&self, args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        let image_path = args
            .get("image_path")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if image_path.is_empty() {
            return Ok(denied("image_path is required"));
        }
        if !path_is_allowed(Path::new(image_path), &ctx.project_root, &self.artifact_dir) {
            return Ok(denied(
                "image_path must be inside the project root or controlled artifact cache",
            ));
        }
        Ok(ToolAssessment {
            risk: RiskLevel::SafeRead,
            requires_approval: false,
            reason: "reads a local image artifact for visual inspection".into(),
            affected_paths: vec![PathBuf::from(image_path)],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> Result<ToolResult, String> {
        let request = VisionInspectRequest {
            image_path: args
                .get("image_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing image_path".to_string())?
                .to_string(),
            question: args
                .get("question")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing question".to_string())?
                .to_string(),
            context: args.get("context").cloned(),
            output_schema: args.get("output_schema").cloned(),
        };
        let response = self.port.inspect(request).await;
        let (data, is_error) = match response {
            Ok(response) => (
                json!({
                    "summary": response.summary,
                    "observations": response.observations,
                    "uncertain": response.uncertain,
                    "suggested_next_action": response.suggested_next_action,
                    "confidence": Confidence::Medium,
                    "warnings": [],
                    "provenance": SourceProvenance::new(None, None, "vision", "image_inspect"),
                    "untrusted": true
                }),
                false,
            ),
            Err(reason) => (
                json!({
                    "summary": "Vision inspection unavailable",
                    "observations": [],
                    "uncertain": [reason],
                    "suggested_next_action": "Configure a VisionPort backend or use browser_snapshot/web_fetch if text is available.",
                    "confidence": Confidence::Low,
                    "warnings": ["no vision backend configured"],
                    "provenance": SourceProvenance::new(None, None, "vision", "image_inspect"),
                    "untrusted": true
                }),
                true,
            ),
        };
        let output = structured_output("Vision inspection result", data)?;
        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output,
            is_error,
        })
    }
}

fn path_is_allowed(path: &Path, project_root: &Path, artifact_dir: &Path) -> bool {
    let Ok(path) = path.canonicalize() else {
        return false;
    };
    let project = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let artifacts = artifact_dir
        .canonicalize()
        .unwrap_or_else(|_| artifact_dir.to_path_buf());
    path.starts_with(project) || path.starts_with(artifacts)
}

fn denied(reason: &str) -> ToolAssessment {
    ToolAssessment {
        risk: RiskLevel::High,
        requires_approval: false,
        reason: reason.into(),
        affected_paths: vec![],
        network_access: NetworkAccess::Disabled,
        writes_to_disk: false,
        runs_real_process: false,
        denied: true,
    }
}
