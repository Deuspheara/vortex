//! Inspector orchestration — set_inspector_mode, select_inspector_view, select_artifact, select_tool_artifact.

use gpui::Context;

use super::super::AgentWindow;
use crate::features::shell::state::{
    ArtifactId, ArtifactKind, ArtifactSelection, DockPlacement, InspectorMode, InspectorTabId,
    InspectorTabKind, InspectorView, ReviewPanelTab, ThreadItem,
};

impl AgentWindow {
    pub fn set_inspector_mode(&mut self, mode: InspectorMode, cx: &mut Context<Self>) {
        if mode.is_visible()
            && self
                .inspector_tabs
                .active_for_dock(DockPlacement::Right)
                .is_none()
        {
            self.inspector_tabs.select_builtin(InspectorView::Changes);
        }
        self.inspector_mode = mode;
        self.diff_panel.open = mode.is_visible();
        if mode == InspectorMode::Hidden {
            self.diff_panel.suppress_auto_open = true;
        }
        cx.notify();
    }

    pub fn select_inspector_view(&mut self, view: InspectorView, cx: &mut Context<Self>) {
        let id = self.inspector_tabs.select_builtin(view);
        self.apply_inspector_tab_by_id(id);
        self.reveal_dock_for_tab(id, cx);
    }

    pub fn select_inspector_tab(&mut self, tab_id: InspectorTabId, cx: &mut Context<Self>) {
        if self.inspector_tabs.select(tab_id).is_some() {
            self.apply_inspector_tab_by_id(tab_id);
            self.reveal_dock_for_tab(tab_id, cx);
        }
    }

    pub fn reorder_inspector_tab(
        &mut self,
        dragged_id: InspectorTabId,
        target_id: InspectorTabId,
        cx: &mut Context<Self>,
    ) {
        if self.inspector_tabs.reorder(dragged_id, target_id) {
            cx.notify();
        }
    }

    pub fn close_inspector_tab(&mut self, tab_id: InspectorTabId, cx: &mut Context<Self>) {
        self.inspector_tabs.close(tab_id);
        if self
            .inspector_tabs
            .tabs_for_dock(DockPlacement::Right)
            .is_empty()
        {
            self.set_inspector_mode(InspectorMode::Hidden, cx);
        }
        self.apply_active_inspector_tab();
        cx.notify();
    }

    pub fn new_inspector_tab(&mut self, cx: &mut Context<Self>) {
        let id = self.inspector_tabs.open_empty();
        self.apply_inspector_tab_by_id(id);
        self.set_inspector_mode(InspectorMode::Review, cx);
    }

    pub fn select_artifact(&mut self, artifact_id: ArtifactId, cx: &mut Context<Self>) {
        if let Some(artifact) = self.artifact_store.get(&artifact_id) {
            if matches!(artifact.kind, ArtifactKind::Terminal) {
                let id = self
                    .inspector_tabs
                    .select_artifact(artifact_id.clone(), artifact.title.clone());
                self.apply_inspector_tab_by_id(id);
                self.reveal_dock_for_tab(id, cx);
            } else {
                let id = self.inspector_tabs.select_builtin(InspectorView::Changes);
                self.apply_inspector_tab_by_id(id);
            }
        }
        self.artifact_selection = ArtifactSelection::Selected(artifact_id);
        if self.inspector_mode == InspectorMode::Hidden {
            self.set_inspector_mode(InspectorMode::Compact, cx);
        } else {
            cx.notify();
        }
    }

    pub fn select_tool_artifact(&mut self, item_id: &str, cx: &mut Context<Self>) {
        let artifact_id = ArtifactId::new(format!("tool-{item_id}"));
        if self.artifact_store.get(&artifact_id).is_none() {
            if let Some(conv_id) = self.selected_conversation_id.clone() {
                if let Some(item) = self
                    .conversations
                    .iter()
                    .find(|c| c.id == conv_id)
                    .and_then(|c| c.thread_items.iter().find(|i| i.id() == item_id))
                {
                    if let ThreadItem::ToolCall {
                        tool_name,
                        command,
                        output,
                        ..
                    } = item
                    {
                        let title = self.tool_row_label(tool_name, command.as_deref(), false);
                        let evidence_kind = match tool_name.as_str() {
                            "bash_virtual" | "run_real_command" | "RunCommand" | "shell" => {
                                Some(crate::features::shell::state::ArtifactKind::Terminal)
                            }
                            "fetch_url" | "web_fetch" | "web_search" | "web_extract" => {
                                Some(crate::features::shell::state::ArtifactKind::WebSource)
                            }
                            "browser_snapshot" => {
                                Some(crate::features::shell::state::ArtifactKind::WebSource)
                            }
                            "browser_screenshot" => {
                                Some(crate::features::shell::state::ArtifactKind::Screenshot)
                            }
                            "vision_inspect" => {
                                Some(crate::features::shell::state::ArtifactKind::Vision)
                            }
                            _ => None,
                        };
                        if let Some(kind) = evidence_kind {
                            let full = output.clone().unwrap_or_default();
                            self.artifact_store.upsert(
                                crate::features::shell::state::Artifact::tool_evidence(
                                    artifact_id.0.clone(),
                                    kind,
                                    title,
                                    crate::features::shell::state::excerpt_output(&full),
                                    Some(item_id.to_string()),
                                ),
                            );
                        }
                    }
                }
            }
        }
        self.select_artifact(artifact_id, cx);
    }

