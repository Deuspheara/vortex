//! Agent run orchestration — send_message, start_agent_run, cancel_active_run.

use gpui::{Context, Window};

use super::super::AgentWindow;
use crate::agent::ReducerState;
use crate::features::composer::state::PendingImageSource;
use crate::features::shell::state::{
    AgentStatus, ConversationId, MessageAttachment, MessageAttachmentPreview, ThreadItem,
};

impl AgentWindow {
    pub fn try_start_simulation(
        &mut self,
        conversation_id: ConversationId,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.simulations_running.contains(&conversation_id) {
            return false;
        }
        self.simulations_running.insert(conversation_id);
        self.status.agent_status = Some(AgentStatus::Thinking);
        cx.notify();
        true
    }

    pub fn finish_simulation(&mut self, conversation_id: ConversationId, cx: &mut Context<Self>) {
        self.simulations_running.remove(&conversation_id);
        cx.notify();
    }

    pub fn send_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input_state.read(cx).value().trim().to_string();
        let image_attachments = self.pending_image_attachments.clone();
        if text.is_empty() && image_attachments.is_empty() {
            self.input_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });
            return;
        }

        if !image_attachments.is_empty() && !self.selected_model_supports_image_input() {
            self.composer_error = Some(format!(
                "{} does not support image input. Choose a vision-capable model.",
                self.selected_model
            ));
            cx.notify();
            return;
        }

        let conv_id = match self.ensure_send_target(cx) {
            Some(cid) => cid,
            None => {
                tracing::warn!("send blocked: no project/session available");
                return;
            }
        };

        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == conv_id) {
            let id = format!("user-msg-{}", conv.thread_items.len());
            let message_text = if text.is_empty() {
                "Image attachment".to_string()
            } else {
                text.clone()
            };
            conv.thread_items.push(ThreadItem::UserMessage {
                id,
                text: message_text,
                attachments: message_attachments_from_pending(&image_attachments),
                expanded: false,
            });
        }
        if self.safety_mode != agent_protocol::AgentMode::PlanOnly {
            self.mark_plan_stale(&conv_id);
        }

        if !text.is_empty() {
            self.maybe_rename_conversation_from_prompt(&conv_id, &text, cx);
        }

        self.input_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.pending_image_attachments.clear();
        self.composer_error = None;

        self.sync_plan_status_for_conversation(&conv_id, cx);
        self.sync_thread_view(conv_id.clone(), cx);

        let run_prompt = if text.is_empty() {
            "Please inspect the attached image(s).".to_string()
        } else {
            text.clone()
        };
        let attachments = image_attachments
            .iter()
            .map(|attachment| attachment.to_context_attachment())
            .collect();
        if let Err(err) = self.start_agent_run(&run_prompt, attachments, cx) {
            tracing::error!("failed to start agent run: {err}");
            if let Some(cid) = &self.selected_conversation_id {
                self.running_conversations.remove(cid);
            }
            self.status.agent_status = Some(AgentStatus::Idle);
            let error_id = format!("error-{}", uuid::Uuid::new_v4());
            self.push_thread_item(
                conv_id.clone(),
                ThreadItem::RunError {
                    id: error_id,
                    message: format!("Could not start run: {err}"),
                    session_ref: None,
                    retryable: true,
                },
                cx,
            );
            self.sync_thread_view(conv_id, cx);
        }

        cx.notify();
    }

    pub fn start_agent_run(
        &mut self,
        prompt: &str,
        attachments: Vec<agent_protocol::ContextAttachment>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let (project_id, session_id) =
            match (&self.selected_project_id, &self.selected_conversation_id) {
                (Some(pid), Some(cid)) => (
                    crate::agent::proto_project_id(pid),
                    crate::agent::proto_session_id(cid),
                ),
                _ => return Err("no project or session selected".into()),
            };

        self.reconcile_stale_run_state();
        if self.selected_conversation_has_active_work() {
            return Err("a run is already in progress for this conversation".into());
        }

        if let Some(cid) = self.selected_conversation_id.clone() {
            self.running_conversations.insert(cid.clone());
            self.finalize_inflight_thread_items(&cid, AgentStatus::Failed, AgentStatus::Failed);
            self.sync_thread_view(cid.clone(), cx);
        }

        self.reducer_state = ReducerState::default();
        self.status.agent_status = Some(AgentStatus::Thinking);

        if let Some(cid) = self.selected_conversation_id.clone() {
            self.push_thinking_indicator(cid, cx);
        }

        let model_name = if self.agent_bridge.uses_mock {
            self.selected_model.clone()
        } else {
            crate::shared::state::openrouter_model_slug(
                &self.selected_provider,
                &self.selected_model,
            )
        };
        let model = agent_protocol::ModelId::new(model_name);
        let subagent_model = self.selected_subagent_model.as_ref().map(|model| {
            let model_name = if self.agent_bridge.uses_mock {
                model.clone()
            } else {
                crate::shared::state::openrouter_model_slug(&self.selected_provider, model)
            };
            agent_protocol::ModelId::new(model_name)
        });
        let prompt = if let Some(conv) = self
            .conversations
            .iter()
            .find(|c| Some(&c.id) == self.selected_conversation_id.as_ref())
        {
            let context = crate::agent::text::conversation_context_from_thread(&conv.thread_items);
            crate::agent::text::prompt_with_conversation_context(&context, prompt)
        } else {
            prompt.to_string()
        };
        self.agent_bridge
            .send(agent_protocol::AgentCommand::StartRun {
                project_id,
                session_id,
                prompt,
                model,
                subagent_model,
                mode: self.safety_mode.clone(),
                attachments,
            })?;
        cx.notify();
        Ok(())
    }

    pub fn cancel_active_run(&mut self, cx: &mut Context<Self>) {
        if let Some(run_id) = &self.active_run_id {
            self.agent_bridge
                .send(agent_protocol::AgentCommand::CancelRun {
                    run_id: run_id.clone(),
                })
                .ok();
        }
        self.pending_approval_id = None;
        self.pending_thread_approval = None;
        self.diff_panel.pending_approval = None;
        self.diff_panel.pending_patch_id = None;
        if let Some(cid) = self.selected_conversation_id.clone() {
            self.running_conversations.remove(&cid);
            self.cancel_in_progress_todos(&cid);
            self.resolve_pending_choices(&cid);
            self.sync_thread_view(cid, cx);
        }
        self.status.agent_status = Some(AgentStatus::Idle);
        self.sync_thread_approval_state(cx);
        cx.notify();
    }

    pub fn can_retry_last_user_turn(&self) -> bool {
        self.selected_conversation_id
            .as_ref()
            .and_then(|cid| self.conversations.iter().find(|conv| &conv.id == cid))
            .and_then(|conv| {
                conv.thread_items.iter().rev().find_map(|item| match item {
                    ThreadItem::UserMessage {
                        text, attachments, ..
                    } => Some((text, attachments)),
                    _ => None,
                })
            })
            .is_some_and(|(text, attachments)| !text.trim().is_empty() || !attachments.is_empty())
    }

    pub fn retry_last_user_turn(&mut self, cx: &mut Context<Self>) {
        let Some(conv_id) = self.selected_conversation_id.clone() else {
            return;
        };
        let Some((prompt, attachments)) = self
            .conversations
            .iter()
            .find(|conv| conv.id == conv_id)
            .and_then(|conv| {
                conv.thread_items.iter().rev().find_map(|item| match item {
                    ThreadItem::UserMessage {
                        text, attachments, ..
                    } => Some((text.clone(), attachments.clone())),
                    _ => None,
                })
            })
        else {
            return;
        };

        if self.selected_conversation_has_active_work() {
            return;
        }

        self.select_conversation(conv_id.clone(), cx);
        self.composer_error = None;

        let prompt = if prompt.trim().is_empty() {
            "Please inspect the attached image(s).".to_string()
        } else {
            prompt
        };
        let attachments = attachments
            .iter()
            .filter_map(message_attachment_to_context_attachment)
            .collect::<Vec<_>>();

        if let Err(err) = self.start_agent_run(&prompt, attachments, cx) {
            tracing::error!("failed to retry run: {err}");
            self.running_conversations.remove(&conv_id);
            self.status.agent_status = Some(AgentStatus::Idle);
            self.push_thread_item(
                conv_id.clone(),
                ThreadItem::RunError {
                    id: format!("retry-error-{}", uuid::Uuid::new_v4()),
                    message: format!("Could not retry run: {err}"),
                    session_ref: None,
                    retryable: false,
                },
                cx,
            );
            self.sync_thread_view(conv_id, cx);
        }
        cx.notify();
    }
}

