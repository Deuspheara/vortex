//! Artifact inspector pane state.
//!
//! The right panel is modeled as a user-customizable tab group. Built-in tabs
//! keep today's review surfaces working, while artifact/custom slots give the
//! panel a stable extension point for browser/search/simulator views later.

use crate::features::inspector::artifact::ArtifactId;
use crate::features::workspace_layout::state::WorkspaceItemId;
use crate::tokens::Tokens;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DockPlacement {
    Bottom,
    #[default]
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum InspectorView {
    #[default]
    Changes,
    Context,
    Plan,
    Terminal,
}

impl InspectorView {
    pub fn label(self) -> &'static str {
        match self {
            Self::Changes => "Changes",
            Self::Context => "Context",
            Self::Plan => "Plan",
            Self::Terminal => "Terminal",
        }
    }
}

pub type InspectorTabId = u64;

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InspectorCustomSlot {
    Browser,
    Search,
    AndroidSimulator,
    IosSimulator,
    Empty,
    Named(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InspectorTabKind {
    BuiltIn(InspectorView),
    Artifact(ArtifactId),
    Subagent(String),
    Custom(InspectorCustomSlot),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectorTab {
    pub id: InspectorTabId,
    pub title: String,
    pub kind: InspectorTabKind,
    pub placement: DockPlacement,
    pub closeable: bool,
}

impl InspectorTab {
    pub fn workspace_item_id(&self) -> WorkspaceItemId {
        WorkspaceItemId::inspector_tab(self.id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectorTabs {
    pub tabs: Vec<InspectorTab>,
    pub active_right_id: Option<InspectorTabId>,
    pub active_bottom_id: Option<InspectorTabId>,
    pub last_selected_id: Option<InspectorTabId>,
    next_id: InspectorTabId,
}

impl Default for InspectorTabs {
    fn default() -> Self {
        let mut tabs = Self {
            tabs: Vec::new(),
            active_right_id: None,
            active_bottom_id: None,
            last_selected_id: None,
            next_id: 1,
        };
        tabs.ensure_builtin(InspectorView::Changes);
        tabs.ensure_builtin(InspectorView::Context);
        tabs.ensure_builtin(InspectorView::Plan);
        tabs.ensure_builtin(InspectorView::Terminal);
        tabs.push_tab(
            "Android",
            InspectorTabKind::Custom(InspectorCustomSlot::AndroidSimulator),
            DockPlacement::Right,
            true,
        );
        tabs.select_builtin(InspectorView::Changes);
        tabs.active_bottom_id = Some(tabs.ensure_builtin(InspectorView::Terminal));
        tabs
    }
}

impl InspectorTabs {
    pub fn active(&self) -> Option<&InspectorTab> {
        self.active_for_dock(DockPlacement::Right)
    }

    pub fn active_for_dock(&self, dock: DockPlacement) -> Option<&InspectorTab> {
        self.active_id_for_dock(dock)
            .and_then(|id| self.tabs.iter().find(|tab| tab.id == id))
    }

    pub fn active_id_for_dock(&self, dock: DockPlacement) -> Option<InspectorTabId> {
        match dock {
            DockPlacement::Right => self.active_right_id,
            DockPlacement::Bottom => self.active_bottom_id,
        }
    }

    pub fn tabs_for_dock(&self, dock: DockPlacement) -> Vec<&InspectorTab> {
        self.tabs
            .iter()
            .filter(|tab| tab.placement == dock)
            .collect()
    }

    pub fn select(&mut self, id: InspectorTabId) -> Option<InspectorTabKind> {
        if let Some((placement, kind)) = self
            .tabs
            .iter()
            .find(|tab| tab.id == id)
            .map(|tab| (tab.placement, tab.kind.clone()))
        {
            self.set_active(placement, Some(id));
            Some(kind)
        } else {
            None
        }
    }

    pub fn reorder(&mut self, dragged_id: InspectorTabId, target_id: InspectorTabId) -> bool {
        if dragged_id == target_id {
            return false;
        }
        let Some(from_ix) = self.tabs.iter().position(|tab| tab.id == dragged_id) else {
            return false;
        };
        let Some(to_ix) = self.tabs.iter().position(|tab| tab.id == target_id) else {
            return false;
        };
        if self.tabs[from_ix].placement != self.tabs[to_ix].placement {
            return false;
        }

        let tab = self.tabs.remove(from_ix);
        let insert_ix = if from_ix < to_ix { to_ix - 1 } else { to_ix };
        self.tabs.insert(insert_ix, tab);
        true
    }

    pub fn close(&mut self, id: InspectorTabId) -> Option<InspectorTabKind> {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return self.active().map(|tab| tab.kind.clone());
        };
        if !self.tabs[index].closeable {
            return self.active().map(|tab| tab.kind.clone());
        }

        let placement = self.tabs[index].placement;
        let was_active = self.active_id_for_dock(placement) == Some(id);
        self.tabs.remove(index);
        if was_active {
            self.set_active(
                placement,
                self.fallback_active_for_dock(placement, index)
                    .map(|tab| tab.id),
            );
        }

        self.active().map(|tab| tab.kind.clone())
    }

    pub fn select_builtin(&mut self, view: InspectorView) -> InspectorTabId {
        let id = self.ensure_builtin(view);
        if let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) {
            self.set_active(tab.placement, Some(id));
        }
        id
    }

    pub fn ensure_builtin(&mut self, view: InspectorView) -> InspectorTabId {
        if let Some(tab) = self
            .tabs
            .iter()
            .find(|tab| matches!(tab.kind, InspectorTabKind::BuiltIn(existing) if existing == view))
        {
            return tab.id;
        }

        self.push_tab(
            Self::builtin_title(view),
            InspectorTabKind::BuiltIn(view),
            Self::builtin_placement(view),
            true,
        )
    }

    pub fn select_artifact(
        &mut self,
        artifact_id: ArtifactId,
        title: impl Into<String>,
    ) -> InspectorTabId {
        if let Some((placement, id)) = self
            .tabs
            .iter()
            .find(|tab| matches!(&tab.kind, InspectorTabKind::Artifact(id) if id == &artifact_id))
            .map(|tab| (tab.placement, tab.id))
        {
            self.set_active(placement, Some(id));
            return id;
        }

        let id = self.push_tab(
            title,
            InspectorTabKind::Artifact(artifact_id),
            DockPlacement::Right,
            true,
        );
        self.set_active(DockPlacement::Right, Some(id));
        id
    }

    pub fn select_subagent(
        &mut self,
        item_id: impl Into<String>,
        title: impl Into<String>,
    ) -> InspectorTabId {
        let item_id = item_id.into();
        if let Some((placement, id)) = self.tabs.iter().find(
            |tab| matches!(&tab.kind, InspectorTabKind::Subagent(existing) if existing == &item_id),
        )
        .map(|tab| (tab.placement, tab.id))
        {
            self.set_active(placement, Some(id));
            return id;
        }

        let id = self.push_tab(
            title,
            InspectorTabKind::Subagent(item_id),
            DockPlacement::Right,
            true,
        );
        self.set_active(DockPlacement::Right, Some(id));
        id
    }

    pub fn open_empty(&mut self) -> InspectorTabId {
        let ordinal = self
            .tabs
            .iter()
            .filter(|tab| matches!(tab.kind, InspectorTabKind::Custom(_)))
            .count()
            + 1;
        let id = self.push_tab(
            format!("Panel {ordinal}"),
            InspectorTabKind::Custom(InspectorCustomSlot::Empty),
            DockPlacement::Right,
            true,
        );
        self.set_active(DockPlacement::Right, Some(id));
        id
    }

    pub fn move_tab_to_dock(&mut self, id: InspectorTabId, dock: DockPlacement) -> bool {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return false;
        };
        let from_dock = self.tabs[index].placement;
        if from_dock == dock {
            self.set_active(dock, Some(id));
            return false;
        }

        self.tabs[index].placement = dock;
        if self.active_id_for_dock(from_dock) == Some(id) {
            self.set_active(
                from_dock,
                self.fallback_active_for_dock(from_dock, index)
                    .map(|tab| tab.id),
            );
        }
        self.set_active(dock, Some(id));
        true
    }

    #[allow(dead_code)]
    pub fn active_builtin_view(&self) -> Option<InspectorView> {
        match self
            .active_for_dock(DockPlacement::Right)
            .map(|tab| &tab.kind)
        {
            Some(InspectorTabKind::BuiltIn(view)) => Some(*view),
            _ => None,
        }
    }

    fn push_tab(
        &mut self,
        title: impl Into<String>,
        kind: InspectorTabKind,
        placement: DockPlacement,
        closeable: bool,
    ) -> InspectorTabId {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(InspectorTab {
            id,
            title: title.into(),
            kind,
            placement,
            closeable,
        });
        id
    }

    fn set_active(&mut self, dock: DockPlacement, id: Option<InspectorTabId>) {
        if id.is_some() {
            self.last_selected_id = id;
        }
        match dock {
            DockPlacement::Right => self.active_right_id = id,
            DockPlacement::Bottom => self.active_bottom_id = id,
        }
    }

    fn fallback_active_for_dock(
        &self,
        dock: DockPlacement,
        removed_index: usize,
    ) -> Option<&InspectorTab> {
        let dock_tabs: Vec<&InspectorTab> = self.tabs_for_dock(dock);
        dock_tabs
            .iter()
            .find(|tab| {
                self.tabs
                    .iter()
                    .position(|candidate| candidate.id == tab.id)
                    .is_some_and(|ix| ix >= removed_index)
            })
            .copied()
            .or_else(|| dock_tabs.last().copied())
    }

    fn builtin_title(view: InspectorView) -> &'static str {
        match view {
            InspectorView::Changes => "Changes",
            InspectorView::Context => "Context",
            InspectorView::Plan => "Plan",
            InspectorView::Terminal => "Terminal",
        }
    }

    fn builtin_placement(view: InspectorView) -> DockPlacement {
        match view {
            InspectorView::Terminal => DockPlacement::Bottom,
            InspectorView::Changes | InspectorView::Context | InspectorView::Plan => {
                DockPlacement::Right
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum InspectorMode {
    #[default]
    Hidden,
    /// Row selection or lightweight review (~360–420px).
    Compact,
    /// Full review surface (~520–680px).
    Review,
}

impl InspectorMode {
    pub fn is_visible(self) -> bool {
        !matches!(self, Self::Hidden)
    }

    pub fn width_px(self) -> f32 {
        match self {
            Self::Hidden => 0.0,
            Self::Compact => Tokens::INSPECTOR_WIDTH_COMPACT,
            Self::Review => Tokens::INSPECTOR_WIDTH_REVIEW,
        }
    }

    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            Self::Hidden => "Hidden",
            Self::Compact => "Review",
            Self::Review => "Review",
        }
    }

    pub fn toggle_review(self) -> Self {
        match self {
            Self::Review => Self::Hidden,
            _ => Self::Review,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DockPlacement, InspectorTabKind, InspectorTabs, InspectorView};

    #[test]
    fn moves_terminal_between_docks_without_duplication() {
        let mut tabs = InspectorTabs::default();
        let terminal_id = tabs.ensure_builtin(InspectorView::Terminal);

        assert_eq!(
            tabs.tabs
                .iter()
                .filter(|tab| matches!(
                    tab.kind,
                    InspectorTabKind::BuiltIn(InspectorView::Terminal)
                ))
                .count(),
            1
        );
        assert!(tabs.move_tab_to_dock(terminal_id, DockPlacement::Right));
        assert_eq!(
            tabs.tabs
                .iter()
                .find(|tab| tab.id == terminal_id)
                .map(|tab| tab.placement),
            Some(DockPlacement::Right)
        );
        assert_eq!(
            tabs.active_id_for_dock(DockPlacement::Right),
            Some(terminal_id)
        );
    }

    #[test]
    fn moving_plan_to_bottom_keeps_single_instance() {
        let mut tabs = InspectorTabs::default();
        let plan_id = tabs.ensure_builtin(InspectorView::Plan);

        assert!(tabs.move_tab_to_dock(plan_id, DockPlacement::Bottom));
        assert_eq!(
            tabs.tabs
                .iter()
                .filter(|tab| matches!(tab.kind, InspectorTabKind::BuiltIn(InspectorView::Plan)))
                .count(),
            1
        );
        assert_eq!(
            tabs.tabs
                .iter()
                .find(|tab| tab.id == plan_id)
                .map(|tab| tab.placement),
            Some(DockPlacement::Bottom)
        );
    }

    #[test]
    fn per_dock_active_selection_is_independent() {
        let mut tabs = InspectorTabs::default();
        let plan_id = tabs.ensure_builtin(InspectorView::Plan);
        let terminal_id = tabs.ensure_builtin(InspectorView::Terminal);

        let changes_id = tabs.select_builtin(InspectorView::Changes);
        tabs.select(terminal_id);
        assert!(tabs.move_tab_to_dock(plan_id, DockPlacement::Bottom));

        assert_eq!(
            tabs.active_id_for_dock(DockPlacement::Right),
            Some(changes_id)
        );
        assert_eq!(
            tabs.active_id_for_dock(DockPlacement::Bottom),
            Some(plan_id)
        );
    }
}
