use serde::{Deserialize, Serialize};

use crate::ToolPack;

/// UI-agnostic icon token; mapped to GPUI icons in the app layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IconToken {
    File,
    Folder,
    Search,
    Terminal,
    Pencil,
    GitCompare,
    Bot,
    Checklist,
    Globe,
    Question,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ToolCategory {
    Read,
    Search,
    PatchProposal,
    PatchApply,
    VirtualCommand,
    RealCommand,
    AskUser,
    Delegate,
    #[default]
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ToolCapabilities {
    pub category: ToolCategory,
    pub parallel_safe: bool,
    pub cache_output: bool,
    pub persist_result_body: bool,
    pub suppress_live_output: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ToolModeGate {
    #[default]
    ReadFiles,
    ProposePatches,
    ApplyPatches,
    RunVirtualBash,
    RunRealCommands,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ToolRepoRequirement {
    #[default]
    Any,
    GitRepository,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ToolNestingPolicy {
    #[default]
    AnyDepth,
    RootRunOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ToolPackPolicy {
    #[default]
    All,
    Only(Vec<ToolPack>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ToolRuntimeFamily {
    #[default]
    Standard,
    AndroidDevice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum AndroidLanePolicy {
    #[default]
    None,
    Observe,
    Action,
    Utility,
    DenyInAgentMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ToolSummaryArgPathKind {
    #[default]
    PlainPath,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ToolSummaryArgPath {
    pub field: String,
    #[serde(default)]
    pub kind: ToolSummaryArgPathKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ToolSummaryArgRange {
    pub path_field: String,
    pub start_line_field: String,
    pub end_line_field: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ToolSummaryOutputPaths {
    pub array_field: String,
    pub path_field: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ToolSummaryPolicy {
    #[serde(default)]
    pub prefer_line_bounded_follow_up: bool,
    #[serde(default)]
    pub arg_paths: Vec<ToolSummaryArgPath>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arg_range: Option<ToolSummaryArgRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_paths: Option<ToolSummaryOutputPaths>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ToolPolicy {
    pub mode_gate: ToolModeGate,
    #[serde(default)]
    pub pack_policy: ToolPackPolicy,
    #[serde(default)]
    pub repo_requirement: ToolRepoRequirement,
    #[serde(default)]
    pub nesting: ToolNestingPolicy,
    #[serde(default)]
    pub runtime_family: ToolRuntimeFamily,
    #[serde(default)]
    pub android_lane: AndroidLanePolicy,
    #[serde(default)]
    pub summary: ToolSummaryPolicy,
}

/// Static presentation metadata for a tool (no GPUI dependencies).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub icon: IconToken,
    #[serde(default)]
    pub capabilities: ToolCapabilities,
    #[serde(default)]
    pub policy: ToolPolicy,
}
