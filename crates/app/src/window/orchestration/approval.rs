//! Approval orchestration — approve/reject/apply_patch/clear_pending_approval.

use gpui::Context;

use super::super::AgentWindow;
use crate::features::shell::state::{AgentStatus, ThreadItem};

impl AgentWindow {
    pub fn approve_pending(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.pending_approval_id.take() else {
            return;
        };
        self.begin_approval_resolution(cx);
        if let Err(err) = self
            .agent_bridge
            .send(agent_protocol::AgentCommand::ApproveTool {
                approval_id: agent_protocol::ApprovalId::new(id),
            })
        {
            tracing::error!("failed to send ApproveTool: {err}");
            if let Some(conv_id) = self.selected_conversation_id.clone() {
                self.push_thread_item(
                    conv_id,
                    ThreadItem::RunError {
                        id: format!("approve-err-{}", uuid::Uuid::new_v4()),
                        message: format!("Could not approve: {err}"),
                        session_ref: self.active_run_id.as_ref().map(|r| r.0.clone()),
                        retryable: false,
                    },
                    cx,
                );
            }
            self.reset_agent_status_to_idle();
            cx.notify();
        }
    }

    pub fn approve_pending_always(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.pending_approval_id.take() else {
            return;
        };
        self.begin_approval_resolution(cx);
        if let Err(err) = self
            .agent_bridge
            .send(agent_protocol::AgentCommand::ApproveToolAlways {
                approval_id: agent_protocol::ApprovalId::new(id),
            })
        {
            tracing::error!("failed to send ApproveToolAlways: {err}");
            if let Some(conv_id) = self.selected_conversation_id.clone() {
                self.push_thread_item(
                    conv_id,
                    ThreadItem::RunError {
                        id: format!("approve-always-err-{}", uuid::Uuid::new_v4()),
                        message: format!("Could not approve and remember: {err}"),
                        session_ref: self.active_run_id.as_ref().map(|r| r.0.clone()),
                        retryable: false,
                    },
                    cx,
                );
            }
            self.reset_agent_status_to_idle();
            cx.notify();
        }
    }

    pub fn apply_pending_patch(&mut self, cx: &mut Context<Self>) {
        let Some(patch_id) = self.diff_panel.pending_patch_id.take() else {
            return;
        };
        self.diff_panel.pending_approval = None;
        self.sync_thread_approval_state(cx);
        if let Some(conv_id) = self.selected_conversation_id.clone() {
            self.mark_plan_execution_implementing(&conv_id);
            self.sync_plan_status_for_conversation(&conv_id, cx);
        }
        if let Err(err) = self
            .agent_bridge
            .send(agent_protocol::AgentCommand::ApprovePatch {
                patch_id: agent_protocol::PatchId::new(patch_id),
                scope: agent_protocol::PatchApprovalScope::All,
            })
        {
            tracing::error!("failed to send ApprovePatch: {err}");
            if let Some(conv_id) = self.selected_conversation_id.clone() {
                self.push_thread_item(
                    conv_id,
                    ThreadItem::RunError {
                        id: format!("patch-err-{}", uuid::Uuid::new_v4()),
                        message: format!("Could not apply patch: {err}"),
                        session_ref: self.active_run_id.as_ref().map(|r| r.0.clone()),
                        retryable: false,
                    },
                    cx,
                );
            }
        }
        self.sync_agent_status(AgentStatus::RunningTool);
        cx.notify();
    }

    pub fn reject_pending_patch(&mut self, reason: Option<String>, cx: &mut Context<Self>) {
        let Some(patch_id) = self.diff_panel.pending_patch_id.take() else {
            return;
        };
        self.diff_panel.pending_approval = None;
        self.pending_thread_approval = None;
        self.sync_thread_approval_state(cx);
        if let Some(conv_id) = self.selected_conversation_id.clone() {
            self.mark_plan_execution_implementing(&conv_id);
            self.sync_plan_status_for_conversation(&conv_id, cx);
        }
        let _ = self
            .agent_bridge
            .send(agent_protocol::AgentCommand::RejectPatch {
                patch_id: agent_protocol::PatchId::new(patch_id),
                reason,
            });
        cx.notify();
    }

    pub fn reject_pending(&mut self, reason: Option<String>, cx: &mut Context<Self>) {
        let Some(id) = self.pending_approval_id.take() else {
            return;
        };
        self.begin_approval_resolution(cx);
        let _ = self
            .agent_bridge
            .send(agent_protocol::AgentCommand::RejectTool {
                approval_id: agent_protocol::ApprovalId::new(id),
                reason,
            });
        cx.notify();
    }

    pub(crate) fn clear_pending_approval_ui(
        &mut self,
        conv_id: &crate::features::shell::state::ConversationId,
        cx: &mut Context<Self>,
    ) {
        self.pending_approval_id = None;
        self.diff_panel.pending_approval = None;
        self.pending_thread_approval = None;
        self.resolve_approval_item(conv_id);
        self.sync_thread_approval_state(cx);
    }
}
