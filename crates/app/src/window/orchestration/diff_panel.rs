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

    pub(crate) fn apply_diff_panel_now(&mut self, unified_diff: &str, cx: &mut Context<Self>) {
        self.diff_preview_pending = None;
        self.diff_preview_parse_scheduled = false;
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
