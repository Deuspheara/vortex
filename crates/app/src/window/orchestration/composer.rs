//! Composer orchestration — composer_scope, branch_items, on_composer_*, pending actions.

use std::path::{Path, PathBuf};

use gpui::{ClipboardEntry, Context, Entity, Window};

use super::super::AgentWindow;
use crate::agent::paths::git_branch_info;
use crate::features::agent_activity::components::approval::ApprovalCardProps;
use crate::features::agent_activity::components::pending_action_bar::{
    CurrentActionProps, PatchActionProps, PendingActionBarProps, PlanReviewActionProps,
};
use crate::features::agent_activity::components::tool_call::tool_call_receipt_text;
use crate::features::composer::state::{PendingImageAttachment, PendingImageSource};
use crate::features::shell::state::{
    AgentStatus, ChipKind, InspectorMode, PlanExecutionState, PlanProgressCounts, Project,
    ProjectId, ReviewPanelTab, ThreadItem,
};
use crate::shared::state::TranscriptMode;

const SUPPORTED_IMAGE_MIME_TYPES: &[&str] = &["image/png", "image/jpeg", "image/webp", "image/gif"];

impl AgentWindow {
    pub(crate) fn pending_action_bar_row_count(&self) -> u8 {
        let patch = self.diff_panel.pending_patch_id.is_some()
            && !self.diff_panel.applied
            && !self.safety_mode.auto_applies_patches();
        let current_action = self.composer_current_action_content().is_some();
        let approval = false;
        let plan_review = self.composer_has_plan_review_action();
        (patch as u8) + (current_action as u8) + (approval as u8) + (plan_review as u8)
    }

    pub(crate) fn pending_composer_actions(
        &self,
        entity: Entity<Self>,
    ) -> (PendingActionBarProps, Option<ApprovalCardProps>) {
        let sticky_approval = self.pending_thread_approval.clone().map(|pending| {
            let reject_entity = entity.clone();
            let approve_entity = entity.clone();
            let approve_always_entity = entity.clone();
            ApprovalCardProps {
                title: pending.title,
                risk: pending.risk,
                can_act: self.has_pending_approval(),
                allow_always_label: pending.allow_always_label,
                on_deny: Box::new(move |app| {
                    reject_entity.update(app, |view, cx| {
                        view.reject_pending(None, cx);
                    });
                }),
                on_approve: Box::new(move |app| {
                    approve_entity.update(app, |view, cx| {
                        view.approve_pending(cx);
                    });
                }),
                on_approve_always: Some(Box::new(move |app| {
                    approve_always_entity.update(app, |view, cx| {
                        view.approve_pending_always(cx);
                    });
                })),
            }
        });

        let current_action = self.current_composer_action(entity.clone());
        let approval = None;

        let patch = if self.diff_panel.pending_patch_id.is_some()
            && !self.diff_panel.applied
            && !self.safety_mode.auto_applies_patches()
        {
            let files_changed = self.diff_panel.files.len();
            let (additions, deletions) = self
                .diff_panel
                .files
                .iter()
                .fold((0usize, 0usize), |(a, d), file| {
                    (a + file.added, d + file.removed)
                });
            let open_entity = entity.clone();
            let cancel_entity = entity.clone();
            Some(PatchActionProps {
                summary: format!("{files_changed} files changed · +{additions} −{deletions}"),
                on_open: Box::new(move |app| {
                    open_entity.update(app, |view, cx| {
                        view.open_diff_panel(cx);
                    });
                }),
                on_cancel: Box::new(move |app| {
                    cancel_entity.update(app, |view, cx| {
                        view.reject_pending_patch(Some("Cancelled by user".into()), cx);
                    });
                }),
            })
        } else {
            None
        };

        let plan_review =
            if sticky_approval.is_none() && current_action.is_none() && patch.is_none() {
                self.composer_plan_review_action(entity.clone())
            } else {
                None
            };

        (
            PendingActionBarProps {
                current_action,
                approval,
                patch,
                plan_review,
            },
            sticky_approval,
        )
    }

    fn composer_has_plan_review_action(&self) -> bool {
        if self.pending_thread_approval.is_some() || self.selected_conversation_has_active_work() {
            return false;
        }
        if self.diff_panel.pending_patch_id.is_some() && !self.diff_panel.applied {
            return false;
        }
        let Some(conv_id) = self.selected_conversation_id.as_ref() else {
            return false;
        };
        let Some(plan) = self
            .conversations
            .iter()
            .find(|conv| &conv.id == conv_id)
            .and_then(|conv| conv.plan_artifact.as_ref())
        else {
            return false;
        };
        !plan.execution_state.is_active()
    }

