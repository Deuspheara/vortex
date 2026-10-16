//! Collapse / expansion policy for timeline rows.

use super::TranscriptMode;
use crate::features::shell::state::AgentStatus;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimelineRowKind {
    UserMessage,
    AssistantMessage,
    FinalSummary,
    ReasoningNote,
    ToolCall,
    DiffSummary,
    ApprovalRequest,
    RunError,
    ChoiceRequest,
    PlanningStatus,
}

/// Whether a row should start expanded when rendered.
pub fn default_expansion(
    mode: TranscriptMode,
    kind: TimelineRowKind,
    status: AgentStatus,
    failed: bool,
    selected: bool,
) -> bool {
    if selected {
        return true;
    }
    if failed || matches!(status, AgentStatus::Failed | AgentStatus::WaitingApproval) {
        return true;
    }
    match (mode, kind) {
        (TranscriptMode::Focus, TimelineRowKind::UserMessage) => false,
        (TranscriptMode::Focus, TimelineRowKind::FinalSummary) => true,
        (TranscriptMode::Focus, TimelineRowKind::RunError) => true,
        (TranscriptMode::Focus, TimelineRowKind::ApprovalRequest) => true,
        (TranscriptMode::Focus, _) => false,

        (TranscriptMode::Activity, TimelineRowKind::RunError) => true,
        (TranscriptMode::Activity, TimelineRowKind::ApprovalRequest) => true,
        (TranscriptMode::Activity, TimelineRowKind::DiffSummary) => false,
        (TranscriptMode::Activity, TimelineRowKind::ToolCall) => false,
        (TranscriptMode::Activity, TimelineRowKind::ReasoningNote) => false,
        (TranscriptMode::Activity, _) => false,

        (TranscriptMode::Debug, TimelineRowKind::ToolCall) => failed,
        (TranscriptMode::Debug, TimelineRowKind::ReasoningNote) => false,
        (TranscriptMode::Debug, TimelineRowKind::RunError) => true,
        (TranscriptMode::Debug, _) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_hides_tool_expansion() {
        assert!(!default_expansion(
            TranscriptMode::Focus,
            TimelineRowKind::ToolCall,
            AgentStatus::Completed,
            false,
            false,
        ));
    }

    #[test]
    fn activity_expands_failures() {
        assert!(default_expansion(
            TranscriptMode::Activity,
            TimelineRowKind::ToolCall,
            AgentStatus::Failed,
            true,
            false,
        ));
    }

    #[test]
    fn activity_expands_waiting_approval() {
        assert!(default_expansion(
            TranscriptMode::Activity,
            TimelineRowKind::ToolCall,
            AgentStatus::WaitingApproval,
            false,
            false,
        ));
    }

    #[test]
    fn selection_overrides_mode() {
        assert!(default_expansion(
            TranscriptMode::Focus,
            TimelineRowKind::ToolCall,
            AgentStatus::Completed,
            false,
            true,
        ));
    }

    #[test]
    fn debug_keeps_tools_collapsed_when_ok() {
        assert!(!default_expansion(
            TranscriptMode::Debug,
            TimelineRowKind::ToolCall,
            AgentStatus::Completed,
            false,
            false,
        ));
    }
}
