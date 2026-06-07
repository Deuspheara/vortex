//! Artifact inspector — width-aware wrapper over the diff panel.

pub use crate::features::diff_panel::layout::{DiffPanelProps, render_diff_panel_with_width};

use std::rc::Rc;

use gpui::{FontWeight, IntoElement, TextAlign, div, prelude::*, px};
use gpui_component::scroll::ScrollableElement;

use crate::features::android_simulator::render_android_simulator_panel;
use crate::features::chat::thread_view::ThreadView;
use crate::features::inspector::artifact::{Artifact, ArtifactSelection, ArtifactStore};
use crate::features::inspector::state::{
    DockPlacement, InspectorCustomSlot, InspectorTabId, InspectorTabKind, InspectorTabs,
    InspectorView,
};
use crate::features::shell::state::ContextInspectorRecap;
use crate::features::terminal::layout::TerminalPanelProps;
use crate::shared::components::buttons::btn_icon_sm;
use crate::shared::components::tab_bar::{
    DraggedTab, DraggedTabKind, TabBarProps, TabItem, dock_tab_drag_placement, tab_bar,
};
use crate::tokens::Tokens;
use crate::tokens::icons;

pub struct DockedInspectorProps {
    pub dock: DockPlacement,
    pub width: f32,
    pub tabs: InspectorTabs,
    pub store: ArtifactStore,
    pub selection: ArtifactSelection,
    pub selected_subagent: Option<SubagentInspectorVm>,
    pub context_recap: ContextInspectorRecap,
    pub diff_panel: DiffPanelProps,
    pub terminal_panel: Option<TerminalPanelProps>,
    pub android_session: agent_protocol::AndroidSessionState,
    pub on_select_tab: Option<Rc<dyn Fn(InspectorTabId, &mut gpui::App)>>,
    pub on_close_tab: Option<Rc<dyn Fn(InspectorTabId, &mut gpui::App)>>,
    pub on_new_tab: Option<Rc<dyn Fn(&mut gpui::App)>>,
    pub on_reorder_tab: Option<Rc<dyn Fn(InspectorTabId, InspectorTabId, &mut gpui::App)>>,
    pub on_move_tab_to_dock: Option<Rc<dyn Fn(InspectorTabId, DockPlacement, &mut gpui::App)>>,
    pub show_left_border: bool,
    pub show_top_border: bool,
}

#[derive(Clone)]
pub struct SubagentInspectorVm {
    pub item_id: String,
    pub task: String,
    pub model: String,
    pub summary: String,
    pub status_label: &'static str,
    pub thread_view: gpui::Entity<ThreadView>,
}

pub fn render_docked_inspector(props: DockedInspectorProps) -> impl IntoElement {
    let tabs = props.tabs.clone();
    let on_select = props.on_select_tab.clone();
    let on_close = props.on_close_tab.clone();
    let on_add = props.on_new_tab.clone();
    let dock = props.dock;
    let move_tab_to_dock = props.on_move_tab_to_dock.clone();

    div()
        .id(match dock {
            DockPlacement::Right => "artifact-inspector",
            DockPlacement::Bottom => "bottom-dock",
        })
        .w_full()
        .h_full()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .bg(Tokens::panel_bg())
        .when(props.show_left_border, |el| {
            el.border_l_1().border_color(Tokens::border_subtle())
        })
        .when(props.show_top_border, |el| {
            el.border_t_1().border_color(Tokens::border_subtle())
        })
        .overflow_hidden()
        .when_some(move_tab_to_dock, |el, move_tab| {
            el.drag_over::<DraggedTab>(move |style, drag, _, _| {
                if dock_tab_drag_placement(drag).is_some_and(|source| source != dock) {
                    style.bg(Tokens::accent().alpha(0.12))
                } else {
                    style
                }
            })
            .on_drop(move |drag: &DraggedTab, _, app: &mut gpui::App| {
                if dock_tab_drag_placement(drag).is_some_and(|source| source != dock) {
                    move_tab(drag.tab_id, dock, app);
                }
            })
        })
        .child(render_inspector_tabs(
            dock,
            tabs,
            on_select,
            on_close,
            on_add,
            props.on_reorder_tab.clone(),
        ))
        .child(render_inspector_body(props).into_any_element())
        .into_any_element()
}