    fn composer_plan_review_action(&self, entity: Entity<Self>) -> Option<PlanReviewActionProps> {
        if !self.composer_has_plan_review_action() {
            return None;
        }

        let conv_id = self.selected_conversation_id.as_ref()?;
        let conv = self.conversations.iter().find(|conv| &conv.id == conv_id)?;
        let plan = conv.plan_artifact.as_ref()?;
        let summary = conv
            .thread_items
            .iter()
            .rev()
            .find_map(|item| match item {
                ThreadItem::PlanStatus { summary, .. } => Some(summary.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "Reviewed plan ready".to_string());

        let review_entity = entity.clone();
        let implement_entity = entity;

        Some(PlanReviewActionProps {
            title: "Review plan".to_string(),
            detail: composer_plan_review_detail(plan.execution_state, &summary),
            primary_label: composer_plan_primary_label(plan.execution_state).to_string(),
            on_review: Box::new(move |app| {
                review_entity.update(app, |view, cx| {
                    view.set_review_tab(ReviewPanelTab::Plan, cx);
                    view.set_inspector_mode(InspectorMode::Review, cx);
                });
            }),
            on_implement: Box::new(move |app| {
                implement_entity.update(app, |view, cx| {
                    view.set_review_tab(ReviewPanelTab::Plan, cx);
                    view.set_inspector_mode(InspectorMode::Review, cx);
                    view.show_plan_implementation_choice(cx);
                });
            }),
        })
    }

    fn current_composer_action(&self, entity: Entity<Self>) -> Option<CurrentActionProps> {
        let (label, detail) = self.composer_current_action_content()?;
        let view_activity = entity.clone();
        let stop_run = entity;
        Some(CurrentActionProps {
            label,
            detail,
            on_view_activity: Box::new(move |app| {
                view_activity.update(app, |view, cx| {
                    view.set_transcript_mode(TranscriptMode::Activity, cx);
                });
            }),
            on_stop: Box::new(move |app| {
                stop_run.update(app, |view, cx| {
                    view.cancel_active_run(cx);
                });
            }),
        })
    }

    fn composer_current_action_content(&self) -> Option<(String, Option<String>)> {
        if self.pending_thread_approval.is_some() {
            return None;
        }
        if self.diff_panel.pending_patch_id.is_some() && !self.diff_panel.applied {
            return None;
        }

        let conv_id = self.selected_conversation_id.as_ref()?;
        if !self.running_conversations.contains(conv_id) && self.active_run_id.is_none() {
            return None;
        }

        let conv = self.conversations.iter().find(|c| &c.id == conv_id)?;
        conv.thread_items.iter().rev().find_map(|item| match item {
            ThreadItem::ToolCall {
                tool_name,
                command,
                status: AgentStatus::RunningTool,
                ..
            } => {
                let display_label = self.tool_row_label(tool_name, command.as_deref(), true);
                let (label, detail) = tool_call_receipt_text(
                    tool_name,
                    &display_label,
                    command.as_deref(),
                    &AgentStatus::RunningTool,
                );
                Some((label, detail))
            }
            ThreadItem::ReasoningStep {
                title,
                summary,
                status: AgentStatus::Thinking,
                ..
            } => Some(reasoning_current_action(title, summary)),
            ThreadItem::SubagentRun {
                task,
                status: AgentStatus::RunningTool,
                ..
            } => Some(("Subagent working".into(), Some(task.clone()))),
            ThreadItem::AssistantMessage {
                streaming: true, ..
            } => Some(("Writing reply".into(), None)),
            ThreadItem::PlanStatus {
                state,
                summary,
                counts,
                ..
            } if state.is_active() => Some(plan_current_action(*state, summary, *counts)),
            _ => None,
        })
    }

    pub fn composer_scope(&self) -> (String, String, Option<ProjectId>) {
        let _profile = crate::shared::render_profile::span("composer_scope");
        if let Some(project) = self.selected_composer_project() {
            return (
                project.name.clone(),
                project.git_branch.clone(),
                Some(project.id.clone()),
            );
        }

        ("Select project".into(), "main".into(), None)
    }

    fn selected_composer_project(&self) -> Option<&Project> {
        if let Some(cid) = &self.selected_conversation_id {
            if let Some(conv) = self.conversations.iter().find(|c| c.id == *cid) {
                if let Some(project) = self.projects.iter().find(|p| p.id == conv.project_id) {
                    return Some(project);
                }
            }
        }

        if let Some(pid) = &self.selected_project_id {
            if let Some(project) = self.projects.iter().find(|p| p.id == *pid) {
                return Some(project);
            }
        }

        None
    }

    pub fn branch_items_for_selected_project(&self) -> Vec<String> {
        let _profile = crate::shared::render_profile::span("branch_items_for_selected_project");
        let Some(project) = self.selected_composer_project() else {
            return vec!["main".into()];
        };

        if let Some((cached_root, items)) = &self.branch_items_cache {
            if *cached_root == project.root_path {
                return items.clone();
            }
        }

        vec![project.git_branch.clone()]
    }

    pub fn refresh_branch_items_for_project(&mut self, project_id: &ProjectId) {
        let Some(root) = self
            .projects
            .iter()
            .find(|project| &project.id == project_id)
            .map(|project| project.root_path.clone())
        else {
            return;
        };

        let branch_info = git_branch_info(std::path::Path::new(&root));
        self.branch_items_cache = Some((root, branch_info.branches));
        if let Some(project) = self
            .projects
            .iter_mut()
            .find(|project| &project.id == project_id)
        {
            project.git_branch = branch_info.current.clone();
        }
        self.sync_branch_chips_for_project(project_id, &branch_info.current);
    }

    pub fn invalidate_branch_items_cache(&mut self) {
        self.branch_items_cache = None;
    }

    pub fn sync_branch_chips_for_project(&mut self, project_id: &ProjectId, branch: &str) {
        for conv in &mut self.conversations {
            if &conv.project_id != project_id {
                continue;
            }
            for chip in &mut conv.context_chips {
                if chip.kind == ChipKind::Branch {
                    chip.label = branch.to_string();
                }
            }
        }
    }

    pub fn open_image_attachment_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entity = cx.weak_entity();
        let future = rfd::AsyncFileDialog::new()
            .set_parent(window)
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"])
            .pick_files();

        cx.spawn(async move |_weak, cx| {
            let Some(handles) = future.await else { return };
            let paths: Vec<PathBuf> = handles
                .into_iter()
                .map(|handle| handle.path().to_path_buf())
                .collect();
            let _ = cx.update(|app| {
                app.defer(move |app| {
                    if let Some(view) = entity.upgrade() {
                        view.update(app, |view, cx| {
                            view.add_image_attachment_paths(paths, cx);
                        });
                    }
                });
            });
        })
        .detach();
    }

    pub fn add_image_attachment_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let mut added = 0usize;
        for path in paths {
            let Some(mime_type) = image_mime_for_path(&path) else {
                self.composer_error = Some(format!(
                    "Unsupported image type: {}",
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("attachment")
                ));
                continue;
            };
            let size_bytes = match std::fs::metadata(&path) {
                Ok(metadata) => metadata.len(),
                Err(err) => {
                    self.composer_error = Some(format!("Could not read image metadata: {err}"));
                    continue;
                }
            };
            let display_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("image")
                .to_string();
            self.pending_image_attachments.push(PendingImageAttachment {
                id: format!("img-{}", uuid::Uuid::new_v4()),
                source: PendingImageSource::File(path),
                mime_type: mime_type.to_string(),
                display_name,
                size_bytes,
            });
            added += 1;
        }
        if added > 0 {
            self.composer_error = None;
        }
        cx.notify();
    }

