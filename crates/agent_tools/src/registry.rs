use std::path::PathBuf;
use std::sync::Arc;

use agent_protocol::{
    AgentMode, ApprovalDecision, ApprovalId, PendingToolCall, RiskLevel, ToolCallId,
    ToolCapabilities, ToolCategory, ToolContext, ToolDescriptor, ToolModeGate, ToolPack,
    ToolPackPolicy, ToolPolicy, ToolRepoRequirement, ToolResult, ToolRuntimeFamily, ToolStatus,
};
use agent_sandbox::ApprovalEngine;
use agent_store::{EventStore, StoredToolCall, storage_tool_call_id};
use chrono::Utc;
use serde_json::Value;

use crate::shared::{BrowserSidecarClient, UnconfiguredVisionPort};
use crate::tool::AgentTool;
use crate::tools::{
    AndroidCliDocsFetchTool, AndroidCliDocsSearchTool, AndroidCliDoctorTool, AndroidCliInfoTool,
    AndroidCliRunTool, AndroidCliTestJourneyTool, AndroidEnsureEmulatorTool, AndroidLaunchAppTool,
    AndroidObserveTool, AndroidPressBackTool, AndroidPressHomeTool, AndroidReadLogcatTool,
    AndroidSwipeTool, AndroidTapPointTool, AndroidTapResourceIdTool, AndroidTapTextTool,
    AndroidTypeTextTool, ApplyPatchTool, AskUserTool, BashVirtualTool, BrowserScreenshotTool,
    BrowserSnapshotTool, DelegateTool, DeleteFileTool, EditFileTool, FetchUrlTool, FindSymbolTool,
    GitDiffTool, GitStatusTool, InspectGradleDependenciesTool, ListFilesTool, OpenNodeTool,
    ProposePatchTool, ReadFileTool, RelatedFilesTool, RepoMapTool, RunRealCommandTool,
    SearchProjectTool, TodoWriteTool, VisionInspectTool, WebExtractTool, WebFetchTool,
    WebSearchTool, WriteFileTool,
};

pub fn default_tool_specs(tools: &[Box<dyn AgentTool>]) -> Vec<agent_protocol::ToolSpec> {
    tools
        .iter()
        .map(|tool| agent_protocol::ToolSpec {
            name: tool.name().to_string(),
            description: tool.description().to_string(),
            parameters: tool.schema(),
            policy: tool.policy(),
        })
        .collect()
}

/// Select the tool specs that should be exposed to the model for a given run context.
///
/// This is the single source of truth shared by the runtime loop and the eval harness so
/// that token measurements reflect exactly what the model receives.
///
/// Filtering rules:
/// - `delegate` is only available at depth 0 (no nested subagents).
/// - git tools are dropped outside git repositories.
/// - tools are gated by [`AgentMode`] capabilities, so read-only modes never advertise
///   write/exec/network tools (saves tokens and prevents wrong-tool selection).
pub fn mode_visible_tool_specs(
    specs: Vec<agent_protocol::ToolSpec>,
    mode: &AgentMode,
    depth: u8,
    is_git_repo: bool,
) -> Vec<agent_protocol::ToolSpec> {
    specs
        .into_iter()
        .filter(|spec| {
            if depth > 0
                && matches!(
                    spec.policy.nesting,
                    agent_protocol::ToolNestingPolicy::RootRunOnly
                )
            {
                return false;
            }
            if !is_git_repo
                && matches!(
                    spec.policy.repo_requirement,
                    ToolRepoRequirement::GitRepository
                )
            {
                return false;
            }
            tool_allowed_in_mode(&spec.policy, mode)
        })
        .collect()
}

pub fn tool_pack_for_task(task_class: agent_protocol::TaskClass) -> ToolPack {
    match task_class {
        agent_protocol::TaskClass::DependencyUpdate => ToolPack::Dependency,
        agent_protocol::TaskClass::UiChange => ToolPack::UiBrowser,
        agent_protocol::TaskClass::TestFailure => ToolPack::GitCi,
        agent_protocol::TaskClass::ArchitectureQuestion => ToolPack::Planning,
        agent_protocol::TaskClass::BugFix | agent_protocol::TaskClass::Refactor => {
            ToolPack::CodeEdit
        }
        agent_protocol::TaskClass::Unknown => ToolPack::General,
    }
}

