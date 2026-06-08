//! Persistent workspace layout tree.
//!
//! This module models panes, splits, and tab placement independently from the
//! current root renderer. Window orchestration can apply these helpers when it
//! starts wiring drag/drop and keyboard commands into the workbench.

use serde::{Deserialize, Serialize};

pub type WorkspacePaneId = u64;
pub type WorkspaceSplitId = u64;

pub const MAIN_PANE_ID: WorkspacePaneId = 1;
pub const RIGHT_PANE_ID: WorkspacePaneId = 2;
pub const BOTTOM_PANE_ID: WorkspacePaneId = 3;
const VERTICAL_SPLIT_ID: WorkspaceSplitId = 1;
const HORIZONTAL_SPLIT_ID: WorkspaceSplitId = 2;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkspaceItemId {
    ConversationThread,
    InspectorTab { tab_id: u64 },
    TerminalSession { tab_id: u64 },
    Custom { id: String },
}

impl WorkspaceItemId {
    pub fn inspector_tab(tab_id: u64) -> Self {
        Self::InspectorTab { tab_id }
    }

    pub fn terminal_session(tab_id: u64) -> Self {
        Self::TerminalSession { tab_id }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceTab {
    pub item: WorkspaceItemId,
    pub title: String,
    #[serde(default)]
    pub closeable: bool,
}

impl WorkspaceTab {
    pub fn new(item: WorkspaceItemId, title: impl Into<String>, closeable: bool) -> Self {
        Self {
            item,
            title: title.into(),
            closeable,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspacePaneRole {
    Primary,
    Inspector,
    Bottom,
    Scratch,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspacePane {
    pub id: WorkspacePaneId,
    pub role: WorkspacePaneRole,
    pub tabs: Vec<WorkspaceTab>,
    pub active_item: Option<WorkspaceItemId>,
}

impl WorkspacePane {
    pub fn new(id: WorkspacePaneId, role: WorkspacePaneRole, tabs: Vec<WorkspaceTab>) -> Self {
        let active_item = tabs.first().map(|tab| tab.item.clone());
        Self {
            id,
            role,
            tabs,
            active_item,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceSplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceSplitSide {
    Before,
    After,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceSplit {
    pub id: WorkspaceSplitId,
    pub axis: WorkspaceSplitAxis,
    /// Fraction of available space assigned to the first child.
    pub ratio: f32,
    pub first: Box<WorkspaceNode>,
    pub second: Box<WorkspaceNode>,
}

impl WorkspaceSplit {
    fn new(
        id: WorkspaceSplitId,
        axis: WorkspaceSplitAxis,
        ratio: f32,
        first: WorkspaceNode,
        second: WorkspaceNode,
    ) -> Self {
        Self {
            id,
            axis,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkspaceNode {
    Split(WorkspaceSplit),
    Pane(WorkspacePane),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceLayoutState {
    pub root: WorkspaceNode,
    pub active_pane: Option<WorkspacePaneId>,
    next_pane_id: WorkspacePaneId,
    next_split_id: WorkspaceSplitId,
}

impl Default for WorkspaceLayoutState {
    fn default() -> Self {
        let main = WorkspaceNode::Pane(WorkspacePane::new(
            MAIN_PANE_ID,
            WorkspacePaneRole::Primary,
            vec![WorkspaceTab::new(
                WorkspaceItemId::ConversationThread,
                "Thread",
                false,
            )],
        ));
        let right = WorkspaceNode::Pane(WorkspacePane::new(
            RIGHT_PANE_ID,
            WorkspacePaneRole::Inspector,
            vec![
                WorkspaceTab::new(WorkspaceItemId::inspector_tab(1), "Changes", true),
                WorkspaceTab::new(WorkspaceItemId::inspector_tab(2), "Context", true),
                WorkspaceTab::new(WorkspaceItemId::inspector_tab(3), "Plan", true),
            ],
        ));
        let bottom = WorkspaceNode::Pane(WorkspacePane::new(
            BOTTOM_PANE_ID,
            WorkspacePaneRole::Bottom,
            vec![WorkspaceTab::new(
                WorkspaceItemId::terminal_session(1),
                "Terminal",
                true,
            )],
        ));
        let content = WorkspaceNode::Split(WorkspaceSplit::new(
            VERTICAL_SPLIT_ID,
            WorkspaceSplitAxis::Vertical,
            0.68,
            main,
            bottom,
        ));

        Self {
            root: WorkspaceNode::Split(WorkspaceSplit::new(
                HORIZONTAL_SPLIT_ID,
                WorkspaceSplitAxis::Horizontal,
                0.74,
                content,
                right,
            )),
            active_pane: Some(MAIN_PANE_ID),
            next_pane_id: BOTTOM_PANE_ID + 1,
            next_split_id: HORIZONTAL_SPLIT_ID + 1,
        }
    }
}

impl WorkspaceLayoutState {
    pub fn reset_to_default(&mut self) {
        *self = Self::default();
    }

    pub fn pane(&self, pane_id: WorkspacePaneId) -> Option<&WorkspacePane> {
        find_pane(&self.root, pane_id)
    }

    pub fn panes(&self) -> Vec<&WorkspacePane> {
        let mut panes = Vec::new();
        collect_panes(&self.root, &mut panes);
        panes
    }

    pub fn pane_of_item(&self, item: &WorkspaceItemId) -> Option<WorkspacePaneId> {
        self.panes()
            .into_iter()
            .find(|pane| pane.tabs.iter().any(|tab| &tab.item == item))
            .map(|pane| pane.id)
    }

    pub fn select_item(&mut self, item: &WorkspaceItemId) -> bool {
        let Some(pane_id) = self.pane_of_item(item) else {
            return false;
        };
        let Some(pane) = find_pane_mut(&mut self.root, pane_id) else {
            return false;
        };
        pane.active_item = Some(item.clone());
        self.active_pane = Some(pane_id);
        true
    }

    pub fn reorder_item(&mut self, item: &WorkspaceItemId, target: &WorkspaceItemId) -> bool {
        if item == target {
            return false;
        }

        let Some(pane_id) = self.pane_of_item(item) else {
            return false;
        };
        if self.pane_of_item(target) != Some(pane_id) {
            return false;
        }

        let Some(pane) = find_pane_mut(&mut self.root, pane_id) else {
            return false;
        };
        let Some(from_ix) = pane.tabs.iter().position(|tab| &tab.item == item) else {
            return false;
        };
        let Some(to_ix) = pane.tabs.iter().position(|tab| &tab.item == target) else {
            return false;
        };

        let tab = pane.tabs.remove(from_ix);
        let insert_ix = if from_ix < to_ix { to_ix - 1 } else { to_ix };
        pane.tabs.insert(insert_ix, tab);
        pane.active_item = Some(item.clone());
        self.active_pane = Some(pane_id);
        true
    }

    pub fn move_item_to_pane(
        &mut self,
        item: &WorkspaceItemId,
        target_pane_id: WorkspacePaneId,
        index: Option<usize>,
    ) -> bool {
        let Some(source_pane_id) = self.pane_of_item(item) else {
            return false;
        };
        if self.pane(target_pane_id).is_none() {
            return false;
        }
        if source_pane_id == target_pane_id {
            return self.move_item_within_pane(item, target_pane_id, index);
        }

        let Some(tab) = remove_tab(&mut self.root, item) else {
            return false;
        };
        let Some(target_pane) = find_pane_mut(&mut self.root, target_pane_id) else {
            return false;
        };
        insert_tab(target_pane, tab, index);
        target_pane.active_item = Some(item.clone());
        self.active_pane = Some(target_pane_id);
        true
    }

    pub fn ensure_tab_in_pane(
        &mut self,
        target_pane_id: WorkspacePaneId,
        tab: WorkspaceTab,
        index: Option<usize>,
    ) -> bool {
        if self.pane_of_item(&tab.item).is_some() {
            return self.move_item_to_pane(&tab.item, target_pane_id, index);
        }

        let item = tab.item.clone();
        let Some(target_pane) = find_pane_mut(&mut self.root, target_pane_id) else {
            return false;
        };
        insert_tab(target_pane, tab, index);
        target_pane.active_item = Some(item);
        self.active_pane = Some(target_pane_id);
        true
    }

    pub fn remove_item(&mut self, item: &WorkspaceItemId) -> bool {
        let Some(pane_id) = self.pane_of_item(item) else {
            return false;
        };
        if remove_tab(&mut self.root, item).is_none() {
            return false;
        }
        if let Some(pane) = find_pane_mut(&mut self.root, pane_id) {
            if pane.active_item.as_ref() == Some(item) {
                pane.active_item = pane.tabs.first().map(|tab| tab.item.clone());
            }
        }
        true
    }

    pub fn move_item_to_new_split(
        &mut self,
        item: &WorkspaceItemId,
        sibling_pane_id: WorkspacePaneId,
        axis: WorkspaceSplitAxis,
        side: WorkspaceSplitSide,
        ratio: f32,
    ) -> Option<WorkspacePaneId> {
        if self.pane(sibling_pane_id).is_none() {
            return None;
        }
        let tab = remove_tab(&mut self.root, item)?;

        let pane_id = self.allocate_pane_id();
        let split_id = self.allocate_split_id();
        let new_pane = WorkspaceNode::Pane(WorkspacePane {
            id: pane_id,
            role: WorkspacePaneRole::Scratch,
            tabs: vec![tab],
            active_item: Some(item.clone()),
        });

        if replace_pane_with_split(
            &mut self.root,
            sibling_pane_id,
            split_id,
            axis,
            side,
            ratio,
            new_pane,
        ) {
            self.active_pane = Some(pane_id);
            Some(pane_id)
        } else {
            None
        }
    }

    pub fn apply_command(&mut self, command: WorkspaceLayoutCommand) -> bool {
        match command {
            WorkspaceLayoutCommand::Reset(scope) => match scope {
                WorkspaceResetScope::All => {
                    self.reset_to_default();
                    true
                }
                WorkspaceResetScope::Pane(pane_id) => self.reset_pane(pane_id),
            },
            WorkspaceLayoutCommand::SelectItem(item) => self.select_item(&item),
            WorkspaceLayoutCommand::ReorderItem { item, target } => {
                self.reorder_item(&item, &target)
            }
            WorkspaceLayoutCommand::MoveItem {
                item,
                target_pane_id,
                index,
            } => self.move_item_to_pane(&item, target_pane_id, index),
        }
    }

    pub fn reset_command(scope: WorkspaceResetScope) -> WorkspaceLayoutCommand {
        WorkspaceLayoutCommand::Reset(scope)
    }

    pub fn move_active_item_command(
        &self,
        target_pane_id: WorkspacePaneId,
        index: Option<usize>,
    ) -> Option<WorkspaceLayoutCommand> {
        let pane = self.active_pane.and_then(|pane_id| self.pane(pane_id))?;
        let item = pane.active_item.clone()?;
        Some(WorkspaceLayoutCommand::MoveItem {
            item,
            target_pane_id,
            index,
        })
    }

    fn move_item_within_pane(
        &mut self,
        item: &WorkspaceItemId,
        pane_id: WorkspacePaneId,
        index: Option<usize>,
    ) -> bool {
        let Some(pane) = find_pane_mut(&mut self.root, pane_id) else {
            return false;
        };
        let Some(from_ix) = pane.tabs.iter().position(|tab| &tab.item == item) else {
            return false;
        };
        let tab = pane.tabs.remove(from_ix);
        insert_tab(pane, tab, index);
        pane.active_item = Some(item.clone());
        self.active_pane = Some(pane_id);
        true
    }

    fn reset_pane(&mut self, pane_id: WorkspacePaneId) -> bool {
        let replacement = Self::default().pane(pane_id).cloned();
        let Some(default_pane) = replacement else {
            return false;
        };
        let Some(pane) = find_pane_mut(&mut self.root, pane_id) else {
            return false;
        };
        *pane = default_pane;
        self.active_pane = Some(pane_id);
        true
    }

    fn allocate_pane_id(&mut self) -> WorkspacePaneId {
        let id = self.next_pane_id;
        self.next_pane_id += 1;
        id
    }

    fn allocate_split_id(&mut self) -> WorkspaceSplitId {
        let id = self.next_split_id;
        self.next_split_id += 1;
        id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceLayoutCommand {
    Reset(WorkspaceResetScope),
    SelectItem(WorkspaceItemId),
    ReorderItem {
        item: WorkspaceItemId,
        target: WorkspaceItemId,
    },
    MoveItem {
        item: WorkspaceItemId,
        target_pane_id: WorkspacePaneId,
        index: Option<usize>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceResetScope {
    All,
    Pane(WorkspacePaneId),
}

fn find_pane(node: &WorkspaceNode, pane_id: WorkspacePaneId) -> Option<&WorkspacePane> {
    match node {
        WorkspaceNode::Pane(pane) if pane.id == pane_id => Some(pane),
        WorkspaceNode::Pane(_) => None,
        WorkspaceNode::Split(split) => {
            find_pane(&split.first, pane_id).or_else(|| find_pane(&split.second, pane_id))
        }
    }
}

fn find_pane_mut(node: &mut WorkspaceNode, pane_id: WorkspacePaneId) -> Option<&mut WorkspacePane> {
    match node {
        WorkspaceNode::Pane(pane) if pane.id == pane_id => Some(pane),
        WorkspaceNode::Pane(_) => None,
        WorkspaceNode::Split(split) => find_pane_mut(&mut split.first, pane_id)
            .or_else(|| find_pane_mut(&mut split.second, pane_id)),
    }
}

fn collect_panes<'a>(node: &'a WorkspaceNode, panes: &mut Vec<&'a WorkspacePane>) {
    match node {
        WorkspaceNode::Pane(pane) => panes.push(pane),
        WorkspaceNode::Split(split) => {
            collect_panes(&split.first, panes);
            collect_panes(&split.second, panes);
        }
    }
}

fn remove_tab(node: &mut WorkspaceNode, item: &WorkspaceItemId) -> Option<WorkspaceTab> {
    match node {
        WorkspaceNode::Pane(pane) => {
            let index = pane.tabs.iter().position(|tab| &tab.item == item)?;
            let tab = pane.tabs.remove(index);
            if pane.active_item.as_ref() == Some(item) {
                pane.active_item = pane
                    .tabs
                    .get(index)
                    .or_else(|| pane.tabs.last())
                    .map(|tab| tab.item.clone());
            }
            Some(tab)
        }
        WorkspaceNode::Split(split) => {
            remove_tab(&mut split.first, item).or_else(|| remove_tab(&mut split.second, item))
        }
    }
}

fn insert_tab(pane: &mut WorkspacePane, tab: WorkspaceTab, index: Option<usize>) {
    let index = index.unwrap_or(pane.tabs.len()).min(pane.tabs.len());
    pane.tabs.insert(index, tab);
}

fn replace_pane_with_split(
    node: &mut WorkspaceNode,
    pane_id: WorkspacePaneId,
    split_id: WorkspaceSplitId,
    axis: WorkspaceSplitAxis,
    side: WorkspaceSplitSide,
    ratio: f32,
    new_pane: WorkspaceNode,
) -> bool {
    match node {
        WorkspaceNode::Pane(pane) if pane.id == pane_id => {
            let existing = std::mem::replace(
                node,
                WorkspaceNode::Pane(WorkspacePane::new(
                    0,
                    WorkspacePaneRole::Scratch,
                    Vec::new(),
                )),
            );
            let (first, second, ratio) = match side {
                WorkspaceSplitSide::Before => (new_pane, existing, 1.0 - ratio),
                WorkspaceSplitSide::After => (existing, new_pane, ratio),
            };
            *node = WorkspaceNode::Split(WorkspaceSplit::new(split_id, axis, ratio, first, second));
            true
        }
        WorkspaceNode::Pane(_) => false,
        WorkspaceNode::Split(split) => {
            replace_pane_with_split(
                &mut split.first,
                pane_id,
                split_id,
                axis,
                side,
                ratio,
                new_pane.clone(),
            ) || replace_pane_with_split(
                &mut split.second,
                pane_id,
                split_id,
                axis,
                side,
                ratio,
                new_pane,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        WorkspaceItemId, WorkspaceLayoutCommand, WorkspaceLayoutState, WorkspaceNode,
        WorkspaceResetScope, WorkspaceSplitAxis, WorkspaceSplitSide,
    };

    #[test]
    fn default_layout_contains_primary_bottom_and_inspector_panes() {
        let layout = WorkspaceLayoutState::default();
        let roles = layout
            .panes()
            .into_iter()
            .map(|pane| pane.role)
            .collect::<Vec<_>>();

        assert_eq!(roles.len(), 3);
        assert!(
            layout
                .pane_of_item(&WorkspaceItemId::ConversationThread)
                .is_some()
        );
        assert!(
            layout
                .pane_of_item(&WorkspaceItemId::terminal_session(1))
                .is_some()
        );
        assert!(
            layout
                .pane_of_item(&WorkspaceItemId::inspector_tab(1))
                .is_some()
        );
    }

    #[test]
    fn reorders_items_inside_one_pane() {
        let mut layout = WorkspaceLayoutState::default();
        let changes = WorkspaceItemId::inspector_tab(1);
        let plan = WorkspaceItemId::inspector_tab(3);

        assert!(layout.reorder_item(&plan, &changes));
        let pane_id = layout.pane_of_item(&plan).unwrap();
        let pane = layout.pane(pane_id).unwrap();

        assert_eq!(pane.tabs[0].item, plan);
        assert_eq!(pane.active_item, Some(plan));
    }

    #[test]
    fn moves_items_between_existing_panes() {
        let mut layout = WorkspaceLayoutState::default();
        let terminal = WorkspaceItemId::terminal_session(1);
        let inspector_pane = layout
            .pane_of_item(&WorkspaceItemId::inspector_tab(1))
            .unwrap();

        assert!(layout.move_item_to_pane(&terminal, inspector_pane, Some(1)));

        let pane = layout.pane(inspector_pane).unwrap();
        assert_eq!(pane.tabs[1].item, terminal);
        assert_eq!(pane.active_item, Some(terminal));
    }

    #[test]
    fn can_move_item_to_new_split_pane() {
        let mut layout = WorkspaceLayoutState::default();
        let plan = WorkspaceItemId::inspector_tab(3);
        let main_pane = layout
            .pane_of_item(&WorkspaceItemId::ConversationThread)
            .unwrap();

        let new_pane = layout
            .move_item_to_new_split(
                &plan,
                main_pane,
                WorkspaceSplitAxis::Horizontal,
                WorkspaceSplitSide::After,
                0.5,
            )
            .unwrap();

        assert_eq!(layout.pane_of_item(&plan), Some(new_pane));
        assert!(matches!(layout.root, WorkspaceNode::Split(_)));
    }

    #[test]
    fn reset_command_restores_default_tree() {
        let mut layout = WorkspaceLayoutState::default();
        let terminal = WorkspaceItemId::terminal_session(1);
        let inspector_pane = layout
            .pane_of_item(&WorkspaceItemId::inspector_tab(1))
            .unwrap();
        layout.move_item_to_pane(&terminal, inspector_pane, None);

        assert!(layout.apply_command(WorkspaceLayoutCommand::Reset(WorkspaceResetScope::All)));

        assert_eq!(layout, WorkspaceLayoutState::default());
    }

    #[test]
    fn serializes_for_persistence() {
        let layout = WorkspaceLayoutState::default();
        let json = serde_json::to_string(&layout).unwrap();
        let restored: WorkspaceLayoutState = serde_json::from_str(&json).unwrap();

        assert_eq!(restored, layout);
    }
}
