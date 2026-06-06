//! AgentWindow — the top-level orchestrator.
//!
//! Owns all mutable state (Entity<AgentWindow>).  Delegates rendering to
//! pure, stateless functions in `components/` and `layouts/`.

#![allow(dead_code)]

mod bridge;
mod conversation_workspace;
mod orchestration;
mod render;
mod types;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use agent_protocol::{AgentCommand, AgentMode, RunId};
use gpui::{Context, Entity, Focusable, Window, prelude::*};
use gpui_component::input::{InputEvent, InputState, Paste};

pub(crate) use types::{ModelPickerCache, SubagentTranscript, TerminalTabGroup};

use crate::agent::{
    AgentBridge, ReducerState, format_session_age, proto_project_id, proto_session_id,
    ui_conversation_id, ui_project_id,
};
use crate::features::composer::state::PendingImageAttachment;

use crate::features::shell::components::tree_row::project_expand_key;
use crate::features::shell::layout::SidebarView;
use crate::features::shell::state::{
    Agent, AgentStatus, ArtifactSelection, ArtifactStore, ChipKind, ContextChip, ContextEntryKind,
    ContextInspectorRecap, ContextTraceSummary, Conversation, ConversationId, DiffPanelState,
    DrawerState, ExpandedItems, InspectorMode, InspectorTabs, InspectorView, PageCacheRecap,
    PendingThreadApproval, PlanExecutionState, PlanProgressCounts, Project, ProjectId,
    ProviderErrorVm, SessionRunState, TaskViewModel, ThreadItem, TodoEntry, TodoState,
    build_task_view,
};

use crate::features::chat::thread_view::ThreadView;
use crate::shared::state::{
    DEFAULT_PROVIDER, ModelOption, ModelPricing, OPENROUTER_PROVIDER, ToolCatalog, TranscriptMode,
    WorkspaceReadiness, WorkspaceReadinessInputs, build_workspace_readiness, context_for_model,
    default_model, format_context_label, openrouter_default_model,
};
use crate::tokens::Tokens;
use crate::tokens::theme::{DEFAULT_DARK_THEME, apply_theme, sync_palette};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppScreen {
    Chat,
    Search,
    Extensions,
    Automations,
    Settings,
}

pub struct AgentWindow {
    pub screen: AppScreen,
    pub projects: Vec<Project>,
    pub conversations: Vec<Conversation>,
    pub agents: Vec<Agent>,
    pub selected_project_id: Option<ProjectId>,
    pub selected_conversation_id: Option<ConversationId>,
    pub sidebar_collapsed: bool,
    pub terminal_panel_open: bool,
    pub right_dock_width: f32,
    pub bottom_panel_height: f32,
    pub expanded_items: ExpandedItems,
    pub drawer: DrawerState,
    pub diff_panel: DiffPanelState,
    pub transcript_mode: TranscriptMode,
    pub inspector_mode: InspectorMode,
    pub inspector_tabs: InspectorTabs,
    pub inspector_view: InspectorView,
    pub artifact_selection: ArtifactSelection,
    pub artifact_store: ArtifactStore,
    pub session_run_state: SessionRunState,
    pub provider_blocked: Option<ProviderErrorVm>,
    pub task_view: TaskViewModel,
    pub status: StatusBarState,
    pub input_state: Entity<InputState>,
    pub search_input: Entity<InputState>,
    pub selected_provider: String,
    pub selected_model: String,
    pub selected_subagent_model: Option<String>,
    pub thread_view: Option<Entity<ThreadView>>,
    pub(crate) subagent_transcripts: HashMap<String, SubagentTranscript>,
    pub(crate) subagent_by_parent_call: HashMap<String, String>,
    pub sidebar_view: Option<Entity<SidebarView>>,
    pub simulations_running: HashSet<ConversationId>,
    pub agent_bridge: Arc<AgentBridge>,
    pub active_run_id: Option<RunId>,
    pub pending_approval_id: Option<String>,
    pub pending_thread_approval: Option<PendingThreadApproval>,
    pub running_conversations: HashSet<ConversationId>,
    pub reducer_state: ReducerState,
    /// O(1) lookup from thread item id → index within `Conversation::thread_items`.
    pub(crate) thread_item_indices: HashMap<ConversationId, HashMap<String, usize>>,
    pub safety_mode: AgentMode,
    pub openrouter_models: Vec<ModelOption>,
    pub model_pricing: HashMap<String, ModelPricing>,
    pub run_cost_usd: f64,
    pub tool_catalog: ToolCatalog,
    pub android_session: agent_protocol::AndroidSessionState,
    pub request_thread_scroll_to_bottom: bool,
    pub context_inspector_recap: ContextInspectorRecap,
    pub collapsed_sessions: HashSet<String>,
    pub plan_implementation_choice_open: bool,
    pub todo_strip_expanded: bool,
    pub(crate) diff_preview_pending: Option<String>,
    pub(crate) diff_preview_parse_scheduled: bool,
    /// Memoized git branch list keyed by project root. Refreshes happen from
    /// state transitions, not during render.
    pub(crate) branch_items_cache: Option<(String, Vec<String>)>,
    pub(crate) openrouter_models_revision: u64,
    pub(crate) model_picker_cache: Option<ModelPickerCache>,
    pub(crate) terminal_tab_groups: HashMap<ProjectId, TerminalTabGroup>,
    pub(crate) command_run_ledger: crate::features::composer::state::CommandRunLedger,
    pub(crate) pending_image_attachments: Vec<PendingImageAttachment>,
    pub(crate) composer_error: Option<String>,
}

