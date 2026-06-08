//! Terminal orchestration — new/close/select/spawn terminal tabs, ensure_terminal_tabs.

use std::path::{Path, PathBuf};

use gpui::{Context, prelude::*};

use super::super::AgentWindow;
use super::super::types::{TerminalTab, TerminalTabGroup};
use crate::features::terminal::components::terminal_view::TerminalView;
use crate::features::terminal::layout::TerminalTabVm;
use crate::features::workspace_layout::state::{BOTTOM_PANE_ID, WorkspaceItemId, WorkspaceTab};
use crate::tokens::Tokens;

impl AgentWindow {
    pub fn toggle_terminal_panel(&mut self, cx: &mut Context<Self>) {
        self.terminal_panel_open = !self.terminal_panel_open;
        if self.terminal_panel_open {
            self.ensure_terminal_tabs_for_project(cx);
        }
        cx.notify();
    }

    pub fn set_bottom_panel_height(&mut self, height: f32, cx: &mut Context<Self>) {
        self.bottom_panel_height = height.clamp(
            Tokens::BOTTOM_PANEL_MIN_HEIGHT,
            Tokens::BOTTOM_PANEL_MAX_HEIGHT,
        );
        // Canvas bounds detection inside TerminalView handles the actual
        // PTY resize on the next frame — no need to force a width here.
        cx.notify();
    }

    pub fn set_right_dock_width(&mut self, width: f32, cx: &mut Context<Self>) {
        self.right_dock_width =
            width.clamp(Tokens::DIFF_PANEL_MIN_WIDTH, Tokens::DIFF_PANEL_MAX_WIDTH);
        cx.notify();
    }

    fn terminal_body_size(&self) -> (f32, f32) {
        let body_h = self.bottom_panel_height - Tokens::terminal_chrome_height();
        let body_w = 800.0_f32;
        (body_w.max(1.0), body_h.max(Tokens::TERMINAL_CELL_HEIGHT))
    }

    fn terminal_grid_size(&self, content_width: f32, content_height: f32) -> (u16, u16) {
        let cols = (content_width / Tokens::TERMINAL_CELL_WIDTH)
            .floor()
            .max(2.0) as u16;
        let rows = (content_height / Tokens::TERMINAL_CELL_HEIGHT)
            .floor()
            .max(1.0) as u16;
        (cols, rows)
    }

    pub fn new_terminal_tab(&mut self, cx: &mut Context<Self>) {
        let Some(project_id) = self.selected_project_id.clone() else {
            return;
        };
        let Some(root) = self
            .projects
            .iter()
            .find(|p| p.id == project_id)
            .map(|p| p.root_path.clone())
        else {
            return;
        };

        self.ensure_terminal_tabs_for_project(cx);

        let (w, h) = self.terminal_body_size();
        let (cols, rows) = self.terminal_grid_size(w, h);
        let Some(group) = self.terminal_tab_groups.get_mut(&project_id) else {
            return;
        };

        let cwd = group
            .active_tab()
            .map(|tab| tab.session.meta().cwd.clone())
            .unwrap_or_else(|| PathBuf::from(&root));
        let tab_id = group.next_tab_id;
        if let Some(mut tab) = Self::spawn_terminal_tab(&cwd, tab_id, cols, rows, cx) {
            group.next_tab_id += 1;

            // ── Unique label with (N) suffix ────────────────────
            let base = tab.label.clone();
            let existing_count = group
                .tabs
                .iter()
                .filter(|t| {
                    t.label == base
                        || t.label.starts_with(&base)
                            && t.label[base.len()..].starts_with(" (")
                            && t.label[base.len() + 2..]
                                .trim_end_matches(')')
                                .parse::<u32>()
                                .is_ok()
                })
                .count();
            if existing_count > 0 {
                tab.label = format!("{} ({})", base, existing_count + 1);
            }

            group.active_tab_id = tab.id;
            self.workspace_layout.ensure_tab_in_pane(
                BOTTOM_PANE_ID,
                WorkspaceTab::new(
                    WorkspaceItemId::terminal_session(tab.id),
                    tab.label.clone(),
                    true,
                ),
                None,
            );
            group.tabs.push(tab);
            self.attach_active_terminal_tab(&project_id, cx);
        }
        cx.notify();
    }

    pub fn close_terminal_tab(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        let Some(project_id) = self.selected_project_id.clone() else {
            return;
        };
        let Some(group) = self.terminal_tab_groups.get_mut(&project_id) else {
            return;
        };
        if group.tabs.len() <= 1 {
            return;
        }
        group.tabs.retain(|tab| tab.id != tab_id);
        self.workspace_layout
            .remove_item(&WorkspaceItemId::terminal_session(tab_id));
        if group.active_tab_id == tab_id {
            group.active_tab_id = group.tabs.last().map(|tab| tab.id).unwrap_or(0);
        }
        self.attach_active_terminal_tab(&project_id, cx);
        cx.notify();
    }

    pub fn select_terminal_tab(&mut self, tab_id: u64, cx: &mut Context<Self>) {
        let Some(project_id) = self.selected_project_id.clone() else {
            return;
        };
        let Some(group) = self.terminal_tab_groups.get_mut(&project_id) else {
            return;
        };
        if group.tabs.iter().any(|tab| tab.id == tab_id) {
            group.active_tab_id = tab_id;
            self.workspace_layout
                .select_item(&WorkspaceItemId::terminal_session(tab_id));
            self.attach_active_terminal_tab(&project_id, cx);
            cx.notify();
        }
    }

