//! Sidebar bridge — sync sidebar state with SidebarView.

use gpui::Context;

use super::super::AgentWindow;
use crate::features::shell::state::SidebarSession;

impl AgentWindow {
    pub(crate) fn sync_sidebar_view(&mut self, cx: &mut Context<Self>) {
        let _profile = crate::shared::render_profile::span("AgentWindow::sync_sidebar_view");
        self.ensure_sidebar_view(cx);
        let projects = self.projects.clone();
        let sessions: Vec<SidebarSession> = self
            .conversations
            .iter()
            .map(SidebarSession::from_conversation)
            .collect();
        let selected = self.selected_conversation_id.clone();
        let expanded = self.expanded_items.clone();
        let collapsed = self.sidebar_collapsed;
        let screen = self.screen;
        let Some(sidebar) = self.sidebar_view.clone() else {
            return;
        };
        sidebar.update(cx, |view, cx| {
            view.sync(
                projects, sessions, selected, expanded, collapsed, screen, cx,
            );
        });
    }
}
