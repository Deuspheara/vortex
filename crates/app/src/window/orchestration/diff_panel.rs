//! Diff panel orchestration — open/close/toggle/set_review_tab/select_diff_file/apply_now.

use gpui::Context;

use super::super::AgentWindow;
use crate::features::shell::state::{DockPlacement, InspectorMode, InspectorView};

impl AgentWindow {
    pub fn open_diff_panel(&mut self, cx: &mut Context<Self>) {
        self.inspector_tabs.select_builtin(InspectorView::Changes);
        self.apply_active_inspector_tab();
        self.set_inspector_mode(InspectorMode::Review, cx);
        self.diff_panel.suppress_auto_open = false;
    }

    pub fn close_diff_panel(&mut self, cx: &mut Context<Self>) {
        self.set_inspector_mode(InspectorMode::Hidden, cx);
    }

    pub fn toggle_diff_panel(&mut self, cx: &mut Context<Self>) {
        let next = self.inspector_mode.toggle_review();
        if next == InspectorMode::Review && self.inspector_view == InspectorView::Terminal {
            self.inspector_tabs.select_builtin(InspectorView::Changes);
            self.apply_active_inspector_tab();
        }
        self.set_inspector_mode(next, cx);
        if next == InspectorMode::Review {
            self.diff_panel.suppress_auto_open = false;
        }
    }

    pub fn set_review_tab(
        &mut self,
        tab: crate::features::shell::state::ReviewPanelTab,
        cx: &mut Context<Self>,
    ) {
        self.diff_panel.active_tab = tab;
        let view = match tab {
            crate::features::shell::state::ReviewPanelTab::Changes => InspectorView::Changes,
            crate::features::shell::state::ReviewPanelTab::Plan => InspectorView::Plan,
        };
        self.inspector_tabs.select_builtin(view);
        self.inspector_view = view;
        cx.notify();
    }

    pub fn select_diff_file(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.diff_panel.files.len() {
            self.diff_panel.selected_file = index;
            cx.notify();
        }
    }

    pub fn toggle_diff_file(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.diff_panel.presentation.collapsed.remove(&path) {
            self.diff_panel.presentation.collapsed.insert(path);
        }
        cx.notify();
    }
    pub fn toggle_diff_collapse_all(&mut self, cx: &mut Context<Self>) {
        if self
            .diff_panel
            .files
            .iter()
            .all(|file| self.diff_panel.presentation.collapsed.contains(&file.path))
        {
            self.diff_panel.presentation.collapsed.clear();
        } else {
            self.diff_panel.presentation.collapsed = self
                .diff_panel
                .files
                .iter()
                .map(|file| file.path.clone())
                .collect();
        }
        cx.notify();
    }
    pub fn set_diff_split(&mut self, split: bool, cx: &mut Context<Self>) {
        self.diff_panel.presentation.split = split;
        cx.notify();
    }
    pub fn toggle_diff_wrap(&mut self, cx: &mut Context<Self>) {
        self.diff_panel.presentation.wrap = !self.diff_panel.presentation.wrap;
        cx.notify();
    }
    pub fn toggle_diff_tree(&mut self, cx: &mut Context<Self>) {
        self.diff_panel.presentation.show_tree = !self.diff_panel.presentation.show_tree;
        cx.notify();
    }

    pub(crate) fn apply_diff_panel_now(&mut self, unified_diff: &str, cx: &mut Context<Self>) {
        self.diff_preview_pending = None;
        self.diff_preview_parse_scheduled = false;
        self.diff_preview_parse_generation = self.diff_preview_parse_generation.wrapping_add(1);
        self.diff_panel.files =
            crate::features::diff_panel::layout::parse_unified_diff(unified_diff);
        self.artifact_store
            .update_diff_files("patch-preview", self.diff_panel.files.clone());
        self.diff_panel.suppress_auto_open = false;
        let id = self.inspector_tabs.select_builtin(InspectorView::Changes);
        self.apply_active_inspector_tab();
        let placement = self
            .inspector_tabs
            .tabs
            .iter()
            .find(|tab| tab.id == id)
            .map(|tab| tab.placement)
            .unwrap_or(DockPlacement::Right);
        match placement {
            DockPlacement::Right => self.set_inspector_mode(InspectorMode::Review, cx),
            DockPlacement::Bottom => {
                self.terminal_panel_open = true;
                cx.notify();
            }
        }
        self.diff_panel.applied = false;
    }
}
