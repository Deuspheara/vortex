//! Conversation workspace view-model seam.
//!
//! Keeps `AgentWindow` as the mutable owner while projecting the active
//! conversation workspace into a narrower, render-focused view model.

use gpui::{Context, Entity};

use super::AgentWindow;
use crate::features::chat::thread_view::ThreadView;
use crate::features::composer::components::composer_pill::composer_input_needs_stacked_layout;
use crate::features::diff_panel::layout::DiffPanelProps;
use crate::features::inspector::layout::SubagentInspectorVm;
use crate::features::shell::state::{
    ArtifactSelection, ArtifactStore, ContextInspectorRecap, DiffPanelState, InspectorTabs,
    PlanArtifact, TodoEntry,
};
use crate::features::terminal::components::terminal_view::TerminalView;
use crate::features::terminal::layout::TerminalTabVm;
use crate::shared::components::context_usage_ring::{ContextUsageProps, parse_token_usage};
use crate::shared::state::{WorkspaceReadiness, context_for_model};
use agent_protocol::AgentMode;

#[derive(Clone)]
pub(super) struct ConversationWorkspaceVm {
    pub title: String,
    pub readiness: WorkspaceReadiness,
    pub thread_view: Entity<ThreadView>,
    pub active_todos: Vec<TodoEntry>,
    pub todo_strip_expanded: bool,
    pub composer: ComposerWorkspaceVm,
    pub inspector: InspectorWorkspaceVm,
    pub terminal: TerminalWorkspaceVm,
}

#[derive(Clone)]
pub(super) struct ComposerWorkspaceVm {
    pub has_text: bool,
    pub input_expanded: bool,
    pub is_running: bool,
    pub dimmed: bool,
    pub disabled: bool,
    pub recommend_fresh_context: bool,
    pub selected_mode: AgentMode,
    pub selected_branch: String,
    pub branch_items: Vec<String>,
    pub selected_model: String,
    pub model_items: std::sync::Arc<[String]>,
    pub model_search_keys: std::sync::Arc<[std::sync::Arc<str>]>,
    pub pending_image_attachments: Vec<crate::features::composer::state::PendingImageAttachment>,
    pub composer_error: Option<String>,
    pub input_entity: Entity<gpui_component::input::InputState>,
    pub context_usage: ComposerContextUsageVm,
}

#[derive(Clone)]
pub(super) struct ComposerContextUsageVm {
    pub used: f32,
    pub max: f32,
    pub usage_label: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub model: String,
    pub agent_status: Option<crate::features::shell::state::AgentStatus>,
    pub estimated_cost: Option<String>,
    pub index_status: Option<String>,
    pub read_cache_summary: Option<String>,
    pub page_cache_summary: Option<String>,
}

impl ComposerContextUsageVm {
    pub(super) fn to_props(&self) -> ContextUsageProps {
        ContextUsageProps {
            used: self.used,
            max: self.max,
            usage_label: self.usage_label.clone(),
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            cache_read_tokens: self.cache_read_tokens,
            cache_write_tokens: self.cache_write_tokens,
            model: self.model.clone(),
            agent_status: self.agent_status.clone(),
            estimated_cost: self.estimated_cost.clone(),
            index_status: self.index_status.clone(),
            read_cache_summary: self.read_cache_summary.clone(),
            page_cache_summary: self.page_cache_summary.clone(),
        }
    }
}

#[derive(Clone)]
pub(super) struct InspectorWorkspaceVm {
    pub open: bool,
    pub right_dock_width: f32,
    pub tabs: InspectorTabs,
    pub store: ArtifactStore,
    pub selection: ArtifactSelection,
    pub selected_subagent_right: Option<SubagentInspectorVm>,
    pub selected_subagent_bottom: Option<SubagentInspectorVm>,
    pub context_recap: ContextInspectorRecap,
    pub diff_panel_state: DiffPanelState,
    pub plan_artifact: Option<PlanArtifact>,
    pub plan_available: bool,
    pub can_implement_plan: bool,
    pub show_implement_choice: bool,
    pub recommend_fresh_context: bool,
    pub has_pending_approval: bool,
    pub android_session: agent_protocol::AndroidSessionState,
}

impl InspectorWorkspaceVm {
    pub(super) fn diff_panel_props(
        &self,
        entity: Entity<AgentWindow>,
        show_close_button: bool,
    ) -> DiffPanelProps {
        DiffPanelProps {
            state: self.diff_panel_state.clone(),
            plan_artifact: self.plan_artifact.clone(),
            plan_available: self.plan_available,
            can_implement_plan: self.can_implement_plan,
            show_implement_choice: self.show_implement_choice,
            recommend_fresh_context: self.recommend_fresh_context,
            entity,
            approval_actions_enabled: self.has_pending_approval,
            pending_patch_id: self.diff_panel_state.pending_patch_id.clone(),
            patch_apply_enabled: !self.diff_panel_state.applied,
            show_close_button,
        }
    }
}

#[derive(Clone)]
pub(super) struct TerminalWorkspaceVm {
    pub open: bool,
    pub bottom_panel_height: f32,
    pub tabs: Vec<TerminalTabVm>,
    pub terminal_view: Option<Entity<TerminalView>>,
}