fn render_inspector_tabs(
    dock: DockPlacement,
    inspector_tabs: InspectorTabs,
    on_select: Option<Rc<dyn Fn(InspectorTabId, &mut gpui::App)>>,
    on_close: Option<Rc<dyn Fn(InspectorTabId, &mut gpui::App)>>,
    on_add: Option<Rc<dyn Fn(&mut gpui::App)>>,
    on_reorder: Option<Rc<dyn Fn(InspectorTabId, InspectorTabId, &mut gpui::App)>>,
) -> impl IntoElement {
    let tabs: Vec<TabItem> = inspector_tabs
        .tabs_for_dock(dock)
        .iter()
        .map(|tab| TabItem {
            id: tab.id,
            label: tab.title.clone(),
            icon: inspector_tab_icon(&tab.kind),
        })
        .collect();

    div()
        .id("inspector-tabs")
        .flex()
        .border_b_1()
        .border_color(Tokens::border_subtle())
        .child(
            div()
                .h(px(Tokens::tab_bar_height()))
                .px(Tokens::spacing_2())
                .flex()
                .items_center()
                .child(tab_bar(TabBarProps {
                    id: match dock {
                        DockPlacement::Right => "inspector-right",
                        DockPlacement::Bottom => "inspector-bottom",
                    },
                    tabs,
                    selected_id: inspector_tabs.active_id_for_dock(dock).unwrap_or(0),
                    on_select,
                    on_close,
                    on_add: None,
                    on_reorder,
                    drag_kind: Some(DraggedTabKind::InspectorDock(dock)),
                })),
        )
        .when_some(on_add, |el, cb| {
            el.child(
                btn_icon_sm("inspector-header-new-tab", icons::PLUS)
                    .on_click(move |_, _, app| cb(app)),
            )
        })
}

fn render_inspector_body(props: DockedInspectorProps) -> impl IntoElement {
    let Some(active_tab) = props.tabs.active_for_dock(props.dock).cloned() else {
        return render_empty_panel().into_any_element();
    };

    match active_tab.kind {
        InspectorTabKind::BuiltIn(InspectorView::Changes) => {
            let mut diff_panel = props.diff_panel;
            diff_panel.state.active_tab = crate::features::shell::state::ReviewPanelTab::Changes;
            render_diff_panel_with_width(diff_panel, props.width).into_any_element()
        }
        InspectorTabKind::BuiltIn(InspectorView::Context) => {
            render_context_tab(&props.context_recap).into_any_element()
        }
        InspectorTabKind::BuiltIn(InspectorView::Plan) => {
            let mut diff_panel = props.diff_panel;
            diff_panel.plan_available = true;
            diff_panel.state.active_tab = crate::features::shell::state::ReviewPanelTab::Plan;
            render_diff_panel_with_width(diff_panel, props.width).into_any_element()
        }
        InspectorTabKind::BuiltIn(InspectorView::Terminal) => props
            .terminal_panel
            .map(|panel| {
                crate::features::terminal::layout::render_terminal_panel(panel).into_any_element()
            })
            .unwrap_or_else(|| render_terminal_tab_hint().into_any_element()),
        InspectorTabKind::Artifact(id) => props
            .store
            .get(&id)
            .map(render_artifact_view)
            .unwrap_or_else(|| render_empty_panel().into_any_element()),
        InspectorTabKind::Subagent(item_id) => props
            .selected_subagent
            .filter(|subagent| subagent.item_id == item_id)
            .map(render_subagent_view)
            .unwrap_or_else(|| render_empty_panel().into_any_element()),
        InspectorTabKind::Custom(slot) => {
            render_custom_slot(slot, props.android_session).into_any_element()
        }
    }
}

fn inspector_tab_icon(kind: &InspectorTabKind) -> Option<gpui_component::IconName> {
    Some(match kind {
        InspectorTabKind::BuiltIn(InspectorView::Changes) => icons::GIT_COMPARE,
        InspectorTabKind::BuiltIn(InspectorView::Context) => icons::SEARCH,
        InspectorTabKind::BuiltIn(InspectorView::Plan) => icons::CHECKLIST,
        InspectorTabKind::BuiltIn(InspectorView::Terminal) => icons::TERMINAL,
        InspectorTabKind::Artifact(_) => icons::FILE_TEXT,
        InspectorTabKind::Subagent(_) => icons::BOT,
        InspectorTabKind::Custom(InspectorCustomSlot::Browser) => icons::GLOBE,
        InspectorTabKind::Custom(InspectorCustomSlot::Search) => icons::SEARCH,
        InspectorTabKind::Custom(InspectorCustomSlot::AndroidSimulator)
        | InspectorTabKind::Custom(InspectorCustomSlot::IosSimulator) => icons::APP_WINDOW,
        InspectorTabKind::Custom(_) => icons::APP_WINDOW,
    })
}