    pub fn reorder_terminal_tab(
        &mut self,
        dragged_id: u64,
        target_id: u64,
        cx: &mut Context<Self>,
    ) {
        let Some(project_id) = self.selected_project_id.clone() else {
            return;
        };
        let Some(group) = self.terminal_tab_groups.get_mut(&project_id) else {
            return;
        };
        if dragged_id == target_id {
            return;
        }
        let Some(from_ix) = group.tabs.iter().position(|tab| tab.id == dragged_id) else {
            return;
        };
        let Some(to_ix) = group.tabs.iter().position(|tab| tab.id == target_id) else {
            return;
        };

        let tab = group.tabs.remove(from_ix);
        let insert_ix = if from_ix < to_ix { to_ix - 1 } else { to_ix };
        group.tabs.insert(insert_ix, tab);
        self.workspace_layout.reorder_item(
            &WorkspaceItemId::terminal_session(dragged_id),
            &WorkspaceItemId::terminal_session(target_id),
        );
        cx.notify();
    }

    fn prune_terminal_tab_groups(&mut self) {
        let keep = self.selected_project_id.clone();
        self.terminal_tab_groups
            .retain(|project_id, _| Some(project_id) == keep.as_ref());
    }

    fn spawn_terminal_tab(
        cwd: &Path,
        tab_id: u64,
        cols: u16,
        rows: u16,
        cx: &mut Context<Self>,
    ) -> Option<TerminalTab> {
        match terminal::TerminalSession::spawn(cwd, cols, rows, Tokens::terminal_theme()) {
            Ok(session) => {
                let session = std::sync::Arc::new(session);
                let label = session
                    .meta()
                    .cwd
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Terminal")
                    .to_string();
                let view = cx.new(TerminalView::new);
                let session_for_view = session.clone();
                view.update(cx, |tv, cx| tv.attach_session(session_for_view, cx));
                Some(TerminalTab {
                    id: tab_id,
                    label,
                    session,
                    view,
                })
            }
            Err(err) => {
                tracing::warn!("terminal spawn failed for {}: {err}", cwd.display());
                None
            }
        }
    }

    pub(crate) fn ensure_terminal_tabs_for_project(&mut self, cx: &mut Context<Self>) {
        let Some(project_id) = self.selected_project_id.clone() else {
            return;
        };
        let Some(project) = self.projects.iter().find(|p| p.id == project_id) else {
            return;
        };
        let root = project.root_path.clone();
        self.prune_terminal_tab_groups();

        if !self.terminal_tab_groups.contains_key(&project_id) {
            let mut group = TerminalTabGroup::new();
            let (w, h) = self.terminal_body_size();
            let (cols, rows) = self.terminal_grid_size(w, h);
            if let Some(tab) =
                Self::spawn_terminal_tab(Path::new(&root), group.next_tab_id, cols, rows, cx)
            {
                group.active_tab_id = tab.id;
                group.next_tab_id += 1;
                group.tabs.push(tab);
                self.terminal_tab_groups.insert(project_id.clone(), group);
                self.attach_active_terminal_tab(&project_id, cx);
            }
            return;
        }

        if self.terminal_panel_open {
            self.attach_active_terminal_tab(&project_id, cx);
        }
    }

    fn attach_active_terminal_tab(
        &mut self,
        project_id: &crate::features::shell::state::ProjectId,
        cx: &mut Context<Self>,
    ) {
        let Some(group) = self.terminal_tab_groups.get(project_id) else {
            return;
        };
        let Some(tab) = group.active_tab() else {
            return;
        };
        let session = tab.session.clone();
        let view = tab.view.clone();
        view.update(cx, |tv, cx| tv.attach_session(session, cx));
    }

    pub(crate) fn active_terminal_view(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Entity<TerminalView>> {
        self.ensure_terminal_tabs_for_project(cx);
        self.selected_project_id
            .as_ref()
            .and_then(|project_id| self.terminal_tab_groups.get(project_id))
            .and_then(|group| group.active_tab())
            .map(|tab| tab.view.clone())
    }

    pub(crate) fn terminal_tabs_vm(&self) -> Vec<TerminalTabVm> {
        let Some(project_id) = &self.selected_project_id else {
            return Vec::new();
        };
        self.terminal_tab_groups
            .get(project_id)
            .map(|group| {
                group
                    .tabs
                    .iter()
                    .map(|tab| TerminalTabVm {
                        id: tab.id,
                        label: tab.label.clone(),
                        selected: tab.id == group.active_tab_id,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn selected_project_details(&self) -> (String, String, String) {
        if let Some(pid) = &self.selected_project_id {
            if let Some(project) = self.projects.iter().find(|p| p.id == *pid) {
                return (
                    project.name.clone(),
                    project.root_path.clone(),
                    project.git_branch.clone(),
                );
            }
        }

        if let Some(cid) = &self.selected_conversation_id {
            if let Some(conv) = self.conversations.iter().find(|c| c.id == *cid) {
                if let Some(project) = self.projects.iter().find(|p| p.id == conv.project_id) {
                    return (
                        project.name.clone(),
                        project.root_path.clone(),
                        project.git_branch.clone(),
                    );
                }
            }
        }

        ("No project".into(), ".".into(), "main".into())
    }
}