    pub fn select_subagent_tab(&mut self, item_id: &str, cx: &mut Context<Self>) {
        if let Some(conv_id) = self.selected_conversation_id.clone() {
            if let Some(ThreadItem::SubagentRun { task, .. }) = self
                .conversations
                .iter()
                .find(|c| c.id == conv_id)
                .and_then(|c| c.thread_items.iter().find(|item| item.id() == item_id))
            {
                self.inspector_tabs
                    .select_subagent(item_id.to_string(), format!("Subagent · {task}"));
                if let Some(id) = self.inspector_tabs.last_selected_id {
                    self.apply_inspector_tab_by_id(id);
                }
                self.set_inspector_mode(InspectorMode::Review, cx);
                return;
            }
        }
        cx.notify();
    }

    pub fn open_context_trace(&mut self, item_id: &str, cx: &mut Context<Self>) {
        self.select_inspector_view(InspectorView::Context, cx);
        if let Some(conv_id) = self.selected_conversation_id.clone() {
            self.update_thread_item(
                conv_id.clone(),
                item_id,
                |item| {
                    if let ThreadItem::ContextTrace { expanded, .. } = item {
                        *expanded = true;
                    }
                },
                cx,
            );
            if let Some(thread) = self.thread_view.clone() {
                let target_id = item_id.to_string();
                thread.update(cx, |view, cx| {
                    view.reveal_item(&target_id);
                    cx.notify();
                });
            }
        }
    }

    pub fn move_inspector_tab_to_dock(
        &mut self,
        tab_id: InspectorTabId,
        dock: DockPlacement,
        cx: &mut Context<Self>,
    ) {
        if self.inspector_tabs.move_tab_to_dock(tab_id, dock) {
            self.apply_inspector_tab_by_id(tab_id);
        }
        match dock {
            DockPlacement::Right => self.set_inspector_mode(InspectorMode::Review, cx),
            DockPlacement::Bottom => {
                self.terminal_panel_open = true;
                cx.notify();
            }
        }
    }

    pub(crate) fn apply_active_inspector_tab(&mut self) {
        let kind = self
            .inspector_tabs
            .last_selected_id
            .and_then(|id| self.tab_kind_for_id(id).cloned())
            .or_else(|| {
                self.inspector_tabs
                    .active_for_dock(DockPlacement::Right)
                    .map(|tab| tab.kind.clone())
            })
            .or_else(|| {
                self.inspector_tabs
                    .active_for_dock(DockPlacement::Bottom)
                    .map(|tab| tab.kind.clone())
            });
        if let Some(kind) = kind.as_ref() {
            self.apply_inspector_kind(kind);
        }
    }

    fn apply_inspector_tab_by_id(&mut self, tab_id: InspectorTabId) {
        if let Some(kind) = self.tab_kind_for_id(tab_id).cloned() {
            self.apply_inspector_kind(&kind);
        }
    }

    fn tab_kind_for_id(&self, tab_id: InspectorTabId) -> Option<&InspectorTabKind> {
        self.inspector_tabs
            .tabs
            .iter()
            .find(|tab| tab.id == tab_id)
            .map(|tab| &tab.kind)
    }

    fn apply_inspector_kind(&mut self, kind: &InspectorTabKind) {
        match kind {
            InspectorTabKind::BuiltIn(InspectorView::Changes) => {
                self.inspector_view = InspectorView::Changes;
                self.diff_panel.active_tab = ReviewPanelTab::Changes;
                self.diff_panel.suppress_auto_open = false;
            }
            InspectorTabKind::BuiltIn(InspectorView::Context) => {
                self.inspector_view = InspectorView::Context;
            }
            InspectorTabKind::BuiltIn(InspectorView::Plan) => {
                self.inspector_view = InspectorView::Plan;
                self.diff_panel.active_tab = ReviewPanelTab::Plan;
                self.diff_panel.suppress_auto_open = false;
            }
            InspectorTabKind::BuiltIn(InspectorView::Terminal)
            | InspectorTabKind::Artifact(_)
            | InspectorTabKind::Subagent(_)
            | InspectorTabKind::Custom(_) => {
                self.inspector_view = InspectorView::Terminal;
            }
        }
    }

    fn reveal_dock_for_tab(&mut self, tab_id: InspectorTabId, cx: &mut Context<Self>) {
        let placement = self
            .inspector_tabs
            .tabs
            .iter()
            .find(|tab| tab.id == tab_id)
            .map(|tab| tab.placement);
        match placement {
            Some(DockPlacement::Right) => self.set_inspector_mode(InspectorMode::Review, cx),
            Some(DockPlacement::Bottom) => {
                self.terminal_panel_open = true;
                cx.notify();
            }
            None => cx.notify(),
        }
    }
}