fn render_artifact_view(artifact: &Artifact) -> gpui::AnyElement {
    match &artifact.kind {
        crate::features::shell::state::ArtifactKind::Terminal
        | crate::features::shell::state::ArtifactKind::WebSource
        | crate::features::shell::state::ArtifactKind::Screenshot
        | crate::features::shell::state::ArtifactKind::Vision => {
            render_terminal_view(artifact).into_any_element()
        }
        _ => render_empty_panel().into_any_element(),
    }
}

fn render_context_tab(recap: &ContextInspectorRecap) -> impl IntoElement {
    let project_status = recap.project_status.clone();
    let context_trace = recap.context_trace.clone();
    let read_cache = recap.read_cache.clone();
    let page_cache = recap.page_cache.clone();

    div()
        .id("artifact-inspector-context")
        .w_full()
        .flex_1()
        .min_h(px(0.0))
        .overflow_hidden()
        .child(
            div()
                .size_full()
                .overflow_y_scrollbar()
                .p(Tokens::spacing_3())
                .flex()
                .flex_col()
                .gap(Tokens::spacing_5())
                .child(render_context_overview(recap))
                .child(inspector_group(
                    "Technical details · repo index",
                    vec![
                        project_status
                            .as_ref()
                            .map(|status| inspector_row("Status", status.badge_label().to_string()))
                            .unwrap_or_else(|| {
                                inspector_row("Status", "No project selected".to_string())
                            }),
                        project_status
                            .as_ref()
                            .map(|status| inspector_row("Phase", status.phase.label().to_string()))
                            .unwrap_or_else(|| inspector_row("Phase", "Unindexed".to_string())),
                        inspector_row(
                            "Last refresh",
                            project_status
                                .as_ref()
                                .and_then(|status| status.last_indexed_at.clone())
                                .unwrap_or_else(|| "Never".to_string()),
                        ),
                        inspector_row(
                            "Ignore sources",
                            project_status
                                .as_ref()
                                .map(|status| {
                                    if status.active_ignore_sources.is_empty() {
                                        "Built-in only".to_string()
                                    } else {
                                        status.active_ignore_sources.join(", ")
                                    }
                                })
                                .unwrap_or_else(|| "Built-in only".to_string()),
                        ),
                        inspector_row(
                            "Indexed files",
                            project_status
                                .as_ref()
                                .map(|status| status.stats.files_indexed.to_string())
                                .unwrap_or_else(|| "0".to_string()),
                        ),
                        inspector_row(
                            "Skipped",
                            project_status
                                .as_ref()
                                .map(|status| {
                                    format!(
                                        "ignore {} · hidden {} · binary {} · large {} · policy {}",
                                        status.stats.skipped_ignore,
                                        status.stats.skipped_hidden,
                                        status.stats.skipped_binary,
                                        status.stats.skipped_large,
                                        status.stats.skipped_policy
                                    )
                                })
                                .unwrap_or_else(|| {
                                    "ignore 0 · hidden 0 · binary 0 · large 0 · policy 0"
                                        .to_string()
                                }),
                        ),
                        inspector_row(
                            "Cached nodes",
                            project_status
                                .as_ref()
                                .map(|status| {
                                    format!(
                                        "symbols {} · summaries {}",
                                        status.stats.symbols_indexed, status.stats.summaries_cached
                                    )
                                })
                                .unwrap_or_else(|| "symbols 0 · summaries 0".to_string()),
                        ),
                        project_status
                            .as_ref()
                            .and_then(|status| status.last_error.clone())
                            .map(|error| inspector_row("Last error", error))
                            .unwrap_or_else(|| inspector_row("Last error", "None".to_string())),
                    ],
                ))
                .child(inspector_group(
                    "Technical details · prompt context",
                    if context_trace.is_empty() {
                        vec![inspector_row(
                            "Summary",
                            "No context trace recorded".to_string(),
                        )]
                    } else {
                        context_trace
                            .iter()
                            .map(|summary| {
                                inspector_row(summary.kind.label(), summary.count.to_string())
                            })
                            .collect()
                    },
                ))
                .child(inspector_group(
                    "Technical details · read cache",
                    vec![
                        inspector_row("Entries", read_cache.entries.to_string()),
                        inspector_row("Hits", read_cache.hits.to_string()),
                        inspector_row("Bytes", format_bytes(read_cache.bytes)),
                    ],
                ))
                .child(inspector_group(
                    "Technical details · page cache",
                    vec![
                        inspector_row(
                            "Configured",
                            if page_cache.configured { "Yes" } else { "No" }.to_string(),
                        ),
                        inspector_row("Cached pages", page_cache.cached_pages.to_string()),
                    ],
                )),
        )
}

