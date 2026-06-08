//! Timeline projection — normalized events from thread items.

use crate::features::agent_activity::state::{ActivityPhase, phase_for_tool_name};
use crate::features::shell::state::{AgentStatus, ThreadItem};
use crate::shared::state::TranscriptMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TimelineEventKind {
    UserMessage,
    AssistantMessage,
    FinalSummary,
    ReasoningNote,
    FileRead,
    FileSearch,
    FileEdit,
    CommandRun,
    SubagentRun,
    DiffSummary,
    ApprovalRequest,
    ChoiceRequest,
    Error,
    ContextTrace,
    TodoUpdate,
    PlanCreated,
    PlanUpdated,
    PlanningStatus,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct TimelineEvent {
    pub id: String,
    pub item_ix: u32,
    pub kind: TimelineEventKind,
    pub label: String,
    pub detail: Option<String>,
    pub phase: ActivityPhase,
    pub status: AgentStatus,
    pub artifact_id: Option<String>,
    pub additions: Option<usize>,
    pub deletions: Option<usize>,
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct TaskViewModel {
    pub goal: String,
    pub timeline: Vec<TimelineEvent>,
}

pub fn project_timeline(items: &[ThreadItem], mode: TranscriptMode) -> Vec<TimelineEvent> {
    let mut events = Vec::new();
    for (item_ix, item) in items.iter().enumerate() {
        if let Some(event) = project_item(item_ix as u32, item, mode, items) {
            events.push(event);
        }
    }
    events
}

pub fn build_task_view(title: &str, items: &[ThreadItem], mode: TranscriptMode) -> TaskViewModel {
    let goal = if title.is_empty() {
        items
            .iter()
            .find_map(|item| {
                if let ThreadItem::UserMessage { text, .. } = item {
                    Some(
                        text.lines()
                            .next()
                            .unwrap_or(text)
                            .chars()
                            .take(80)
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "New task".into())
    } else {
        title.to_string()
    };
    TaskViewModel {
        goal,
        timeline: project_timeline(items, mode),
    }
}

fn project_item(
    item_ix: u32,
    item: &ThreadItem,
    mode: TranscriptMode,
    all_items: &[ThreadItem],
) -> Option<TimelineEvent> {
    match item {
        ThreadItem::UserMessage { id, text, .. } => Some(TimelineEvent {
            id: id.clone(),
            item_ix,
            kind: TimelineEventKind::UserMessage,
            label: text
                .lines()
                .next()
                .unwrap_or(text)
                .chars()
                .take(80)
                .collect(),
            detail: None,
            phase: ActivityPhase::Explore,
            status: AgentStatus::Completed,
            artifact_id: None,
            additions: None,
            deletions: None,
        }),
        ThreadItem::AssistantMessage { id, streaming, .. } => {
            let is_final =
                crate::features::chat::state::is_final_assistant(item_ix as usize, all_items);
            let kind = if is_final && mode == TranscriptMode::Summary {
                TimelineEventKind::FinalSummary
            } else {
                TimelineEventKind::AssistantMessage
            };
            Some(TimelineEvent {
                id: id.clone(),
                item_ix,
                kind,
                label: "Assistant".into(),
                detail: None,
                phase: ActivityPhase::Review,
                status: if *streaming {
                    AgentStatus::Thinking
                } else {
                    AgentStatus::Completed
                },
                artifact_id: None,
                additions: None,
                deletions: None,
            })
        }
        ThreadItem::PlanStatus { .. } => None,
        ThreadItem::SubagentRun {
            id,
            task,
            model,
            status,
            ..
        } => Some(TimelineEvent {
            id: id.clone(),
            item_ix,
            kind: TimelineEventKind::SubagentRun,
            label: format!("Subagent: {task}"),
            detail: Some(model.clone()),
            phase: ActivityPhase::Run,
            status: status.clone(),
            artifact_id: None,
            additions: None,
            deletions: None,
        }),
        ThreadItem::ReasoningStep {
            id,
            title,
            summary,
            status,
            ..
        } => {
            if !mode.shows_reasoning_rows() {
                return None;
            }
            let preview = summary
                .lines()
                .next()
                .unwrap_or(title)
                .chars()
                .take(80)
                .collect();
            Some(TimelineEvent {
                id: id.clone(),
                item_ix,
                kind: TimelineEventKind::ReasoningNote,
                label: title.clone(),
                detail: Some(preview),
                phase: ActivityPhase::Explore,
                status: status.clone(),
                artifact_id: None,
                additions: None,
                deletions: None,
            })
        }
        ThreadItem::ToolCall {
            id,
            tool_name,
            command,
            status,
            ..
        } => {
            let phase = phase_for_tool_name(tool_name);
            let kind = tool_kind_from_name(tool_name);
            Some(TimelineEvent {
                id: id.clone(),
                item_ix,
                kind,
                label: tool_name.clone(),
                detail: command.clone(),
                phase,
                status: status.clone(),
                artifact_id: Some(format!("tool-{id}")),
                additions: None,
                deletions: None,
            })
        }
        ThreadItem::DiffSummary {
            id,
            files_changed,
            additions,
            deletions,
            ..
        } => Some(TimelineEvent {
            id: id.clone(),
            item_ix,
            kind: TimelineEventKind::FileEdit,
            label: format!("{files_changed} files changed"),
            detail: Some(format!("+{additions} −{deletions}")),
            phase: ActivityPhase::Edit,
            status: AgentStatus::Completed,
            artifact_id: Some(format!("diff-{id}")),
            additions: Some(*additions),
            deletions: Some(*deletions),
        }),
        ThreadItem::ApprovalRequest { id, title, .. } => Some(TimelineEvent {
            id: id.clone(),
            item_ix,
            kind: TimelineEventKind::ApprovalRequest,
            label: title.clone(),
            detail: None,
            phase: ActivityPhase::Review,
            status: AgentStatus::WaitingApproval,
            artifact_id: None,
            additions: None,
            deletions: None,
        }),
        ThreadItem::RunError { id, message, .. } => Some(TimelineEvent {
            id: id.clone(),
            item_ix,
            kind: TimelineEventKind::Error,
            label: message
                .lines()
                .next()
                .unwrap_or(message)
                .chars()
                .take(80)
                .collect(),
            detail: Some(message.clone()),
            phase: ActivityPhase::Run,
            status: AgentStatus::Failed,
            artifact_id: None,
            additions: None,
            deletions: None,
        }),
        ThreadItem::ChoiceRequest { id, prompt, .. } => Some(TimelineEvent {
            id: id.clone(),
            item_ix,
            kind: TimelineEventKind::ChoiceRequest,
            label: prompt.chars().take(80).collect(),
            detail: None,
            phase: ActivityPhase::Review,
            status: AgentStatus::WaitingApproval,
            artifact_id: None,
            additions: None,
            deletions: None,
        }),
        ThreadItem::ContextTrace { id, entries, .. } => Some(TimelineEvent {
            id: id.clone(),
            item_ix,
            kind: TimelineEventKind::ContextTrace,
            label: "Context".into(),
            detail: Some(format!("{} items", entries.len())),
            phase: ActivityPhase::Explore,
            status: AgentStatus::Completed,
            artifact_id: None,
            additions: None,
            deletions: None,
        }),
        ThreadItem::TodoList { .. } => None,
    }
}

fn tool_kind_from_name(name: &str) -> TimelineEventKind {
    match name {
        "read_file" | "Read" => TimelineEventKind::FileRead,
        "search" | "grep" | "glob_file_search" | "codebase_search" => TimelineEventKind::FileSearch,
        "propose_patch" | "apply_patch" | "edit_file" | "write" => TimelineEventKind::FileEdit,
        "bash_virtual" | "run_real_command" | "RunCommand" | "shell" => {
            TimelineEventKind::CommandRun
        }
        _ => TimelineEventKind::CommandRun,
    }
}

pub fn phase_label(phase: ActivityPhase) -> &'static str {
    match phase {
        ActivityPhase::Explore => "Planning",
        ActivityPhase::Edit => "Implementation",
        ActivityPhase::Run => "Validation",
        ActivityPhase::Review => "Review",
    }
}

pub fn should_emit_thread_item(
    item: &ThreadItem,
    mode: TranscriptMode,
    item_ix: usize,
    items: &[ThreadItem],
) -> bool {
    match mode {
        TranscriptMode::Summary => match item {
            ThreadItem::UserMessage { .. }
            | ThreadItem::RunError { .. }
            | ThreadItem::ApprovalRequest { .. }
            | ThreadItem::ChoiceRequest { .. } => true,
            ThreadItem::AssistantMessage {
                streaming: true, ..
            } => true,
            ThreadItem::AssistantMessage { .. } => is_final_assistant(item_ix, items),
            ThreadItem::ReasoningStep { status, .. } => {
                matches!(status, AgentStatus::Thinking)
            }
            ThreadItem::ToolCall { status, .. } => matches!(
                status,
                AgentStatus::RunningTool | AgentStatus::WaitingApproval
            ),
            ThreadItem::DiffSummary { .. }
            | ThreadItem::SubagentRun { .. }
            | ThreadItem::ContextTrace { .. }
            | ThreadItem::TodoList { .. }
            | ThreadItem::PlanStatus { .. } => false,
        },
        _ => match item {
            ThreadItem::ReasoningStep { status, .. } => {
                mode.shows_reasoning_rows() || matches!(status, AgentStatus::Thinking)
            }
            ThreadItem::TodoList { .. } => false,
            _ => true,
        },
    }
}

/// Last non-streaming assistant in the thread (ignoring trailing todo lists).
pub fn is_final_assistant(item_ix: usize, items: &[ThreadItem]) -> bool {
    match items.get(item_ix) {
        Some(ThreadItem::AssistantMessage {
            streaming: false, ..
        }) => items.get(item_ix + 1..).is_none_or(|rest| {
            rest.iter()
                .all(|i| matches!(i, ThreadItem::TodoList { .. }))
        }),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_msg(id: &str) -> ThreadItem {
        ThreadItem::UserMessage {
            id: id.into(),
            text: "hi".into(),
            attachments: Vec::new(),
            expanded: false,
        }
    }

    fn assistant(id: &str, streaming: bool) -> ThreadItem {
        ThreadItem::AssistantMessage {
            id: id.into(),
            markdown: String::new(),
            streaming,
            depth: 0,
            parent_call_id: None,
        }
    }

    fn thinking() -> ThreadItem {
        ThreadItem::ReasoningStep {
            id: "r1".into(),
            title: "Thinking".into(),
            summary: String::new(),
            expanded: false,
            status: AgentStatus::Thinking,
            depth: 0,
            parent_call_id: None,
        }
    }

    #[test]
    fn summary_shows_streaming_assistant() {
        let items = vec![user_msg("u1"), assistant("a1", true)];
        assert!(should_emit_thread_item(
            &items[1],
            TranscriptMode::Summary,
            1,
            &items,
        ));
    }

    #[test]
    fn summary_hides_completed_intermediate_assistant() {
        let items = vec![
            user_msg("u1"),
            assistant("a1", false),
            assistant("a2", true),
        ];
        assert!(!should_emit_thread_item(
            &items[1],
            TranscriptMode::Summary,
            1,
            &items,
        ));
    }

    #[test]
    fn normal_shows_thinking_row() {
        let items = vec![user_msg("u1"), thinking()];
        assert!(should_emit_thread_item(
            &items[1],
            TranscriptMode::Normal,
            1,
            &items,
        ));
    }
}
