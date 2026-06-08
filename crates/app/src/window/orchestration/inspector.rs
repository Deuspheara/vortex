//! Inspector orchestration — set_inspector_mode, select_inspector_view, select_artifact, select_tool_artifact.

use std::path::{Path, PathBuf};
use std::process::Command;

use gpui::Context;

use super::super::AgentWindow;
use crate::features::agent_activity::components::tool_call::ToolLineRange;
use crate::features::shell::state::{
    ArtifactId, ArtifactKind, ArtifactSelection, DockPlacement, InspectorMode, InspectorTabId,
    InspectorTabKind, InspectorView, ReviewPanelTab, ThreadItem,
};
use crate::features::workspace_layout::state::{
    BOTTOM_PANE_ID, RIGHT_PANE_ID, WorkspaceItemId, WorkspaceTab,
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
        self.sync_inspector_tab_in_workspace_layout(id, None);
        self.apply_inspector_tab_by_id(id);
        self.reveal_dock_for_tab(id, cx);
    }

    pub fn select_inspector_tab(&mut self, tab_id: InspectorTabId, cx: &mut Context<Self>) {
        if self.inspector_tabs.select(tab_id).is_some() {
            self.sync_inspector_tab_in_workspace_layout(tab_id, None);
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
            self.workspace_layout.reorder_item(
                &WorkspaceItemId::inspector_tab(dragged_id),
                &WorkspaceItemId::inspector_tab(target_id),
            );
            cx.notify();
        }
    }

    pub fn close_inspector_tab(&mut self, tab_id: InspectorTabId, cx: &mut Context<Self>) {
        self.inspector_tabs.close(tab_id);
        self.workspace_layout
            .remove_item(&WorkspaceItemId::inspector_tab(tab_id));
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
        self.sync_inspector_tab_in_workspace_layout(id, Some(DockPlacement::Right));
        self.apply_inspector_tab_by_id(id);
        self.set_inspector_mode(InspectorMode::Review, cx);
    }

    pub fn select_artifact(&mut self, artifact_id: ArtifactId, cx: &mut Context<Self>) {
        if let Some(artifact) = self.artifact_store.get(&artifact_id) {
            if matches!(artifact.kind, ArtifactKind::Terminal) {
                let id = self
                    .inspector_tabs
                    .select_artifact(artifact_id.clone(), artifact.title.clone());
                self.sync_inspector_tab_in_workspace_layout(id, None);
                self.apply_inspector_tab_by_id(id);
                self.reveal_dock_for_tab(id, cx);
            } else {
                let id = self.inspector_tabs.select_builtin(InspectorView::Changes);
                self.sync_inspector_tab_in_workspace_layout(id, None);
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

    pub fn open_file_in_external_editor(
        &mut self,
        path: &str,
        _line_range: Option<ToolLineRange>,
        cx: &mut Context<Self>,
    ) {
        let target = self.resolve_external_editor_path(path);
        if !target.exists() {
            tracing::warn!(
                path = %path,
                resolved = %target.display(),
                "tool file link target does not exist"
            );
            cx.notify();
            return;
        }

        if let Err(error) = open_path_with_default_editor(&target) {
            tracing::warn!(
                path = %path,
                resolved = %target.display(),
                error = %error,
                "failed to open tool file link in external editor"
            );
        }
        cx.notify();
    }

    fn resolve_external_editor_path(&self, path: &str) -> PathBuf {
        let target = Path::new(path);
        if target.is_absolute() {
            return target.to_path_buf();
        }
        self.selected_project()
            .map(|project| Path::new(&project.root_path).join(target))
            .unwrap_or_else(|| target.to_path_buf())
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
                    self.sync_inspector_tab_in_workspace_layout(id, None);
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
            self.sync_inspector_tab_in_workspace_layout(tab_id, Some(dock));
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

    fn sync_inspector_tab_in_workspace_layout(
        &mut self,
        tab_id: InspectorTabId,
        placement_override: Option<DockPlacement>,
    ) {
        let Some(tab) = self.inspector_tabs.tabs.iter().find(|tab| tab.id == tab_id) else {
            return;
        };
        let dock = placement_override.unwrap_or(tab.placement);
        let pane_id = match dock {
            DockPlacement::Right => RIGHT_PANE_ID,
            DockPlacement::Bottom => BOTTOM_PANE_ID,
        };
        let item = WorkspaceItemId::inspector_tab(tab_id);
        let workspace_tab = WorkspaceTab::new(item.clone(), tab.title.clone(), tab.closeable);
        if !self
            .workspace_layout
            .move_item_to_pane(&item, pane_id, None)
        {
            self.workspace_layout
                .ensure_tab_in_pane(pane_id, workspace_tab, None);
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

fn open_path_with_default_editor(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn()?.wait()?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .spawn()?
            .wait()?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(path).spawn()?.wait()?;
        return Ok(());
    }
}