fn render_context_overview(recap: &ContextInspectorRecap) -> impl IntoElement {
    let project_status = recap.project_status.as_ref();
    let context_groups = recap
        .context_trace
        .iter()
        .map(|item| item.count)
        .sum::<usize>();
    let (badge, title, detail) = match project_status.map(|status| status.phase) {
        None => (
            "Needs project",
            "Open a project to ground the agent in your repo.",
            "Until a project is selected, runs can’t build repo-aware context or inspect code health.",
        ),
        Some(crate::features::shell::state::IndexPhase::Ready)
            if context_groups > 0 || recap.read_cache.entries > 0 =>
        {
            (
                "Ready",
                "The agent has grounded context ready for the next run.",
                "Recent context traces and cached reads give the next answer a strong starting point.",
            )
        }
        Some(crate::features::shell::state::IndexPhase::Ready) => (
            "Almost ready",
            "The repo is indexed and ready for the first grounded run.",
            "Run a task to build evidence you can inspect here afterward.",
        ),
        Some(
            crate::features::shell::state::IndexPhase::Queued
            | crate::features::shell::state::IndexPhase::Scanning
            | crate::features::shell::state::IndexPhase::Parsing
            | crate::features::shell::state::IndexPhase::Summarizing,
        ) => (
            "Preparing",
            "Context quality is improving while indexing finishes.",
            "You can keep working, but search and grounding will get stronger as the repo index completes.",
        ),
        Some(crate::features::shell::state::IndexPhase::Stale) => (
            "Refresh recommended",
            "The repo changed since the last good index.",
            "Refresh context before relying on search-heavy or edit-heavy runs.",
        ),
        Some(crate::features::shell::state::IndexPhase::Failed) => (
            "Needs attention",
            "Indexing failed for the selected project.",
            "Use the details below to inspect the last error before your next grounded run.",
        ),
        Some(crate::features::shell::state::IndexPhase::Unindexed) => (
            "Needs indexing",
            "The project is open, but repo context is not ready yet.",
            "Let indexing finish or inspect the repo index details below.",
        ),
    };

    let indexed_files = project_status
        .map(|status| status.stats.files_indexed.to_string())
        .unwrap_or_else(|| "0".to_string());
    let cache_summary = if recap.read_cache.entries == 0 {
        "No cached reads yet".to_string()
    } else {
        format!(
            "{} cached read{}",
            recap.read_cache.entries,
            if recap.read_cache.entries == 1 {
                ""
            } else {
                "s"
            }
        )
    };
    let trace_summary = if context_groups == 0 {
        "No recent evidence yet".to_string()
    } else {
        format!(
            "{} recent context signal{}",
            context_groups,
            if context_groups == 1 { "" } else { "s" }
        )
    };

    div()
        .w_full()
        .px(Tokens::spacing_3())
        .py(Tokens::spacing_3())
        .rounded(Tokens::radius_md())
        .border_1()
        .border_color(Tokens::border_subtle())
        .bg(Tokens::surface())
        .flex()
        .flex_col()
        .gap(Tokens::spacing_3())
        .child(
            div()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_1())
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Tokens::accent())
                        .child(badge),
                )
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Tokens::text_primary())
                        .child(title),
                )
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .line_height(Tokens::text_sm_leading())
                        .text_color(Tokens::text_secondary())
                        .child(detail),
                ),
        )
        .child(inspector_row("Indexed files", indexed_files))
        .child(inspector_row("Context evidence", trace_summary))
        .child(inspector_row("Cached reads", cache_summary))
        .child(inspector_row(
            "Web page support",
            if recap.page_cache.configured {
                "Configured".to_string()
            } else {
                "Not configured".to_string()
            },
        ))
}

