#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityPhase {
    Explore,
    Edit,
    Run,
    Review,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SessionStatus {
    Running,
    Done,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum StepStatus {
    Running,
    Done,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum StepKind {
    Thought,
    Tool(String),
    Diff,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub struct SessionStep {
    pub item_ix: u32,
    pub kind: StepKind,
    pub label: String,
    pub detail: Option<String>,
    pub status: StepStatus,
    pub phase: ActivityPhase,
    pub depth: u8,
    pub parent_call_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub struct WorkSession {
    pub id: String,
    pub steps: Vec<SessionStep>,
    pub status: SessionStatus,
    pub collapsed: bool,
}

/// Heuristic phase from tool name until reducer emits explicit metadata.
pub fn phase_for_tool_name(tool_name: &str) -> ActivityPhase {
    match tool_name {
        "read_file" | "Read" | "search" | "grep" | "glob_file_search" | "codebase_search" => {
            ActivityPhase::Explore
        }
        "propose_patch" | "apply_patch" | "edit_file" | "write" => ActivityPhase::Edit,
        "bash_virtual" | "run_real_command" | "RunCommand" | "shell" | "cargo" | "test" => {
            ActivityPhase::Run
        }
        _ => ActivityPhase::Edit,
    }
}