    pub fn add_clipboard_image_attachment(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(clipboard) = cx.read_from_clipboard() else {
            return false;
        };
        let Some(image) = clipboard.entries().iter().find_map(|entry| match entry {
            ClipboardEntry::Image(image) => Some(image.clone()),
            ClipboardEntry::String(_) => None,
        }) else {
            return false;
        };
        let mime_type = image.format().mime_type();
        if !SUPPORTED_IMAGE_MIME_TYPES.contains(&mime_type) {
            self.composer_error = Some(format!("Unsupported pasted image type: {mime_type}"));
            cx.notify();
            return true;
        }
        self.pending_image_attachments.push(PendingImageAttachment {
            id: format!("img-{}", uuid::Uuid::new_v4()),
            source: PendingImageSource::Clipboard(image.bytes().to_vec()),
            mime_type: mime_type.to_string(),
            display_name: format!("Pasted image {}", self.pending_image_attachments.len() + 1),
            size_bytes: image.bytes().len() as u64,
        });
        self.composer_error = None;
        cx.notify();
        true
    }

    pub fn remove_image_attachment(&mut self, id: &str, cx: &mut Context<Self>) {
        self.pending_image_attachments
            .retain(|attachment| attachment.id != id);
        if self.pending_image_attachments.is_empty() {
            self.composer_error = None;
        }
        cx.notify();
    }

