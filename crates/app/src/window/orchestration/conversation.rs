//! Conversation orchestration — select/move/new/delete/reposition conversation.

use gpui::{Context, Window};

use super::super::AgentWindow;
use crate::agent::{proto_session_id, ui_conversation_id};
use crate::features::shell::components::tree_row::project_expand_key;
use crate::features::shell::state::{Conversation, ConversationId, ProjectId};

impl AgentWindow {
    pub fn new_conversation_from_nav(&mut self, cx: &mut Context<Self>) {
        if let Some(project_id) = self
            .selected_project_id
            .clone()
            .or_else(|| self.projects.first().map(|project| project.id.clone()))
        {
            self.create_conversation_in_project(project_id, cx);
            self.open_chat(cx);
        }
    }

    pub fn select_conversation(&mut self, id: ConversationId, cx: &mut Context<Self>) {
        self.selected_conversation_id = Some(id.clone());
        self.screen = crate::window::AppScreen::Chat;
        self.plan_implementation_choice_open = false;
        self.todo_strip_expanded = false;
        if let Some(conv) = self.conversations.iter().find(|c| c.id == id) {
            self.selected_project_id = Some(conv.project_id.clone());
        }
        if let Some(project_id) = self.selected_project_id.clone() {
            self.refresh_branch_items_for_project(&project_id);
        }
        if self.terminal_panel_open {
            self.ensure_terminal_tabs_for_project(cx);
        }
        self.replay_session_events(&id.0, cx);
        let items = self.thread_items_for(&id);
        if let Some(thread) = &self.thread_view {
            let _collapsed = self.collapsed_sessions.clone();
            thread.update(cx, |view, cx| view.set_conversation(id, items, cx));
        }
        self.refresh_indexing_state(cx);
        self.sync_sidebar_view(cx);
        cx.notify();
    }

    /// Move a session to the end of a project's conversation list.
    pub fn move_conversation_to_project(
        &mut self,
        conversation_id: ConversationId,
        target_project_id: ProjectId,
        cx: &mut Context<Self>,
    ) {
        for project in &mut self.projects {
            project.conversations.retain(|id| id != &conversation_id);
        }

        if let Some(project) = self.projects.iter_mut().find(|p| p.id == target_project_id) {
            if !project.conversations.contains(&conversation_id) {
                project.conversations.push(conversation_id.clone());
            }
        }

        if let Some(conv) = self
            .conversations
            .iter_mut()
            .find(|c| c.id == conversation_id)
        {
            if conv.project_id != target_project_id {
                conv.project_id = target_project_id.clone();
                self.sync_context_chips_for_conversation(&conversation_id);
            }
        }

        self.expanded_items
            .insert(project_expand_key(&target_project_id));
        self.sync_sidebar_view(cx);
        cx.notify();
    }

    /// Insert a session before another session in the target project's list.
    pub fn reposition_conversation(
        &mut self,
        conversation_id: ConversationId,
        before_id: ConversationId,
        cx: &mut Context<Self>,
    ) {
        if conversation_id == before_id {
            return;
        }

        let target_project_id = self
            .projects
            .iter()
            .find(|p| p.conversations.contains(&before_id))
            .map(|p| p.id.clone());

        let Some(target_project_id) = target_project_id else {
            return;
        };

        for project in &mut self.projects {
            project.conversations.retain(|id| id != &conversation_id);
        }

        if let Some(project) = self.projects.iter_mut().find(|p| p.id == target_project_id) {
            if let Some(idx) = project.conversations.iter().position(|id| id == &before_id) {
                project.conversations.insert(idx, conversation_id.clone());
            } else {
                project.conversations.push(conversation_id.clone());
            }
        }

        if let Some(conv) = self
            .conversations
            .iter_mut()
            .find(|c| c.id == conversation_id)
        {
            if conv.project_id != target_project_id {
                conv.project_id = target_project_id.clone();
                self.sync_context_chips_for_conversation(&conversation_id);
            }
        }

        self.sync_sidebar_view(cx);
        cx.notify();
    }

    pub fn new_conversation(&mut self, cx: &mut Context<Self>) {
        let pid = match &self.selected_project_id {
            Some(pid) => pid.clone(),
            None => return,
        };
        self.create_conversation_in_project(pid, cx);
    }

    pub fn new_conversation_in_project(&mut self, project_id: ProjectId, cx: &mut Context<Self>) {
        self.create_conversation_in_project(project_id, cx);
    }