fn message_attachments_from_pending(
    attachments: &[crate::features::composer::state::PendingImageAttachment],
) -> Vec<MessageAttachment> {
    attachments
        .iter()
        .map(|attachment| MessageAttachment {
            label: attachment.display_name.clone(),
            mime_type: attachment.mime_type.clone(),
            size_bytes: attachment.size_bytes,
            preview: match &attachment.source {
                PendingImageSource::File(path) => MessageAttachmentPreview::File(path.clone()),
                PendingImageSource::Clipboard(bytes) => MessageAttachmentPreview::Bytes {
                    mime_type: attachment.mime_type.clone(),
                    bytes: bytes.clone(),
                },
            },
        })
        .collect()
}

fn message_attachment_to_context_attachment(
    attachment: &MessageAttachment,
) -> Option<agent_protocol::ContextAttachment> {
    match &attachment.preview {
        MessageAttachmentPreview::File(path) => {
            Some(agent_protocol::ContextAttachment::image_file(
                path.clone(),
                attachment.mime_type.clone(),
                attachment.size_bytes,
            ))
        }
        MessageAttachmentPreview::Bytes { mime_type, bytes } => {
            Some(agent_protocol::ContextAttachment::image_bytes(
                bytes.clone(),
                mime_type.clone(),
                attachment.label.clone(),
            ))
        }
    }
}