pub fn task_visible_tool_specs(
    specs: Vec<agent_protocol::ToolSpec>,
    mode: &AgentMode,
    depth: u8,
    is_git_repo: bool,
    pack: ToolPack,
) -> Vec<agent_protocol::ToolSpec> {
    mode_visible_tool_specs(specs, mode, depth, is_git_repo)
        .into_iter()
        .filter(|spec| tool_allowed_in_pack(&spec.policy, pack))
        .collect()
}

pub fn tool_allowed_in_pack(policy: &ToolPolicy, pack: ToolPack) -> bool {
    match &policy.pack_policy {
        ToolPackPolicy::All => true,
        ToolPackPolicy::Only(packs) => packs.contains(&pack),
    }
}

/// Whether a tool should be advertised to the model in the given mode.
///
/// Read-only modes never see write/exec tools; this both saves prompt tokens and prevents the
/// model from selecting a tool that would only be denied at execution time. Execution-time
/// gating still lives in each tool's `assess()` (defence in depth).
pub fn tool_allowed_in_mode(policy: &ToolPolicy, mode: &AgentMode) -> bool {
    match policy.mode_gate {
        ToolModeGate::ReadFiles => mode.can_read_files(),
        ToolModeGate::ProposePatches => mode.can_propose_patches(),
        ToolModeGate::ApplyPatches => mode.can_apply_patches(),
        ToolModeGate::RunVirtualBash => mode.can_run_virtual_bash(),
        ToolModeGate::RunRealCommands => mode.can_run_real_commands(),
    }
}

fn tool_call_dedupe_key_from_policy(policy: &ToolPolicy, args: &Value) -> Option<String> {
    if let Some(range) = policy.summary.arg_range.as_ref() {
        let path = args
            .get(&range.path_field)
            .and_then(|v| v.as_str())?
            .to_string();
        let start = args
            .get(&range.start_line_field)
            .and_then(|v| v.as_u64())
            .map(|v| v.to_string())
            .unwrap_or_default();
        let end = args
            .get(&range.end_line_field)
            .and_then(|v| v.as_u64())
            .map(|v| v.to_string())
            .unwrap_or_default();
        return Some(format!("{path}:{start}:{end}"));
    }

    let mut parts = Vec::new();
    for entry in &policy.summary.arg_paths {
        let value = args.get(&entry.field).and_then(|v| v.as_str())?;
        parts.push(format!("{}={value}", entry.field));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("|"))
    }
}

pub struct ToolRegistry {
    tools: Vec<Box<dyn AgentTool>>,
}