    pub(crate) fn create_conversation_in_project(
        &mut self,
        pid: ProjectId,
        cx: &mut Context<Self>,
    ) {
        let proto_project_id = crate::agent::proto_project_id(&pid);
        let title = format!("New Conversation {}", self.conversations.len() + 1);
        let stored = match self.agent_bridge.create_session(&proto_project_id, &title) {
            Ok(s) => s,
            Err(err) => {
                tracing::error!("failed to create session: {err}");
                return;
            }
        };

        let conv_id = ui_conversation_id(&stored.id);
        let mut conv = Conversation::new(conv_id.0.clone(), pid.clone(), &stored.title, "now");
        if let Some(project) = self.projects.iter().find(|p| p.id == pid) {
            conv.context_chips = Self::context_chips_for_project(project);
        }

        self.conversations.push(conv.clone());
        if let Some(project) = self.projects.iter_mut().find(|p| p.id == pid) {
            project.conversations.push(conv.id.clone());
        }
        self.selected_project_id = Some(pid);
        self.selected_conversation_id = Some(conv.id.clone());
        self.refresh_branch_items_for_project(&conv.project_id);
        let key = project_expand_key(&conv.project_id);
        self.expanded_items.insert(key);
        if let Some(thread) = &self.thread_view {
            thread.update(cx, |view, cx| {
                view.set_conversation(conv.id.clone(), vec![], cx)
            });
        }
        self.refresh_indexing_state(cx);
        self.sync_sidebar_view(cx);
        cx.notify();
    }

    pub(crate) fn clear_sidebar_drag_state(&mut self, cx: &mut Context<Self>) {
        if let Some(sidebar) = &self.sidebar_view {
            sidebar.update(cx, |view, cx| {
                view.clear_drop_target(cx);
                view.close_action_menu(cx);
            });
        }
    }

    pub fn confirm_delete_conversation(
        &mut self,
        conversation_id: ConversationId,
        title: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let conv_id = conversation_id.clone();
        cx.defer_in(window, move |view, window, cx| {
            let confirmed = rfd::MessageDialog::new()
                .set_title("Delete conversation")
                .set_description(format!(
                    "Delete conversation \"{title}\"? This cannot be undone."
                ))
                .set_level(rfd::MessageLevel::Warning)
                .set_buttons(rfd::MessageButtons::OkCancel)
                .set_parent(window)
                .show();
            if confirmed == rfd::MessageDialogResult::Ok {
                view.delete_conversation(conv_id, cx);
            }
        });
    }

    pub fn delete_conversation(&mut self, conversation_id: ConversationId, cx: &mut Context<Self>) {
        self.clear_sidebar_drag_state(cx);

        let project_id = self
            .conversations
            .iter()
            .find(|c| c.id == conversation_id)
            .map(|c| c.project_id.clone());
        let was_selected = self.selected_conversation_id.as_ref() == Some(&conversation_id);

        if was_selected && self.active_run_id.is_some() {
            self.cancel_active_run(cx);
        }

        self.simulations_running.remove(&conversation_id);
        self.running_conversations.remove(&conversation_id);
        self.collapsed_sessions.remove(&conversation_id.0);
        self.thread_item_indices.remove(&conversation_id);

        self.conversations.retain(|c| c.id != conversation_id);
        if let Some(pid) = &project_id {
            if let Some(project) = self.projects.iter_mut().find(|p| p.id == *pid) {
                project.conversations.retain(|id| id != &conversation_id);
            }
        }

        let session_id = proto_session_id(&conversation_id);
        if let Err(err) = self.agent_bridge.delete_session(&session_id) {
            tracing::error!("failed to delete session: {err}");
        }

        let Some(pid) = project_id else {
            self.sync_sidebar_view(cx);
            cx.notify();
            return;
        };

        let remaining_in_project = self
            .projects
            .iter()
            .find(|p| p.id == pid)
            .map(|p| p.conversations.len())
            .unwrap_or(0);

        if remaining_in_project == 0 {
            self.create_conversation_in_project(pid, cx);
            return;
        }

        if was_selected {
            if let Some(next_id) = self
                .projects
                .iter()
                .find(|p| p.id == pid)
                .and_then(|p| p.conversations.first().cloned())
            {
                self.select_conversation(next_id, cx);
            } else {
                self.reselect_after_delete(cx);
            }
            return;
        }

        self.sync_sidebar_view(cx);
        cx.notify();
    }

    fn reselect_after_delete(&mut self, cx: &mut Context<Self>) {
        if let Some(conv_id) = self.conversations.first().map(|c| c.id.clone()) {
            self.select_conversation(conv_id, cx);
            return;
        }

        self.selected_project_id = None;
        self.selected_conversation_id = None;
        self.bootstrap_workspace_session(cx);
    }
}