impl AgentWindow {
    pub(super) fn build_conversation_workspace(
        &mut self,
        cx: &mut Context<Self>,
    ) -> ConversationWorkspaceVm {
        let title = self
            .selected_conversation_id
            .as_ref()
            .and_then(|cid| self.conversations.iter().find(|c| c.id == *cid))
            .map(|c| c.title.clone())
            .unwrap_or_default();
        let readiness = self.workspace_readiness();
        let has_text = !self.input_state.read(cx).value().trim().is_empty()
            || !self.pending_image_attachments.is_empty();
        let input_expanded =
            composer_input_needs_stacked_layout(&self.input_state.read(cx).value())
                || !self.pending_image_attachments.is_empty()
                || self.composer_error.is_some();
        let is_running = self.selected_conversation_has_active_work();
        let recommend_fresh_context = self.status.input_tokens
            > context_for_model(
                &self.selected_provider,
                &self.selected_model,
                &self.openrouter_models,
            ) / 2;
        let active_todos = self
            .selected_conversation_id
            .as_ref()
            .and_then(|cid| self.conversations.iter().find(|c| c.id == *cid))
            .map(|c| c.active_todos.clone())
            .unwrap_or_default();
        let (_, selected_branch, _) = self.composer_scope();
        let branch_items = self.branch_items_for_selected_project();
        let terminal_tabs = self.terminal_tabs_vm();
        let terminal_view = self.active_terminal_view(cx);
        let (model_items, model_search_keys) = self.model_picker_items_for_selected_provider();
        let selected_conversation = self
            .selected_conversation_id
            .as_ref()
            .and_then(|cid| self.conversations.iter().find(|c| c.id == *cid));
        let plan_artifact = selected_conversation.and_then(|c| c.plan_artifact.clone());
        let plan_available = plan_artifact.is_some();
        let inspector_tabs = self.inspector_tabs.clone();

        ConversationWorkspaceVm {
            title,
            readiness,
            thread_view: self
                .thread_view
                .clone()
                .expect("thread view initialized in prepare_render"),
            active_todos,
            todo_strip_expanded: self.todo_strip_expanded,
            composer: ComposerWorkspaceVm {
                has_text,
                input_expanded,
                is_running,
                dimmed: self.session_run_state.composer_dimmed(),
                disabled: self.session_run_state.composer_disabled(),
                recommend_fresh_context,
                selected_mode: self.safety_mode.clone(),
                selected_branch,
                branch_items,
                selected_model: self.selected_model.clone(),
                model_items,
                model_search_keys,
                pending_image_attachments: self.pending_image_attachments.clone(),
                composer_error: self.composer_error.clone(),
                input_entity: self.input_state.clone(),
                context_usage: self.build_composer_context_usage_vm(),
            },
            inspector: InspectorWorkspaceVm {
                open: self.inspector_mode.is_visible(),
                right_dock_width: self.right_dock_width,
                tabs: inspector_tabs.clone(),
                store: self.artifact_store.clone(),
                selection: self.artifact_selection.clone(),
                selected_subagent_right: self.selected_subagent_for_dock(
                    &inspector_tabs,
                    crate::features::shell::state::DockPlacement::Right,
                ),
                selected_subagent_bottom: self.selected_subagent_for_dock(
                    &inspector_tabs,
                    crate::features::shell::state::DockPlacement::Bottom,
                ),
                context_recap: self.context_inspector_recap.clone(),
                diff_panel_state: self.diff_panel.clone(),
                plan_artifact,
                plan_available,
                can_implement_plan: !is_running,
                show_implement_choice: self.plan_implementation_choice_open,
                recommend_fresh_context,
                has_pending_approval: self.has_pending_approval(),
                android_session: self.android_session.clone(),
            },
            terminal: TerminalWorkspaceVm {
                open: self.terminal_panel_open,
                bottom_panel_height: self.bottom_panel_height,
                tabs: terminal_tabs,
                terminal_view,
            },
        }
    }

    fn build_composer_context_usage_vm(&self) -> ComposerContextUsageVm {
        let (used, max, usage_label) = parse_token_usage(&self.status.token_usage);
        ComposerContextUsageVm {
            used,
            max,
            usage_label,
            input_tokens: self.status.input_tokens,
            output_tokens: self.status.output_tokens,
            cache_read_tokens: self.status.cache_read_tokens,
            cache_write_tokens: self.status.cache_write_tokens,
            model: self.status.model.clone(),
            agent_status: self.status.agent_status.clone(),
            estimated_cost: self.status.estimated_cost.clone(),
            index_status: self
                .context_inspector_recap
                .project_status
                .as_ref()
                .map(|status| {
                    let mut value = format!("Repo index: {}", status.badge_label());
                    if let Some(last) = &status.last_indexed_at {
                        value.push_str(" · ");
                        value.push_str(last);
                    }
                    value
                }),
            read_cache_summary: Some(format!(
                "Read cache: {} entries · {} hits · {} bytes",
                self.context_inspector_recap.read_cache.entries,
                self.context_inspector_recap.read_cache.hits,
                self.context_inspector_recap.read_cache.bytes
            )),
            page_cache_summary: Some(format!(
                "Page cache: {} configured · {} cached",
                if self.context_inspector_recap.page_cache.configured {
                    "API"
                } else {
                    "not"
                },
                self.context_inspector_recap.page_cache.cached_pages
            )),
        }
    }

    fn selected_subagent_for_dock(
        &self,
        tabs: &InspectorTabs,
        dock: crate::features::shell::state::DockPlacement,
    ) -> Option<SubagentInspectorVm> {
        tabs.active_for_dock(dock)
            .and_then(|tab| match &tab.kind {
                crate::features::shell::state::InspectorTabKind::Subagent(item_id) => {
                    self.subagent_transcripts.get(item_id)
                }
                _ => None,
            })
            .and_then(|projection| {
                projection
                    .view
                    .clone()
                    .map(|thread_view| SubagentInspectorVm {
                        item_id: projection.item_id.clone(),
                        task: projection.task.clone(),
                        model: projection.model.clone(),
                        summary: projection.summary.clone(),
                        status_label: projection.status_label(),
                        thread_view,
                    })
            })
    }
}