impl ToolRegistry {
    pub fn new(checkpoint_dir: PathBuf, sidecar_entry: PathBuf) -> Self {
        let browser_entry = browser_sidecar_entry(&sidecar_entry);
        let browser_client = Arc::new(BrowserSidecarClient::new(browser_entry));
        let web_artifact_dir = checkpoint_dir.join("web_artifacts");
        let screenshot_dir = web_artifact_dir.join("screenshots");
        let vision_port = Arc::new(UnconfiguredVisionPort);
        let tools: Vec<Box<dyn AgentTool>> = vec![
            Box::new(ReadFileTool),
            Box::new(ListFilesTool),
            Box::new(SearchProjectTool),
            Box::new(RepoMapTool),
            Box::new(FindSymbolTool),
            Box::new(OpenNodeTool),
            Box::new(RelatedFilesTool),
            Box::new(InspectGradleDependenciesTool),
            Box::new(GitStatusTool),
            Box::new(GitDiffTool),
            Box::new(AndroidEnsureEmulatorTool),
            Box::new(AndroidObserveTool),
            Box::new(AndroidTapTextTool),
            Box::new(AndroidTapResourceIdTool),
            Box::new(AndroidTapPointTool),
            Box::new(AndroidTypeTextTool),
            Box::new(AndroidSwipeTool),
            Box::new(AndroidPressBackTool),
            Box::new(AndroidPressHomeTool),
            Box::new(AndroidLaunchAppTool),
            Box::new(AndroidReadLogcatTool),
            Box::new(AndroidCliDoctorTool),
            Box::new(AndroidCliInfoTool),
            Box::new(AndroidCliRunTool),
            Box::new(AndroidCliTestJourneyTool),
            Box::new(AndroidCliDocsSearchTool),
            Box::new(AndroidCliDocsFetchTool),
            Box::new(BashVirtualTool),
            Box::new(ProposePatchTool),
            Box::new(ApplyPatchTool { checkpoint_dir }),
            Box::new(EditFileTool),
            Box::new(WriteFileTool),
            Box::new(DeleteFileTool),
            Box::new(TodoWriteTool),
            Box::new(AskUserTool),
            Box::new(FetchUrlTool),
            Box::new(WebSearchTool),
            Box::new(WebFetchTool),
            Box::new(WebExtractTool),
            Box::new(BrowserSnapshotTool {
                client: browser_client.clone(),
            }),
            Box::new(BrowserScreenshotTool {
                client: browser_client,
                artifact_dir: screenshot_dir.clone(),
            }),
            Box::new(VisionInspectTool {
                port: vision_port,
                artifact_dir: screenshot_dir,
            }),
            Box::new(RunRealCommandTool),
            Box::new(DelegateTool),
        ];
        Self { tools }
    }

    pub fn tool_specs(&self) -> Vec<agent_protocol::ToolSpec> {
        default_tool_specs(&self.tools)
    }

    pub fn catalog(&self) -> Vec<ToolDescriptor> {
        self.tools.iter().map(|t| t.descriptor()).collect()
    }

    pub fn descriptor(&self, name: &str) -> Option<ToolDescriptor> {
        self.get(name).map(|t| t.descriptor())
    }

    pub fn capabilities(&self, name: &str) -> ToolCapabilities {
        self.get(name).map(|t| t.capabilities()).unwrap_or_default()
    }

    pub fn has_category(&self, name: &str, category: ToolCategory) -> bool {
        self.capabilities(name).category == category
    }

    pub fn policy(&self, name: &str) -> ToolPolicy {
        self.get(name).map(|t| t.policy()).unwrap_or_default()
    }

    pub fn is_parallel_safe(&self, name: &str) -> bool {
        self.capabilities(name).parallel_safe
    }

    pub fn caches_output(&self, name: &str) -> bool {
        self.capabilities(name).cache_output
    }

    pub fn persists_result_body(&self, name: &str) -> bool {
        self.capabilities(name).persist_result_body
    }

    pub fn suppresses_live_output(&self, name: &str) -> bool {
        self.capabilities(name).suppress_live_output
    }

    pub fn is_android_tool(&self, name: &str) -> bool {
        matches!(
            self.policy(name).runtime_family,
            ToolRuntimeFamily::AndroidDevice
        )
    }

    pub fn get(&self, name: &str) -> Option<&dyn AgentTool> {
        let name = normalized_tool_name(name);
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .map(|t| t.as_ref())
    }

    pub fn tool_label(&self, name: &str, running: bool) -> String {
        let name = normalized_tool_name(name);
        self.get(name).map(|t| t.label(running)).unwrap_or_else(|| {
            if running {
                format!("Running {name}")
            } else {
                name.to_string()
            }
        })
    }

    pub fn row_label(&self, name: &str, command: Option<&str>, running: bool) -> String {
        let name = normalized_tool_name(name);
        let cleaned = command.and_then(crate::shared::sanitize_display_arg);
        self.get(name)
            .map(|t| t.row_label(cleaned.as_deref(), running))
            .unwrap_or_else(|| {
                if running {
                    format!("Running {name}")
                } else {
                    name.to_string()
                }
            })
    }

    pub fn tool_finish_summary(
        &self,
        name: &str,
        args: &Value,
        output: &str,
        is_error: bool,
    ) -> String {
        let name = normalized_tool_name(name);
        self.get(name)
            .map(|t| t.finish_summary(args, output, is_error))
            .unwrap_or_else(|| name.to_string())
    }

