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

/// Keep the actual action in history; the server heading stays in the decision card.
pub fn approval_action_description(request: &str) -> String {
    let action = request
        .split_once(" requests permission\n\n")
        .map(|(_, action)| action)
        .unwrap_or(request);
    action.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod permission_tests {
    use super::*;

    #[test]
    fn permission_history_preserves_the_requested_action() {
        assert_eq!(
            approval_action_description(
                "computer-use requests permission\n\nView the browser screen\n\n• Screen access"
            ),
            "View the browser screen • Screen access"
        );
        assert_eq!(
            approval_action_description("Run cargo check"),
            "Run cargo check"
        );
    }
}