fn inspector_group(title: &str, rows: Vec<gpui::AnyElement>) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_1())
        .child(
            div()
                .text_size(Tokens::text_xs())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(Tokens::text_tertiary())
                .child(title.to_uppercase()),
        )
        .children(rows)
}

fn inspector_row(label: &str, value: String) -> gpui::AnyElement {
    let label = label.to_string();
    div()
        .w_full()
        .min_h(px(Tokens::ROW_HEIGHT_MD))
        .py(Tokens::spacing_1())
        .border_b_1()
        .border_color(Tokens::border_subtle())
        .flex()
        .items_start()
        .justify_between()
        .gap(Tokens::spacing_2())
        .child(
            div()
                .text_size(Tokens::text_xs())
                .text_color(Tokens::text_tertiary())
                .child(label),
        )
        .child(
            div()
                .max_w(px(260.0))
                .text_size(Tokens::text_xs())
                .text_align(TextAlign::Right)
                .text_color(Tokens::text_secondary())
                .child(value),
        )
        .into_any_element()
}

fn format_bytes(bytes: usize) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{bytes} B")
    }
}

fn render_custom_slot(
    slot: InspectorCustomSlot,
    android_session: agent_protocol::AndroidSessionState,
) -> impl IntoElement {
    match slot {
        InspectorCustomSlot::AndroidSimulator => {
            render_android_simulator_panel(android_session).into_any_element()
        }
        _ => render_empty_panel().into_any_element(),
    }
}

fn render_subagent_view(subagent: SubagentInspectorVm) -> gpui::AnyElement {
    div()
        .id("artifact-inspector-subagent")
        .w_full()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            div()
                .px(Tokens::spacing_3())
                .py(Tokens::spacing_3())
                .flex()
                .flex_col()
                .gap(Tokens::spacing_1())
                .border_b_1()
                .border_color(Tokens::border_subtle())
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_tertiary())
                        .child("Subagent"),
                )
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .font_weight(FontWeight::MEDIUM)
                        .child(subagent.task),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(Tokens::spacing_2())
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_tertiary())
                        .child(subagent.model)
                        .child("•")
                        .child(subagent.status_label),
                )
                .when(!subagent.summary.trim().is_empty(), |el| {
                    el.child(
                        div()
                            .text_size(Tokens::text_sm())
                            .text_color(Tokens::text_secondary())
                            .line_height(Tokens::text_sm_leading())
                            .child(subagent.summary),
                    )
                }),
        )
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .overflow_hidden()
                .child(subagent.thread_view),
        )
        .into_any_element()
}

fn render_empty_panel() -> impl IntoElement {
    div()
        .id("artifact-inspector-empty")
        .w_full()
        .flex_1()
        .min_h(px(0.0))
        .bg(Tokens::panel_bg())
}

fn render_terminal_view(artifact: &Artifact) -> impl IntoElement {
    let output = artifact.terminal_output.as_deref().unwrap_or("(no output)");
    div()
        .id("artifact-inspector-terminal")
        .w_full()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            div()
                .px(Tokens::spacing_3())
                .py(Tokens::spacing_2())
                .border_b_1()
                .border_color(Tokens::border_subtle())
                .text_size(Tokens::text_sm())
                .font_weight(FontWeight::MEDIUM)
                .child(artifact.title.clone()),
        )
        .child(
            div()
                .flex_1()
                .overflow_hidden()
                .p(Tokens::spacing_3())
                .text_size(Tokens::text_xs())
                .text_color(Tokens::text_secondary())
                .child(output.to_string()),
        )
}

fn render_terminal_tab_hint() -> impl IntoElement {
    div()
        .id("artifact-inspector-terminal-hint")
        .w_full()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .overflow_hidden()
                .p(Tokens::spacing_3())
                .flex()
                .flex_col()
                .gap(Tokens::spacing_2())
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .text_color(Tokens::text_secondary())
                        .child("Use the bottom panel for an interactive shell at the project root."),
                )
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_tertiary())
                        .child("Agent command output appears here when you select a terminal artifact from the thread."),
                ),
        )
}