    pub fn args_preview(&self, name: &str, args: &Value) -> String {
        let name = normalized_tool_name(name);
        self.get(name)
            .map(|t| t.args_preview(args))
            .unwrap_or_default()
    }

    pub fn tool_call_dedupe_key(&self, name: &str, args: &Value) -> Option<String> {
        tool_call_dedupe_key_from_policy(&self.policy(name), args)
    }

    pub fn streaming_tool_call_dedupe_key(&self, name: &str, raw_json: &str) -> Option<String> {
        let name = normalized_tool_name(name);
        let tool = self.get(name)?;
        let partial = crate::shared::partial_args_from_schema(&tool.schema(), raw_json)?;
        self.tool_call_dedupe_key(name, &partial)
    }

    pub fn output_cache_key(&self, name: &str, args: &Value) -> Option<String> {
        self.tool_call_dedupe_key(name, args)
    }

    pub async fn assess(
        &self,
        name: &str,
        args: &Value,
        ctx: &ToolContext,
    ) -> Result<agent_protocol::ToolAssessment, String> {
        let name = normalized_tool_name(name);
        let tool = self
            .get(name)
            .ok_or_else(|| format!("unknown tool: {name}"))?;
        tool.assess(args, ctx).await
    }

    pub async fn execute(
        &self,
        name: &str,
        args: Value,
        ctx: ToolContext,
        call_id: &ToolCallId,
    ) -> Result<ToolResult, String> {
        let name = normalized_tool_name(name);
        let tool = self
            .get(name)
            .ok_or_else(|| format!("unknown tool: {name}"))?;
        let mut result = tool.execute(args, ctx).await?;
        result.call_id = call_id.clone();
        Ok(result)
    }
}

fn normalized_tool_name(name: &str) -> &str {
    name.split("<|")
        .next()
        .unwrap_or(name)
        .split_whitespace()
        .next()
        .unwrap_or(name)
}