fn implementation_prompt(plan_markdown: &str, fresh_context: bool) -> String {
    if fresh_context {
        format!(
            "You are implementing an approved plan in a fresh conversation context.\n\nImplement the plan below. Keep changes scoped to it, use todo_write as the live execution checklist, and ask before changing scope.\n\n[APPROVED_PLAN]\n{plan_markdown}"
        )
    } else {
        "Implement the approved plan artifact already present in this conversation. Do not re-plan. Keep changes scoped to that plan, use todo_write as the live execution checklist, and ask before changing scope.".to_string()
    }
}

fn implementation_user_message(fresh_context: bool) -> String {
    if fresh_context {
        "Implement the approved plan in a fresh context.".to_string()
    } else {
        "Implement the approved plan in this conversation.".to_string()
    }
}

fn todos_from_plan(markdown: &str) -> Vec<crate::features::shell::state::TodoEntry> {
    let mut todos = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        let content = if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
            rest
        } else if let Some(rest) = trimmed.strip_prefix("- ") {
            rest
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            rest
        } else {
            continue;
        };
        let content = content.trim();
        if content.is_empty()
            || matches!(
                content.to_ascii_lowercase().as_str(),
                "summary" | "test plan" | "assumptions" | "key changes"
            )
        {
            continue;
        }
        todos.push(crate::features::shell::state::TodoEntry {
            id: format!("plan-todo-{}", todos.len() + 1),
            content: content.chars().take(140).collect(),
            state: crate::features::shell::state::TodoState::Pending,
        });
        if todos.len() >= 8 {
            break;
        }
    }
    todos
}

fn plan_progress_counts(items: &[TodoEntry]) -> PlanProgressCounts {
    let mut counts = PlanProgressCounts::default();
    for item in items {
        match item.state {
            TodoState::Pending => counts.pending += 1,
            TodoState::InProgress => counts.in_progress += 1,
            TodoState::Completed => counts.completed += 1,
            TodoState::Cancelled => counts.cancelled += 1,
        }
    }
    counts
}

fn first_incomplete_todo(items: &[TodoEntry]) -> Option<&str> {
    items
        .iter()
        .find(|item| !matches!(item.state, TodoState::Completed | TodoState::Cancelled))
        .map(|item| item.content.as_str())
}

fn plan_status_summary(markdown: &str, todos: &[TodoEntry]) -> String {
    if let Some(todo) = first_incomplete_todo(todos) {
        return todo.to_string();
    }
    markdown
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "Approved plan ready".to_string())
}

fn ui_timestamp_now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::todos_from_plan;
    use crate::features::shell::state::TodoState;

    #[test]
    fn derives_pending_todos_from_plan_markdown() {
        let todos = todos_from_plan(
            "# Plan\n## Summary\n## Key Changes\n- [ ] Add plan artifact event\n- Wire UI\n",
        );
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].content, "Add plan artifact event");
        assert!(matches!(todos[0].state, TodoState::Pending));
    }
}

#[derive(Clone)]
pub struct StatusBarState {
    pub model: String,
    pub token_usage: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub agent_status: Option<AgentStatus>,
    pub estimated_cost: Option<String>,
}

impl Default for StatusBarState {
    fn default() -> Self {
        Self {
            model: "Claude Sonnet 4.5".into(),
            token_usage: "0 / 200K".into(),
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            agent_status: Some(AgentStatus::Idle),
            estimated_cost: None,
        }
    }
}

impl AgentWindow {
    pub(crate) fn live_edit_change_counts(
        &self,
        tool_name: &str,
        running: bool,
    ) -> Option<(usize, usize)> {
        if !running
            || !matches!(
                tool_name,
                "write_file" | "edit_file" | "delete_file" | "apply_patch" | "propose_patch"
            )
        {
            return None;
        }

        let (added, removed) = self
            .diff_panel
            .files
            .iter()
            .fold((0usize, 0usize), |(added, removed), file| {
                (added + file.added, removed + file.removed)
            });
        if added == 0 && removed == 0 {
            None
        } else {
            Some((added, removed))
        }
    }

