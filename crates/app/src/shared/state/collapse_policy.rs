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
        (TranscriptMode::Summary, TimelineRowKind::UserMessage) => false,
        (TranscriptMode::Summary, TimelineRowKind::FinalSummary) => true,
        (TranscriptMode::Summary, TimelineRowKind::RunError) => true,
        (TranscriptMode::Summary, TimelineRowKind::ApprovalRequest) => true,
        (TranscriptMode::Summary, _) => false,

        (TranscriptMode::Normal, TimelineRowKind::RunError) => true,
        (TranscriptMode::Normal, TimelineRowKind::ApprovalRequest) => true,
        (TranscriptMode::Normal, TimelineRowKind::DiffSummary) => false,
        (TranscriptMode::Normal, TimelineRowKind::ToolCall) => false,
        (TranscriptMode::Normal, TimelineRowKind::ReasoningNote) => false,
        (TranscriptMode::Normal, _) => false,

        (TranscriptMode::Verbose, TimelineRowKind::ToolCall) => failed,
        (TranscriptMode::Verbose, TimelineRowKind::ReasoningNote) => false,
        (TranscriptMode::Verbose, TimelineRowKind::RunError) => true,
        (TranscriptMode::Verbose, _) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_hides_tool_expansion() {
        assert!(!default_expansion(
            TranscriptMode::Summary,
            TimelineRowKind::ToolCall,
            AgentStatus::Completed,
            false,
            false,
        ));
    }

    #[test]
    fn normal_expands_failures() {
        assert!(default_expansion(
            TranscriptMode::Normal,
            TimelineRowKind::ToolCall,
            AgentStatus::Failed,
            true,
            false,
        ));
    }

    #[test]
    fn normal_expands_waiting_approval() {
        assert!(default_expansion(
            TranscriptMode::Normal,
            TimelineRowKind::ToolCall,
            AgentStatus::WaitingApproval,
            false,
            false,
        ));
    }

    #[test]
    fn selection_overrides_mode() {
        assert!(default_expansion(
            TranscriptMode::Summary,
            TimelineRowKind::ToolCall,
            AgentStatus::Completed,
            false,
            true,
        ));
    }

    #[test]
    fn verbose_keeps_tools_collapsed_when_ok() {
        assert!(!default_expansion(
            TranscriptMode::Verbose,
            TimelineRowKind::ToolCall,
            AgentStatus::Completed,
            false,
            false,
        ));
    }
}