fn browser_sidecar_entry(sidecar_entry: &std::path::Path) -> PathBuf {
    sidecar_entry
        .ancestors()
        .nth(2)
        .and_then(|sidecar_dir| sidecar_dir.parent())
        .map(|sidecars| sidecars.join("browser_worker/src/main.ts"))
        .unwrap_or_else(|| PathBuf::from("sidecars/browser_worker/src/main.ts"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_pack_includes_gradle_inspector_and_excludes_browser() {
        let registry = ToolRegistry::new(
            PathBuf::from("/tmp/checkpoints"),
            PathBuf::from("/tmp/sidecars/browser_worker/src/main.rs"),
        );
        assert!(tool_allowed_in_pack(
            &registry.policy("inspect_gradle_dependencies"),
            ToolPack::Dependency
        ));
        assert!(tool_allowed_in_pack(
            &registry.policy("search_project"),
            ToolPack::Dependency
        ));
        assert!(!tool_allowed_in_pack(
            &registry.policy("browser_screenshot"),
            ToolPack::Dependency
        ));
    }

    #[test]
    fn task_visible_specs_layer_pack_after_mode() {
        let registry = ToolRegistry::new(
            PathBuf::from("/tmp/checkpoints"),
            PathBuf::from("/tmp/sidecars/browser_worker/src/main.rs"),
        );
        let specs = vec![
            agent_protocol::ToolSpec {
                name: "inspect_gradle_dependencies".into(),
                description: String::new(),
                parameters: serde_json::json!({}),
                policy: registry.policy("inspect_gradle_dependencies"),
            },
            agent_protocol::ToolSpec {
                name: "browser_screenshot".into(),
                description: String::new(),
                parameters: serde_json::json!({}),
                policy: registry.policy("browser_screenshot"),
            },
            agent_protocol::ToolSpec {
                name: "edit_file".into(),
                description: String::new(),
                parameters: serde_json::json!({}),
                policy: registry.policy("edit_file"),
            },
        ];
        let visible = task_visible_tool_specs(
            specs,
            &AgentMode::ApplyWithApproval,
            0,
            true,
            ToolPack::Dependency,
        );
        let names: Vec<_> = visible.iter().map(|spec| spec.name.as_str()).collect();
        assert!(names.contains(&"inspect_gradle_dependencies"));
        assert!(names.contains(&"edit_file"));
        assert!(!names.contains(&"browser_screenshot"));
    }

    #[test]
    fn output_cache_key_uses_range_policy() {
        let registry = ToolRegistry::new(
            PathBuf::from("/tmp/checkpoints"),
            PathBuf::from("/tmp/sidecars/browser_worker/src/main.rs"),
        );
        let args = serde_json::json!({
            "path": "src/lib.rs",
            "start_line": 10,
            "end_line": 20,
        });
        assert_eq!(
            registry.output_cache_key("read_file", &args).as_deref(),
            Some("src/lib.rs:10:20")
        );
    }

    #[test]
    fn output_cache_key_falls_back_to_arg_paths() {
        let registry = ToolRegistry::new(
            PathBuf::from("/tmp/checkpoints"),
            PathBuf::from("/tmp/sidecars/browser_worker/src/main.rs"),
        );
        let args = serde_json::json!({"focus": "crates/app"});
        assert_eq!(
            registry.output_cache_key("repo_map", &args).as_deref(),
            Some("focus=crates/app")
        );
    }
}

pub struct ToolOrchestrator {
    pub registry: Arc<ToolRegistry>,
    pub store: Arc<dyn EventStore>,
}

#[derive(Clone, Debug)]
pub struct PendingApproval {
    pub approval_id: ApprovalId,
    pub call: PendingToolCall,
    pub decision: ApprovalDecision,
}

impl ToolOrchestrator {
    pub async fn classify(
        &self,
        mode: &AgentMode,
        tool_name: &str,
        args: &Value,
        project_id: &agent_protocol::ProjectId,
        ctx: &ToolContext,
    ) -> Result<ApprovalDecision, String> {
        let assessment = self.registry.assess(tool_name, args, ctx).await?;
        let rules = self
            .store
            .list_approval_rules(project_id)
            .unwrap_or_default();
        Ok(ApprovalEngine::decide(
            mode,
            tool_name,
            args,
            &assessment,
            &rules,
            None,
        ))
    }

    pub fn record_tool_start(
        &self,
        run_id: &agent_protocol::RunId,
        call_id: &ToolCallId,
        name: &str,
        args: &Value,
        risk: RiskLevel,
    ) -> Result<(), String> {
        self.store.record_tool_call(&StoredToolCall {
            id: storage_tool_call_id(run_id, call_id),
            run_id: run_id.clone(),
            name: name.to_string(),
            args_json: args.to_string(),
            risk,
            status: ToolStatus::Running,
            started_at: Utc::now(),
            finished_at: None,
        })
    }

    pub fn record_tool_finish(
        &self,
        run_id: &agent_protocol::RunId,
        call_id: &ToolCallId,
        status: ToolStatus,
    ) -> Result<(), String> {
        self.store.update_tool_call(
            &storage_tool_call_id(run_id, call_id),
            status,
            Some(Utc::now()),
        )
    }
}

pub fn args_preview(registry: &ToolRegistry, name: &str, args: &Value) -> String {
    registry.args_preview(name, args)
}

pub fn args_preview_raw(registry: &ToolRegistry, name: &str, json: &str) -> String {
    if json.is_empty() {
        return String::new();
    }
    if let Ok(value) = serde_json::from_str::<Value>(json) {
        return registry.args_preview(name, &value);
    }
    registry
        .get(name)
        .and_then(|tool| tool.streaming_args_preview(json))
        .unwrap_or_default()
}

pub fn tool_finish_summary(
    registry: &ToolRegistry,
    name: &str,
    args: &Value,
    output: &str,
    is_error: bool,
) -> String {
    registry.tool_finish_summary(name, args, output, is_error)
}

pub fn tool_label(registry: &ToolRegistry, name: &str, running: bool) -> String {
    registry.tool_label(name, running)
}