    pub fn new(
        cx: &mut Context<Self>,
        window: &mut Window,
        agent_bridge: Arc<AgentBridge>,
    ) -> Self {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Ask anything, @ to mention, / for actions")
                .auto_grow(
                    Tokens::COMPOSER_INPUT_MIN_ROWS,
                    Tokens::COMPOSER_INPUT_MAX_ROWS,
                )
        });

        cx.subscribe_in(&input_state, window, move |view, _, event, window, cx| {
            let InputEvent::PressEnter { secondary: false } = event else {
                return;
            };
            view.send_message(window, cx);
        })
        .detach();

        let paste_entity = cx.weak_entity();
        cx.intercept_keystrokes(move |event, window, app| {
            let is_paste_action = event
                .action
                .as_ref()
                .is_some_and(|action| action.as_any().is::<Paste>());
            let is_paste_key = event.keystroke.key.eq_ignore_ascii_case("v")
                && event.keystroke.modifiers.secondary()
                && !event.keystroke.modifiers.alt
                && !event.keystroke.modifiers.shift
                && !event.keystroke.modifiers.function;
            if !is_paste_action && !is_paste_key {
                return;
            }

            let Ok(handled) = paste_entity.update(app, |view, cx| {
                let composer_focused = view
                    .input_state
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window);
                if !composer_focused {
                    return false;
                }
                view.add_clipboard_image_attachment(cx)
            }) else {
                return;
            };

            if handled {
                app.stop_propagation();
            }
        })
        .detach();

        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search conversations…"));

        apply_theme(DEFAULT_DARK_THEME, Some(window), cx);
        sync_palette(cx);

        let bridge = agent_bridge.clone();
        let entity_weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            loop {
                match bridge.event_rx.recv_async().await {
                    Ok(first) => {
                        let mut events = vec![first];
                        while let Ok(next) = bridge.event_rx.try_recv() {
                            events.push(next);
                        }
                        let weak = entity_weak.clone();
                        let _ = weak.update(cx, |view, cx| {
                            view.apply_agent_events(events, cx);
                        });
                    }
                    Err(_) => break,
                }
            }
        })
        .detach();

        let (selected_provider, selected_model) = if agent_bridge.uses_mock {
            (DEFAULT_PROVIDER.to_string(), default_model())
        } else {
            (OPENROUTER_PROVIDER.to_string(), openrouter_default_model())
        };

        let mut window_state = Self {
            screen: AppScreen::Chat,
            projects: Vec::new(),
            conversations: Vec::new(),
            agents: Vec::new(),
            selected_project_id: None,
            selected_conversation_id: None,
            sidebar_collapsed: false,
            terminal_panel_open: false,
            right_dock_width: Tokens::INSPECTOR_WIDTH_REVIEW,
            bottom_panel_height: Tokens::BOTTOM_PANEL_HEIGHT,
            expanded_items: ExpandedItems::new(),
            drawer: DrawerState::default(),
            diff_panel: DiffPanelState::default(),
            transcript_mode: TranscriptMode::default(),
            inspector_mode: InspectorMode::default(),
            inspector_tabs: InspectorTabs::default(),
            inspector_view: InspectorView::default(),
            artifact_selection: ArtifactSelection::default(),
            artifact_store: ArtifactStore::default(),
            session_run_state: SessionRunState::default(),
            provider_blocked: None,
            task_view: TaskViewModel::default(),
            status: StatusBarState {
                model: default_model(),
                ..Default::default()
            },
            input_state,
            search_input,
            selected_provider,
            selected_model: selected_model.clone(),
            selected_subagent_model: None,
            thread_view: None,
            subagent_transcripts: HashMap::new(),
            subagent_by_parent_call: HashMap::new(),
            sidebar_view: None,
            simulations_running: HashSet::new(),
            agent_bridge: agent_bridge.clone(),
            active_run_id: None,
            pending_approval_id: None,
            pending_thread_approval: None,
            running_conversations: HashSet::new(),
            reducer_state: ReducerState::default(),
            thread_item_indices: HashMap::new(),
            safety_mode: AgentMode::ApplyWithApproval,
            openrouter_models: Vec::new(),
            model_pricing: HashMap::new(),
            run_cost_usd: 0.0,
            tool_catalog: ToolCatalog::from_descriptors(agent_bridge.runtime.tool_catalog()),
            android_session: agent_protocol::AndroidSessionState::default(),
            request_thread_scroll_to_bottom: false,
            context_inspector_recap: ContextInspectorRecap::default(),
            collapsed_sessions: HashSet::new(),
            plan_implementation_choice_open: false,
            todo_strip_expanded: false,
            diff_preview_pending: None,
            diff_preview_parse_scheduled: false,
            branch_items_cache: None,
            openrouter_models_revision: 0,
            model_picker_cache: None,
            terminal_tab_groups: HashMap::new(),
            command_run_ledger: crate::features::composer::state::CommandRunLedger::default(),
            pending_image_attachments: Vec::new(),
            composer_error: None,
        };

        window_state.status.model = selected_model;
        window_state.refresh_token_usage_display();
        window_state.hydrate_from_store(cx);
        window_state.start_index_status_poll(cx);

        if let Some(models_rx) = agent_bridge.openrouter_models_rx.clone() {
            let weak = cx.weak_entity();
            cx.spawn(async move |_, cx| match models_rx.recv_async().await {
                Ok(Ok(models)) => {
                    let _ = cx.update(|app| {
                        app.defer(move |app| {
                            let _ = weak.update(app, |view, cx| {
                                view.apply_openrouter_models(models, cx);
                            });
                        });
                    });
                }
                Ok(Err(err)) => {
                    tracing::warn!("failed to load OpenRouter models: {err}");
                }
                Err(_) => {}
            })
            .detach();
        }

        window_state
    }

    fn hydrate_from_store(&mut self, cx: &mut Context<Self>) {
        let stored_projects = self.agent_bridge.list_projects().unwrap_or_default();
        let mut projects = Vec::new();
        let mut conversations = Vec::new();
        let mut latest_session: Option<(ConversationId, ProjectId)> = None;
        let mut latest_updated = chrono::DateTime::<chrono::Utc>::MIN_UTC;

        for stored in stored_projects {
            let project_id = ui_project_id(&stored.id);
            let git_branch = self.agent_bridge.git_branch_for_path(&stored.root_path);
            let mut project = Project::new(
                project_id.0.clone(),
                &stored.name,
                &stored.root_path,
                &git_branch,
                stored.trusted,
            );

            let sessions = self
                .agent_bridge
                .list_sessions(&stored.id)
                .unwrap_or_default();
            for session in sessions {
                let conv_id = ui_conversation_id(&session.id);
                project.conversations.push(conv_id.clone());
                let conv = Conversation::new(
                    conv_id.0.clone(),
                    project_id.clone(),
                    &session.title,
                    &format_session_age(&session.updated_at),
                );
                if session.updated_at > latest_updated {
                    latest_updated = session.updated_at;
                    latest_session = Some((conv_id.clone(), project_id.clone()));
                }
                conversations.push(conv);
            }

            projects.push(project);
        }

        self.projects = projects;
        self.conversations = conversations;
        self.refresh_indexing_state(cx);

        if self.conversations.is_empty() {
            self.bootstrap_workspace_session(cx);
            return;
        }

        if let Some((conv_id, project_id)) = latest_session {
            self.selected_project_id = Some(project_id.clone());
            self.selected_conversation_id = Some(conv_id.clone());
            // Only expand the active project — not every folder on disk.
            self.expanded_items.clear();
            self.expanded_items.insert(project_expand_key(&project_id));
            self.sync_context_chips_for_conversation(&conv_id);
            self.replay_session_events(&conv_id.0, cx);
        } else if let Some(conv_id) = self
            .projects
            .first()
            .and_then(|p| p.conversations.first().cloned())
        {
            let project_id = self
                .projects
                .first()
                .map(|p| p.id.clone())
                .expect("project exists");
            self.selected_project_id = Some(project_id.clone());
            self.selected_conversation_id = Some(conv_id.clone());
            self.expanded_items.clear();
            self.expanded_items.insert(project_expand_key(&project_id));
            self.sync_context_chips_for_conversation(&conv_id);
            self.replay_session_events(&conv_id.0, cx);
        }

        self.refresh_indexing_state(cx);
        self.sync_sidebar_view(cx);
        cx.notify();
    }

    fn bootstrap_workspace_session(&mut self, cx: &mut Context<Self>) {
        let Ok((stored, session)) = self.agent_bridge.ensure_workspace_session() else {
            tracing::error!("failed to bootstrap workspace session");
            return;
        };

        let project_id = ui_project_id(&stored.id);
        let git_branch = self.agent_bridge.git_branch_for_path(&stored.root_path);
        let conv_id = ui_conversation_id(&session.id);

        if !self.projects.iter().any(|p| p.id == project_id) {
            let mut project = Project::new(
                project_id.0.clone(),
                &stored.name,
                &stored.root_path,
                &git_branch,
                stored.trusted,
            );
            project.conversations.push(conv_id.clone());
            self.expanded_items.insert(project_expand_key(&project_id));
            self.projects.push(project);
        } else if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
            project.name = stored.name.clone();
            project.root_path = stored.root_path.clone();
            project.git_branch = git_branch;
            project.trusted = stored.trusted;
            if !project.conversations.contains(&conv_id) {
                project.conversations.push(conv_id.clone());
            }
        }

        if !self.conversations.iter().any(|c| c.id == conv_id) {
            let conv =
                Conversation::new(conv_id.0.clone(), project_id.clone(), &session.title, "now");
            self.conversations.push(conv);
        }

        self.selected_project_id = Some(project_id.clone());
        self.selected_conversation_id = Some(conv_id.clone());
        self.sync_context_chips_for_conversation(&conv_id);
        self.replay_session_events(&conv_id.0, cx);
        self.refresh_indexing_state(cx);
        self.sync_sidebar_view(cx);
    }

    fn ensure_send_target(&mut self, cx: &mut Context<Self>) -> Option<ConversationId> {
        if self.selected_conversation_id.is_some() && self.selected_project_id.is_some() {
            return self.selected_conversation_id.clone();
        }

        if self.conversations.is_empty() {
            self.bootstrap_workspace_session(cx);
        }

        self.selected_conversation_id.clone()
    }

    pub(crate) fn selected_project(&self) -> Option<&Project> {
        if let Some(cid) = &self.selected_conversation_id {
            if let Some(conv) = self.conversations.iter().find(|c| c.id == *cid) {
                if let Some(project) = self.projects.iter().find(|p| p.id == conv.project_id) {
                    return Some(project);
                }
            }
        }

        self.selected_project_id
            .as_ref()
            .and_then(|pid| self.projects.iter().find(|p| p.id == *pid))
    }

    pub(crate) fn workspace_readiness(&self) -> WorkspaceReadiness {
        let selected_project = self.selected_project();
        let context_recap = &self.context_inspector_recap;
        build_workspace_readiness(WorkspaceReadinessInputs {
            provider_connected: !self.agent_bridge.uses_mock,
            uses_mock_provider: self.agent_bridge.uses_mock,
            has_project: selected_project.is_some(),
            project_trusted: selected_project.is_some_and(|project| project.trusted),
            index_phase: selected_project.map(|project| project.index_status.phase),
            context_trace_groups: context_recap.context_trace.len(),
            read_cache_entries: context_recap.read_cache.entries,
            page_cache_configured: context_recap.page_cache.configured,
        })
    }

    fn sync_context_chips_for_conversation(&mut self, conv_id: &ConversationId) {
        let Some(conv) = self.conversations.iter_mut().find(|c| c.id == *conv_id) else {
            return;
        };
        let Some(project) = self.projects.iter().find(|p| p.id == conv.project_id) else {
            return;
        };
        conv.context_chips = vec![
            ContextChip {
                label: project.name.clone(),
                kind: ChipKind::Repo,
            },
            ContextChip {
                label: project.git_branch.clone(),
                kind: ChipKind::Branch,
            },
        ];
    }

    fn start_index_status_poll(&self, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        cx.spawn(async move |_weak, cx| {
            loop {
                gpui::Timer::after(std::time::Duration::from_secs(2)).await;
                let _ = weak.update(cx, |view, cx| {
                    view.refresh_indexing_state(cx);
                });
            }
        })
        .detach();
    }

    pub(crate) fn refresh_indexing_state(&mut self, cx: &mut Context<Self>) {
        let bridge = self.agent_bridge.clone();
        let mut sidebar_dirty = false;
        for project in &mut self.projects {
            bridge.ensure_project_indexing(&proto_project_id(&project.id), &project.root_path);
            let next = bridge.project_index_status(&project.id);
            if project.index_status != next {
                project.index_status = next;
                sidebar_dirty = true;
            }
        }

        let next_recap = self.build_context_inspector_recap();
        let recap_dirty = self.context_inspector_recap != next_recap;
        if recap_dirty {
            self.context_inspector_recap = next_recap;
        }

        if sidebar_dirty {
            self.sync_sidebar_view(cx);
        }
        if sidebar_dirty || recap_dirty {
            cx.notify();
        }
    }

    fn build_context_inspector_recap(&self) -> ContextInspectorRecap {
        let project_status = self.selected_project_id.as_ref().and_then(|project_id| {
            self.projects
                .iter()
                .find(|project| &project.id == project_id)
                .map(|project| project.index_status.clone())
        });

        let context_trace = self
            .selected_conversation_id
            .as_ref()
            .map_or_else(Vec::new, |conv_id| self.context_trace_summary(conv_id));

        let read_cache = self
            .selected_conversation_id
            .as_ref()
            .map(|conv_id| {
                self.agent_bridge
                    .read_cache_recap(&proto_session_id(conv_id))
            })
            .unwrap_or_default();

        ContextInspectorRecap {
            project_status,
            context_trace,
            read_cache,
            page_cache: PageCacheRecap {
                configured: self.agent_bridge.page_index_configured(),
                cached_pages: 0,
            },
        }
    }

    fn context_trace_summary(&self, conv_id: &ConversationId) -> Vec<ContextTraceSummary> {
        let Some(conversation) = self.conversations.iter().find(|conv| &conv.id == conv_id) else {
            return Vec::new();
        };

        let mut counts = [0usize; 6];
        for item in &conversation.thread_items {
            let ThreadItem::ContextTrace { entries, .. } = item else {
                continue;
            };
            for entry in entries {
                let slot = match entry.kind {
                    ContextEntryKind::RepoMap => 0,
                    ContextEntryKind::FileSlice => 1,
                    ContextEntryKind::Symbol => 2,
                    ContextEntryKind::Search => 3,
                    ContextEntryKind::Command => 4,
                    ContextEntryKind::Rule => 5,
                };
                counts[slot] += 1;
            }
        }

        [
            ContextEntryKind::RepoMap,
            ContextEntryKind::FileSlice,
            ContextEntryKind::Symbol,
            ContextEntryKind::Search,
            ContextEntryKind::Command,
            ContextEntryKind::Rule,
        ]
        .into_iter()
        .zip(counts)
        .filter_map(|(kind, count)| (count > 0).then_some(ContextTraceSummary { kind, count }))
        .collect()
    }

    fn context_chips_for_project(project: &Project) -> Vec<ContextChip> {
        vec![
            ContextChip {
                label: project.name.clone(),
                kind: ChipKind::Repo,
            },
            ContextChip {
                label: project.git_branch.clone(),
                kind: ChipKind::Branch,
            },
        ]
    }

    fn ensure_thread_view(&mut self, cx: &mut Context<Self>) {
        if self.thread_view.is_some() {
            return;
        }

        let agent = cx.entity();
        let conversation_id = self.selected_conversation_id.clone();
        let items = conversation_id
            .as_ref()
            .map(|cid| self.thread_items_for(cid))
            .unwrap_or_default();

        let thread =
            cx.new(|cx| ThreadView::new(agent.clone(), conversation_id.clone(), items, cx));
        self.thread_view = Some(thread);
        self.sync_thread_approval_state(cx);
    }

    fn ensure_sidebar_view(&mut self, cx: &mut Context<Self>) {
        if self.sidebar_view.is_some() {
            return;
        }

        let agent = cx.entity();
        let search_input = self.search_input.clone();
        let sidebar = cx.new(|cx| SidebarView::new(agent, search_input, cx));
        self.sidebar_view = Some(sidebar);
        self.sync_sidebar_view(cx);
    }

    fn prepare_active_subagent_transcript(&mut self, cx: &mut Context<Self>) {
        let Some(item_id) = self
            .inspector_tabs
            .active()
            .and_then(|tab| match &tab.kind {
                crate::features::shell::state::InspectorTabKind::Subagent(item_id) => {
                    Some(item_id.clone())
                }
                _ => None,
            })
        else {
            return;
        };
        self.ensure_subagent_transcript_view(&item_id, cx);
    }

    pub fn thread_items_for(&self, conversation_id: &ConversationId) -> Vec<ThreadItem> {
        self.conversations
            .iter()
            .find(|c| c.id == *conversation_id)
            .map(|c| c.thread_items.clone())
            .unwrap_or_default()
    }

    pub fn set_agent_status(&mut self, status: AgentStatus, cx: &mut Context<Self>) {
        self.status.agent_status = Some(status);
        cx.notify();
    }

    pub fn has_pending_approval(&self) -> bool {
        self.pending_approval_id.is_some()
    }

    pub(crate) fn composer_overlay_bar_height(&self) -> f32 {
        if self.pending_thread_approval.is_some() {
            return Tokens::COMPOSER_APPROVAL_BAR_HEIGHT;
        }
        Tokens::composer_pending_action_bar_height(self.pending_action_bar_row_count())
    }

    pub fn tool_row_label(&self, tool_name: &str, command: Option<&str>, running: bool) -> String {
        self.agent_bridge
            .runtime
            .tool_row_label(tool_name, command, running)
    }

    pub fn active_context_chips(&self) -> Vec<ContextChip> {
        self.selected_conversation_id
            .as_ref()
            .and_then(|cid| self.conversations.iter().find(|c| c.id == *cid))
            .map(|c| c.context_chips.clone())
            .unwrap_or_default()
    }

    fn refresh_session_run_state(&mut self) {
        let blocked = self.provider_blocked.is_some();
        self.session_run_state = SessionRunState::from_agent_status(
            self.status
                .agent_status
                .as_ref()
                .unwrap_or(&AgentStatus::Idle),
            blocked,
        );
    }

    /// Whether the thread should treat the run as actively streaming (debounced tail updates).
    /// Paused for patch or tool approval is not streaming — avoids stale tail row heights.
    pub(crate) fn thread_run_active(&self, conversation_id: &ConversationId) -> bool {
        self.running_conversations.contains(conversation_id)
            && self.pending_approval_id.is_none()
            && self.diff_panel.pending_patch_id.is_none()
    }

    fn sync_inspector_open(&mut self) {
        self.diff_panel.open = self.inspector_mode.is_visible();
    }

    pub(crate) fn refresh_task_projection(&mut self, conv_id: &ConversationId) {
        if let Some(conv) = self.conversations.iter().find(|c| c.id == *conv_id) {
            self.task_view = build_task_view(&conv.title, &conv.thread_items, self.transcript_mode);
        }
        self.refresh_session_run_state();
    }

    pub(crate) fn schedule_diff_preview_parse(
        &mut self,
        unified_diff: String,
        cx: &mut Context<Self>,
    ) {
        self.diff_preview_pending = Some(unified_diff);
        if self.diff_preview_parse_scheduled {
            return;
        }
        self.diff_preview_parse_scheduled = true;
        let weak = cx.weak_entity();
        cx.spawn(async move |_weak, cx| {
            gpui::Timer::after(std::time::Duration::from_millis(150)).await;
            weak.update(cx, |view, cx| {
                view.diff_preview_parse_scheduled = false;
                if let Some(diff) = view.diff_preview_pending.take() {
                    let files = crate::features::diff_panel::layout::parse_unified_diff(&diff);
                    view.diff_panel.files = files.clone();
                    view.artifact_store
                        .update_diff_files("patch-preview", files);
                    view.diff_panel.applied = false;
                    if let Some(thread) = view.thread_view.clone() {
                        thread.update(cx, |_thread, cx| cx.notify());
                    }
                    // Silent update — do not auto-open inspector on streaming previews.
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn toggle_collapsed_session(&mut self, session_id: &str, cx: &mut Context<Self>) {
        if self.collapsed_sessions.contains(session_id) {
            self.collapsed_sessions.remove(session_id);
        } else {
            self.collapsed_sessions.insert(session_id.to_string());
        }
        if let Some(conv_id) = self.selected_conversation_id.clone() {
            self.sync_thread_view(conv_id, cx);
        }
    }

    pub fn submit_choice(&mut self, choice_id: &str, option_id: &str, cx: &mut Context<Self>) {
        if let Some(conv_id) = self.selected_conversation_id.clone() {
            self.update_thread_item(
                conv_id.clone(),
                choice_id,
                |item| {
                    if let ThreadItem::ChoiceRequest {
                        selected, resolved, ..
                    } = item
                    {
                        *selected = Some(option_id.to_string());
                        *resolved = true;
                    }
                },
                cx,
            );
            let _ = self.agent_bridge.send(AgentCommand::SubmitChoice {
                choice_id: choice_id.to_string(),
                option_id: option_id.to_string(),
            });
        }
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        self.sync_sidebar_view(cx);
        cx.notify();
    }

    pub fn implement_plan_here(&mut self, cx: &mut Context<Self>) {
        if self.selected_conversation_has_active_work() {
            return;
        }
        let Some(conv_id) = self.selected_conversation_id.clone() else {
            return;
        };
        let Some(plan) = self
            .conversations
            .iter()
            .find(|c| c.id == conv_id)
            .and_then(|c| c.plan_artifact.clone())
        else {
            return;
        };
        self.seed_todos_from_plan(&conv_id, &plan.markdown, cx);
        self.start_plan_execution(&conv_id, None);
        self.sync_plan_status_for_conversation(&conv_id, cx);
        self.plan_implementation_choice_open = false;
        self.safety_mode = AgentMode::ApplyWithApproval;
        let prompt = implementation_prompt(&plan.markdown, false);
        let display = implementation_user_message(false);
        self.push_user_and_start(conv_id, display, prompt, cx);
    }

    pub fn implement_plan_fresh(&mut self, cx: &mut Context<Self>) {
        if self.selected_conversation_has_active_work() {
            return;
        }
        let Some(source_conv_id) = self.selected_conversation_id.clone() else {
            return;
        };
        let Some((project_id, plan)) = self
            .conversations
            .iter()
            .find(|c| c.id == source_conv_id)
            .and_then(|c| c.plan_artifact.clone().map(|p| (c.project_id.clone(), p)))
        else {
            return;
        };
        self.start_plan_execution(&source_conv_id, Some(source_conv_id.clone()));
        self.sync_plan_status_for_conversation(&source_conv_id, cx);
        self.plan_implementation_choice_open = false;
        self.create_conversation_in_project(project_id, cx);
        let Some(new_conv_id) = self.selected_conversation_id.clone() else {
            return;
        };
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == new_conv_id) {
            conv.plan_artifact = Some(plan.clone());
        }
        self.seed_todos_from_plan(&new_conv_id, &plan.markdown, cx);
        self.start_plan_execution(&new_conv_id, Some(source_conv_id));
        self.sync_plan_status_for_conversation(&new_conv_id, cx);
        self.safety_mode = AgentMode::ApplyWithApproval;
        let prompt = implementation_prompt(&plan.markdown, true);
        let display = implementation_user_message(true);
        self.push_user_and_start(new_conv_id, display, prompt, cx);
    }

    pub fn show_plan_implementation_choice(&mut self, cx: &mut Context<Self>) {
        if self.selected_conversation_has_active_work() {
            return;
        }
        self.plan_implementation_choice_open = true;
        cx.notify();
    }

    fn push_user_and_start(
        &mut self,
        conv_id: ConversationId,
        display_text: String,
        run_prompt: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == conv_id) {
            let id = format!("user-msg-{}", conv.thread_items.len());
            conv.thread_items.push(ThreadItem::UserMessage {
                id,
                text: display_text,
                attachments: Vec::new(),
                expanded: false,
            });
        }
        self.sync_thread_view(conv_id.clone(), cx);
        if let Err(err) = self.start_agent_run(&run_prompt, Vec::new(), cx) {
            tracing::error!("failed to start plan implementation: {err}");
            self.running_conversations.remove(&conv_id);
            self.status.agent_status = Some(AgentStatus::Idle);
        }
    }

    /// Drop stale run markers when the runtime never confirmed or already finished.
    fn reconcile_stale_run_state(&mut self) {
        let Some(cid) = self.selected_conversation_id.clone() else {
            return;
        };
        if !self.running_conversations.contains(&cid) {
            return;
        }
        if self.active_run_id.is_some() {
            return;
        }
        let stale = matches!(
            self.status.agent_status,
            Some(AgentStatus::Idle)
                | Some(AgentStatus::Completed)
                | Some(AgentStatus::Failed)
                | None
        );
        if stale {
            self.running_conversations.remove(&cid);
        }
    }

    fn push_thinking_indicator(&mut self, conv_id: ConversationId, cx: &mut Context<Self>) {
        let already_thinking = self
            .conversations
            .iter()
            .find(|c| c.id == conv_id)
            .is_some_and(|conv| {
                conv.thread_items.iter().any(|item| {
                    matches!(
                        item,
                        ThreadItem::ReasoningStep {
                            status: AgentStatus::Thinking,
                            depth: 0,
                            ..
                        }
                    )
                })
            });
        if already_thinking {
            return;
        }
        let id = format!("reason-{}", uuid::Uuid::new_v4());
        self.push_thread_item(
            conv_id,
            ThreadItem::ReasoningStep {
                id,
                title: "Thinking".into(),
                summary: String::new(),
                expanded: false,
                status: AgentStatus::Thinking,
                depth: 0,
                parent_call_id: None,
            },
            cx,
        );
    }

    fn selected_conversation_has_active_work(&self) -> bool {
        self.selected_conversation_id
            .as_ref()
            .is_some_and(|cid| self.running_conversations.contains(cid))
            || matches!(
                self.status.agent_status,
                Some(AgentStatus::Thinking) | Some(AgentStatus::RunningTool)
            )
            || self.pending_approval_id.is_some()
            || self.pending_thread_approval.is_some()
            || self.diff_panel.pending_patch_id.is_some()
            || self.selected_conversation_id.as_ref().is_some_and(|cid| {
                self.conversations
                    .iter()
                    .find(|c| c.id == *cid)
                    .is_some_and(|conv| {
                        conv.thread_items.iter().any(|item| {
                            matches!(
                                item,
                                ThreadItem::ChoiceRequest {
                                    resolved: false,
                                    ..
                                }
                            )
                        })
                    })
            })
    }

    fn start_plan_execution(
        &mut self,
        conv_id: &ConversationId,
        source_conversation_id: Option<ConversationId>,
    ) {
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == *conv_id) {
            if let Some(plan) = &mut conv.plan_artifact {
                plan.execution_state = PlanExecutionState::Implementing;
                plan.source_conversation_id = source_conversation_id;
                plan.started_at = Some(ui_timestamp_now());
                plan.completed_at = None;
            }
        }
    }

    fn mark_plan_stale(&mut self, conv_id: &ConversationId) {
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == *conv_id) {
            if let Some(plan) = &mut conv.plan_artifact {
                if !matches!(plan.execution_state, PlanExecutionState::Completed) {
                    plan.execution_state = PlanExecutionState::Stale;
                    plan.completed_at = None;
                }
            }
        }
    }

    pub(crate) fn sync_plan_status_for_conversation(
        &mut self,
        conv_id: &ConversationId,
        cx: &mut Context<Self>,
    ) {
        let mut index_dirty = false;
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == *conv_id) {
            if let Some(plan) = &mut conv.plan_artifact {
                let counts = plan_progress_counts(&conv.active_todos);
                if matches!(plan.execution_state, PlanExecutionState::Implementing)
                    && counts.is_done()
                {
                    plan.execution_state = PlanExecutionState::Completed;
                    if plan.completed_at.is_none() {
                        plan.completed_at = Some(ui_timestamp_now());
                    }
                }

                let item = ThreadItem::PlanStatus {
                    id: "plan-status".to_string(),
                    state: plan.execution_state,
                    summary: plan_status_summary(&plan.markdown, &conv.active_todos),
                    counts,
                    source_conversation_id: plan.source_conversation_id.clone(),
                };
                if let Some(existing) = conv
                    .thread_items
                    .iter_mut()
                    .find(|item| matches!(item, ThreadItem::PlanStatus { .. }))
                {
                    *existing = item;
                } else {
                    conv.thread_items.push(item);
                }
                index_dirty = true;
            } else {
                let before = conv.thread_items.len();
                conv.thread_items
                    .retain(|item| !matches!(item, ThreadItem::PlanStatus { .. }));
                index_dirty = conv.thread_items.len() != before;
            }
        }

        if index_dirty {
            self.rebuild_thread_item_index(conv_id);
        }
        if self.selected_conversation_id.as_ref() == Some(conv_id) {
            self.sync_thread_view(conv_id.clone(), cx);
        }
    }

    fn seed_todos_from_plan(
        &mut self,
        conv_id: &ConversationId,
        markdown: &str,
        cx: &mut Context<Self>,
    ) {
        let should_seed = self
            .conversations
            .iter()
            .find(|c| c.id == *conv_id)
            .is_some_and(|c| c.active_todos.is_empty());
        if !should_seed {
            return;
        }
        let todos = todos_from_plan(markdown);
        if todos.is_empty() {
            return;
        }
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == *conv_id) {
            conv.active_todos = todos;
        }
        self.sync_plan_status_for_conversation(conv_id, cx);
        cx.notify();
    }

    fn resolve_pending_choices(&mut self, conv_id: &ConversationId) {
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == *conv_id) {
            for item in &mut conv.thread_items {
                if let ThreadItem::ChoiceRequest { resolved, .. } = item {
                    *resolved = true;
                }
            }
        }
    }

    fn begin_approval_resolution(&mut self, cx: &mut Context<Self>) {
        self.diff_panel.pending_approval = None;
        self.pending_thread_approval = None;
        if let Some(conv_id) = self.selected_conversation_id.clone() {
            self.mark_waiting_tools_running(&conv_id);
            self.resolve_approval_item(&conv_id);
        }
        self.status.agent_status = Some(AgentStatus::RunningTool);
        self.sync_thread_approval_state(cx);
    }

    fn mark_waiting_tools_running(&mut self, conv_id: &ConversationId) {
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == *conv_id) {
            for item in &mut conv.thread_items {
                if let ThreadItem::ToolCall { status, .. } = item {
                    if matches!(status, AgentStatus::WaitingApproval) {
                        *status = AgentStatus::RunningTool;
                    }
                }
            }
        }
    }

    fn refresh_token_usage_display(&mut self) {
        let max = context_for_model(
            &self.selected_provider,
            &self.selected_model,
            &self.openrouter_models,
        );
        let max_label = format_context_label(max);
        let parts: Vec<&str> = self.status.token_usage.split(" / ").collect();
        let used = parts.first().copied().unwrap_or("0");
        self.status.token_usage = format!("{used} / {max_label}");
    }

    fn cancel_in_progress_todos(&mut self, conv_id: &ConversationId) {
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == *conv_id) {
            for todo in &mut conv.active_todos {
                if matches!(
                    todo.state,
                    crate::features::shell::state::TodoState::InProgress
                ) {
                    todo.state = crate::features::shell::state::TodoState::Cancelled;
                }
            }
        }
    }
}