    pub fn on_composer_project_selected(&mut self, project_name: String, cx: &mut Context<Self>) {
        let Some(project) = self.projects.iter().find(|p| p.name == project_name) else {
            return;
        };
        let project_id = project.id.clone();

        self.selected_project_id = Some(project_id.clone());

        if let Some(conv_id) = project.conversations.first().cloned() {
            self.select_conversation(conv_id, cx);
        } else {
            self.create_conversation_in_project(project_id, cx);
        }

        cx.notify();
    }

    pub fn on_composer_branch_selected(&mut self, branch: String, cx: &mut Context<Self>) {
        let Some(project_id) = self
            .selected_composer_project()
            .map(|project| project.id.clone())
        else {
            return;
        };

        let root = self
            .projects
            .iter_mut()
            .find(|p| p.id == project_id)
            .map(|project| {
                project.git_branch = branch.clone();
                project.root_path.clone()
            });

        if let Some(root) = root {
            match &mut self.branch_items_cache {
                Some((cached_root, items)) if *cached_root == root => {
                    if !items.iter().any(|item| item == &branch) {
                        items.insert(0, branch.clone());
                    }
                }
                _ => {
                    self.branch_items_cache = Some((root, vec![branch.clone()]));
                }
            }
        } else if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
            project.git_branch = branch.clone();
        }

        self.sync_branch_chips_for_project(&project_id, &branch);

        cx.notify();
    }
}

fn reasoning_current_action(title: &str, summary: &str) -> (String, Option<String>) {
    let detail = summary
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(trim_action_detail);
    match detail {
        Some(detail) => ("Current step".into(), Some(detail)),
        None if matches!(title, "Thinking" | "thinking" | "Current step") => {
            ("Planning".into(), None)
        }
        None => (title.to_string(), None),
    }
}

fn composer_plan_primary_label(state: PlanExecutionState) -> &'static str {
    match state {
        PlanExecutionState::Completed => "Implement again",
        PlanExecutionState::Failed | PlanExecutionState::Cancelled | PlanExecutionState::Stale => {
            "Retry plan"
        }
        _ => "Implement plan",
    }
}

fn composer_plan_review_detail(state: PlanExecutionState, summary: &str) -> String {
    let prefix = match state {
        PlanExecutionState::Completed => "Last run completed",
        PlanExecutionState::Failed => "Last run failed",
        PlanExecutionState::Cancelled => "Last run cancelled",
        PlanExecutionState::Stale => "Plan updated",
        _ => "Reviewed plan ready",
    };
    let summary = trim_action_detail(summary.trim());
    if summary.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix} · {summary}")
    }
}

fn plan_current_action(
    state: PlanExecutionState,
    summary: &str,
    counts: PlanProgressCounts,
) -> (String, Option<String>) {
    let label = match state {
        PlanExecutionState::Starting => "Plan starting".to_string(),
        PlanExecutionState::Implementing => {
            let total = counts.total();
            if total > 0 {
                format!("Plan · {}/{} complete", counts.completed, total)
            } else {
                "Plan in progress".to_string()
            }
        }
        PlanExecutionState::WaitingApproval => "Plan blocked".to_string(),
        PlanExecutionState::Completed => "Plan completed".to_string(),
        PlanExecutionState::Failed => "Plan failed".to_string(),
        PlanExecutionState::Cancelled => "Plan cancelled".to_string(),
        PlanExecutionState::Stale => "Plan needs review".to_string(),
        PlanExecutionState::NotStarted => "Plan ready".to_string(),
    };
    let detail = (!summary.trim().is_empty()).then(|| trim_action_detail(summary.trim()));
    (label, detail)
}

fn trim_action_detail(value: &str) -> String {
    let mut chars = value.chars();
    let trimmed = chars.by_ref().take(96).collect::<String>();
    if chars.next().is_some() {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}

fn image_mime_for_path(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}
