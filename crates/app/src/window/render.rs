//! Render impl for AgentWindow.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnimationExt, Context, Entity, IntoElement, Render, SharedString, Window, div, prelude::*, px,
};
use gpui_component::resizable::{h_resizable, resizable_panel, v_resizable};

use super::AgentWindow;
use super::AppScreen;
use super::conversation_workspace::ConversationWorkspaceVm;
use crate::features::chat::thread_view::{self as thread};
use crate::features::composer::layout::{self as composer, ComposerProps};

use crate::features::inspector::layout::{self as artifact_inspector, DockedInspectorProps};
use crate::features::settings::layout::{self as settings_ui, SettingsProps};
use crate::features::settings::state::SettingsSection;
use crate::features::shell::sidebar_helpers::{self as sidebar};
use crate::features::shell::state::{DockPlacement, InspectorTabId};
use crate::features::terminal::layout::TerminalPanelProps;
use crate::features::todos::components::todo_list::todo_progress_header;
use crate::features::top_bar::layout::{self as top_bar_ui, TopBarProps};
use crate::shared::components::context_usage_ring::ContextUsageProps;
use crate::shared::components::tab_bar::{DraggedTab, dock_tab_drag_placement};
use crate::shared::components::workspace_readiness::{
    WorkspaceReadinessCardProps, workspace_readiness_card,
};
use crate::shared::state::TranscriptMode;
use crate::tokens::Tokens;
use crate::tokens::motion::Motion;
use crate::tokens::theme::{current_mode, current_theme_name, themes_for_mode};
use agent_protocol::AgentMode;

// ── ViewState (cheap, cloneable) ──────────────────────────────────────────

struct ViewState {
    entity: Entity<AgentWindow>,
    screen: AppScreen,
    sidebar_collapsed: bool,
    dark_mode: bool,
    active_theme: String,
    theme_list: Vec<SharedString>,
    selected_provider: String,
    selected_model: String,
    selected_subagent_model: Option<String>,
    selected_settings_section: SettingsSection,
    model_items: Arc<[String]>,
    model_search_keys: Arc<[Arc<str>]>,
    sidebar_view: Entity<crate::features::shell::layout::SidebarView>,
    safety_mode: AgentMode,
    transcript_mode: TranscriptMode,
    conversation_workspace: ConversationWorkspaceVm,
}

// ── Cbs (cloneable callbacks) ─────────────────────────────────────────────

#[derive(Clone)]
struct Cbs {
    toggle_sidebar: Rc<dyn Fn(&mut gpui::App)>,
    toggle_inspector: Rc<dyn Fn(&mut gpui::App)>,
    toggle_terminal: Rc<dyn Fn(&mut gpui::App)>,
    send: Rc<dyn Fn(&mut Window, &mut gpui::App)>,
    cancel: Rc<dyn Fn(&mut gpui::App)>,
    select_inspector_tab: Rc<dyn Fn(InspectorTabId, &mut gpui::App)>,
    close_inspector_tab: Rc<dyn Fn(InspectorTabId, &mut gpui::App)>,
    reorder_inspector_tab: Rc<dyn Fn(InspectorTabId, InspectorTabId, &mut gpui::App)>,
    new_inspector_tab: Rc<dyn Fn(&mut gpui::App)>,
    transcript_mode: Rc<dyn Fn(TranscriptMode, &mut gpui::App)>,
    open_project: Rc<dyn Fn(&mut Window, &mut gpui::App)>,
    new_chat: Rc<dyn Fn(&mut gpui::App)>,
    open_chat: Rc<dyn Fn(&mut gpui::App)>,
    open_search: Rc<dyn Fn(&mut gpui::App)>,
    open_extensions: Rc<dyn Fn(&mut gpui::App)>,
    open_automations: Rc<dyn Fn(&mut gpui::App)>,
    open_settings: Rc<dyn Fn(&mut gpui::App)>,
    open_context: Rc<dyn Fn(&mut gpui::App)>,
    trust_project: Rc<dyn Fn(&mut gpui::App)>,
    toggle_todo_strip: Rc<dyn Fn(&mut gpui::App)>,
    new_terminal_tab: Rc<dyn Fn(&mut gpui::App)>,
    close_terminal_tab: Rc<dyn Fn(u64, &mut gpui::App)>,
    select_terminal_tab: Rc<dyn Fn(u64, &mut gpui::App)>,
    reorder_terminal_tab: Rc<dyn Fn(u64, u64, &mut gpui::App)>,
    move_inspector_tab_to_dock: Rc<dyn Fn(InspectorTabId, DockPlacement, &mut gpui::App)>,
    on_bottom_resize: Rc<dyn Fn(f32, &mut gpui::App)>,
    on_right_dock_resize: Rc<dyn Fn(f32, &mut gpui::App)>,
}

// ═══════════════════════════════════════════════════════════════════════════
//  RENDER
// ═══════════════════════════════════════════════════════════════════════════

impl Render for AgentWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _profile = crate::shared::render_profile::span("AgentWindow::render");
        self.prepare_render(window, cx);
        let cbs = self.build_cbs(cx);
        let vs = self.build_view_state(cx);
        root(self, vs, cbs)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  PREPARATION
// ═══════════════════════════════════════════════════════════════════════════

impl AgentWindow {
    fn prepare_render(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.ensure_thread_view(cx);
        self.ensure_sidebar_view(cx);
        self.sync_inspector_open();
        self.prepare_active_subagent_transcript(cx);
    }

    fn build_view_state(&mut self, cx: &mut Context<Self>) -> ViewState {
        let _profile = crate::shared::render_profile::span("build_view_state");
        let dark_mode = current_mode(cx).is_dark();
        let active_theme = current_theme_name(cx).to_string();
        let theme_list = themes_for_mode(cx, dark_mode);
        let (model_items, model_search_keys) = self.model_picker_items_for_selected_provider();

        ViewState {
            entity: cx.entity().clone(),
            screen: self.screen,
            sidebar_collapsed: self.sidebar_collapsed,
            dark_mode,
            active_theme,
            theme_list,
            selected_provider: self.selected_provider.clone(),
            selected_model: self.selected_model.clone(),
            selected_subagent_model: self.selected_subagent_model.clone(),
            selected_settings_section: self.selected_settings_section,
            model_items,
            model_search_keys,
            sidebar_view: self
                .sidebar_view
                .clone()
                .expect("sidebar view initialized in prepare_render"),
            safety_mode: self.safety_mode.clone(),
            transcript_mode: self.transcript_mode,
            conversation_workspace: self.build_conversation_workspace(cx),
        }
    }

    fn build_cbs(&self, cx: &mut Context<Self>) -> Cbs {
        let e = cx.entity().clone();
        Cbs {
            toggle_sidebar: rc1(e.clone(), |v, cx| v.toggle_sidebar(cx)),
            toggle_inspector: rc1(e.clone(), |v, cx| v.toggle_diff_panel(cx)),
            toggle_terminal: rc1(e.clone(), |v, cx| v.toggle_terminal_panel(cx)),
            send: Rc::new({
                let e = e.clone();
                move |window: &mut Window, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.send_message(window, cx))
                }
            }),
            cancel: rc1(e.clone(), |v, cx| v.cancel_active_run(cx)),
            select_inspector_tab: Rc::new({
                let e = e.clone();
                move |tab_id: InspectorTabId, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.select_inspector_tab(tab_id, cx))
                }
            }),
            close_inspector_tab: Rc::new({
                let e = e.clone();
                move |tab_id: InspectorTabId, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.close_inspector_tab(tab_id, cx))
                }
            }),
            reorder_inspector_tab: Rc::new({
                let e = e.clone();
                move |dragged_id: InspectorTabId, target_id: InspectorTabId, app: &mut gpui::App| {
                    e.update(app, |v, cx| {
                        v.reorder_inspector_tab(dragged_id, target_id, cx)
                    })
                }
            }),
            new_inspector_tab: rc1(e.clone(), |v, cx| v.new_inspector_tab(cx)),
            transcript_mode: Rc::new({
                let e = e.clone();
                move |mode: TranscriptMode, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.set_transcript_mode(mode, cx))
                }
            }),
            open_project: Rc::new({
                let e = e.clone();
                move |window: &mut Window, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.open_project_folder(window, cx))
                }
            }),
            new_chat: rc1(e.clone(), |v, cx| v.new_conversation_from_nav(cx)),
            open_chat: rc1(e.clone(), |v, cx| v.open_chat(cx)),
            open_search: rc1(e.clone(), |v, cx| v.open_search(cx)),
            open_extensions: rc1(e.clone(), |v, cx| v.open_extensions(cx)),
            open_automations: rc1(e.clone(), |v, cx| v.open_automations(cx)),
            open_settings: rc1(e.clone(), |v, cx| v.open_settings(cx)),
            open_context: rc1(e.clone(), |v, cx| v.open_context_workspace_panel(cx)),
            trust_project: rc1(e.clone(), |v, cx| v.trust_selected_project(cx)),
            toggle_todo_strip: rc1(e.clone(), |v, cx| v.toggle_todo_strip(cx)),
            new_terminal_tab: rc1(e.clone(), |v, cx| v.new_terminal_tab(cx)),
            close_terminal_tab: Rc::new({
                let e = e.clone();
                move |id: u64, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.close_terminal_tab(id, cx))
                }
            }),
            reorder_terminal_tab: Rc::new({
                let e = e.clone();
                move |dragged_id: u64, target_id: u64, app: &mut gpui::App| {
                    e.update(app, |v, cx| {
                        v.reorder_terminal_tab(dragged_id, target_id, cx)
                    })
                }
            }),
            move_inspector_tab_to_dock: Rc::new({
                let e = e.clone();
                move |tab_id: InspectorTabId, dock: DockPlacement, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.move_inspector_tab_to_dock(tab_id, dock, cx))
                }
            }),
            on_bottom_resize: Rc::new({
                let e = e.clone();
                move |h: f32, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.set_bottom_panel_height(h, cx))
                }
            }),
            on_right_dock_resize: Rc::new({
                let e = e.clone();
                move |w: f32, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.set_right_dock_width(w, cx))
                }
            }),
            select_terminal_tab: Rc::new({
                let e = e;
                move |id: u64, app: &mut gpui::App| {
                    e.update(app, |v, cx| v.select_terminal_tab(id, cx))
                }
            }),
        }
    }
}

fn rc1<F>(e: Entity<AgentWindow>, f: F) -> Rc<dyn Fn(&mut gpui::App)>
where
    F: Fn(&mut AgentWindow, &mut gpui::Context<AgentWindow>) + 'static,
{
    Rc::new(move |app| e.update(app, |v, cx| f(v, cx)))
}

fn to_boxed(f: Rc<dyn Fn(&mut gpui::App)>) -> Box<dyn Fn(&mut gpui::App) + 'static> {
    Box::new(move |app| f(app))
}

// ═══════════════════════════════════════════════════════════════════════════
//  LAYOUT
// ═══════════════════════════════════════════════════════════════════════════

// Root layout
fn root(agent: &AgentWindow, vs: ViewState, cbs: Cbs) -> impl IntoElement {
    if vs.screen == AppScreen::Settings {
        return div()
            .id("agent-window")
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(Tokens::app_bg())
            .text_color(Tokens::text_primary())
            .child(settings_column(&vs, &cbs))
            .into_any_element();
    }

    div()
        .id("agent-window")
        .size_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(Tokens::app_bg())
        .text_color(Tokens::text_primary())
        .child(top_bar(&vs, &cbs))
        .child(
            div()
                .id("agent-body")
                .flex_1()
                .min_h(px(0.0))
                .overflow_hidden()
                .child(body(agent, &vs, &cbs)),
        )
        .into_any_element()
}

// ── Top bar ───────────────────────────────────────────────────────────────

fn top_bar(vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    let title = top_bar_title(vs);
    let title_copy = title.clone();
    top_bar_ui::render_top_bar(TopBarProps {
        conversation_title: title,
        sidebar_collapsed: vs.sidebar_collapsed,
        inspector_open: vs.conversation_workspace.inspector.open,
        terminal_panel_open: vs.conversation_workspace.terminal.open,
        show_panel_controls: vs.screen == AppScreen::Chat,
        workspace_readiness_label: (!vs.conversation_workspace.readiness.is_ready())
            .then(|| vs.conversation_workspace.readiness.summary_label()),
        on_toggle_sidebar: Some(to_boxed(cbs.toggle_sidebar.clone())),
        on_toggle_inspector: Some(to_boxed(cbs.toggle_inspector.clone())),
        on_toggle_terminal: Some(to_boxed(cbs.toggle_terminal.clone())),
        on_new_chat: Some(to_boxed(cbs.new_chat.clone())),
        on_title_copy: Some(Box::new({
            let title = title_copy;
            move |app: &mut gpui::App| {
                if !title.trim().is_empty() {
                    app.write_to_clipboard(title.clone().into());
                }
            }
        })),
        on_new_terminal_tab: Some(to_boxed(cbs.new_terminal_tab.clone())),
        on_workspace_status: Some(to_boxed(cbs.open_settings.clone())),
        on_open_settings: Some(cbs.open_settings.clone()),
    })
}

// ── Body (sidebar + main workspace + terminal) ────────────────────────────

fn body(agent: &AgentWindow, vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    h_resizable("agent-body-resizable")
        // ── Sidebar panel (left) ──
        .child(
            resizable_panel()
                .visible(!vs.sidebar_collapsed)
                .size(px(if vs.sidebar_collapsed {
                    0.0
                } else {
                    Tokens::SIDEBAR_WIDTH
                }))
                .size_range(if vs.sidebar_collapsed {
                    px(0.0)..px(0.0)
                } else {
                    px(Tokens::SIDEBAR_MIN_WIDTH)..px(Tokens::SIDEBAR_MAX_WIDTH)
                })
                .child(
                    div()
                        .size_full()
                        .min_w(px(0.0))
                        .min_h(px(0.0))
                        .overflow_hidden()
                        .child(sidebar::render_sidebar(vs.sidebar_view.clone()))
                        .with_animation("sidebar-fade", Motion::fade_in(), |el, d| el.opacity(d)),
                ),
        )
        // ── Main workspace ──
        .child(
            resizable_panel()
                .size_range(px(Tokens::CENTER_MIN_WIDTH)..px(6000.0))
                .child(main_workspace(agent, vs, cbs)),
        )
}

// ── Main workspace (center panel + bottom dock) ───────────────────────────

fn main_workspace(agent: &AgentWindow, vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    let on_bottom_resize = {
        let cb = cbs.on_bottom_resize.clone();
        move |state: &gpui::Entity<gpui_component::resizable::ResizableState>,
              _: &mut Window,
              cx: &mut gpui::App| {
            let sizes = state.read(cx).sizes();
            if sizes.len() >= 2 {
                cb(f32::from(sizes[1]), cx);
            }
        }
    };

    div()
        .id("main-workspace")
        .size_full()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .overflow_hidden()
        .relative()
        .child(
            v_resizable("main-workspace-resizable")
                .on_resize(on_bottom_resize)
                // ── Center panel (top) ──
                .child(
                    resizable_panel()
                        .size_range(px(Tokens::CENTER_MIN_WIDTH)..px(6000.0))
                        .child(center_panel(agent, vs, cbs)),
                )
                // ── Terminal panel (bottom) ──
                .child(
                    resizable_panel()
                        .visible(
                            vs.screen == AppScreen::Chat && vs.conversation_workspace.terminal.open,
                        )
                        .size(px(vs.conversation_workspace.terminal.bottom_panel_height))
                        .size_range(
                            px(Tokens::BOTTOM_PANEL_MIN_HEIGHT)
                                ..px(Tokens::BOTTOM_PANEL_MAX_HEIGHT),
                        )
                        .child(
                            div()
                                .size_full()
                                .min_w(px(0.0))
                                .min_h(px(0.0))
                                .overflow_hidden()
                                .child(bottom_dock_content(agent, vs, cbs))
                                .with_animation("terminal-fade", Motion::fade_in(), |el, d| {
                                    el.opacity(d)
                                }),
                        ),
                ),
        )
        .when(
            vs.screen == AppScreen::Chat && !vs.conversation_workspace.terminal.open,
            |el| {
                let move_tab = cbs.move_inspector_tab_to_dock.clone();
                el.child(
                    div()
                        .id("bottom-dock-drop-zone")
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(px(72.0))
                        .drag_over::<DraggedTab>(move |style, drag, _, _| {
                            if dock_tab_drag_placement(drag)
                                .is_some_and(|dock| dock != DockPlacement::Bottom)
                            {
                                style.bg(Tokens::accent().alpha(0.12))
                            } else {
                                style
                            }
                        })
                        .on_drop(move |drag: &DraggedTab, _, app: &mut gpui::App| {
                            if dock_tab_drag_placement(drag)
                                .is_some_and(|dock| dock != DockPlacement::Bottom)
                            {
                                move_tab(drag.tab_id, DockPlacement::Bottom, app);
                            }
                        }),
                )
            },
        )
}

// ── Center panel (chat column + inspector) ────────────────────────────────

fn center_panel(agent: &AgentWindow, vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    let on_right_resize = {
        let cb = cbs.on_right_dock_resize.clone();
        move |state: &gpui::Entity<gpui_component::resizable::ResizableState>,
              _: &mut Window,
              cx: &mut gpui::App| {
            let sizes = state.read(cx).sizes();
            if sizes.len() >= 2 {
                cb(f32::from(sizes[1]), cx);
            }
        }
    };

    div()
        .id("workspace-panels-row")
        .size_full()
        .min_h(px(0.0))
        .overflow_hidden()
        .relative()
        .child(
            h_resizable("workspace-row-resizable")
                .on_resize(on_right_resize)
                .child(
                    resizable_panel()
                        .size_range(px(Tokens::CENTER_MIN_WIDTH)..px(6000.0))
                        .child(column(agent, vs, cbs)),
                )
                .child(
                    resizable_panel()
                        .visible(
                            vs.screen == AppScreen::Chat
                                && vs.conversation_workspace.inspector.open,
                        )
                        .size(px(vs.conversation_workspace.inspector.right_dock_width))
                        .size_range(
                            px(Tokens::DIFF_PANEL_MIN_WIDTH)..px(Tokens::DIFF_PANEL_MAX_WIDTH),
                        )
                        .child(inspector_content(agent, vs, cbs)),
                ),
        )
        .when(
            vs.screen == AppScreen::Chat && !vs.conversation_workspace.inspector.open,
            |el| {
                let move_tab = cbs.move_inspector_tab_to_dock.clone();
                el.child(
                    div()
                        .id("right-dock-drop-zone")
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .w(px(72.0))
                        .drag_over::<DraggedTab>(move |style, drag, _, _| {
                            if dock_tab_drag_placement(drag)
                                .is_some_and(|dock| dock != DockPlacement::Right)
                            {
                                style.bg(Tokens::accent().alpha(0.12))
                            } else {
                                style
                            }
                        })
                        .on_drop(move |drag: &DraggedTab, _, app: &mut gpui::App| {
                            if dock_tab_drag_placement(drag)
                                .is_some_and(|dock| dock != DockPlacement::Right)
                            {
                                move_tab(drag.tab_id, DockPlacement::Right, app);
                            }
                        }),
                )
            },
        )
}

// ── Column ────────────────────────────────────────────────────────────────

fn column(agent: &AgentWindow, vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    match vs.screen {
        AppScreen::Chat => chat_column(agent, vs, cbs).into_any_element(),
        AppScreen::Search => placeholder_screen(
            "Search",
            "Project-wide search and search workflows will live here.",
            "Use the sidebar search box to filter threads, or open the Context/Search inspector for tool evidence today.",
            Some(("Back to threads", cbs.open_chat.clone())),
        )
        .into_any_element(),
        AppScreen::Extensions => placeholder_screen(
            "Extensions",
            "Manage connected tools and extension modules from one place.",
            "This pass adds the product surface and routing; backend management flows can land behind this screen next.",
            Some(("Back to threads", cbs.open_chat.clone())),
        )
        .into_any_element(),
        AppScreen::Automations => placeholder_screen(
            "Automations",
            "Scheduled tasks, monitors, and follow-ups will be managed here.",
            "The screen is now routable from the sidebar chrome and ready for automation-specific state.",
            Some(("Back to threads", cbs.open_chat.clone())),
        )
        .into_any_element(),
        AppScreen::Settings => settings_column(vs, cbs).into_any_element(),
    }
}

fn settings_column(vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    div()
        .id("settings-column")
        .size_full()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .overflow_hidden()
        .bg(Tokens::main_bg())
        .child(settings_ui::render_settings(SettingsProps {
            dark_mode: vs.dark_mode,
            active_theme: vs.active_theme.clone(),
            themes: vs.theme_list.clone(),
            safety_mode: vs.safety_mode.clone(),
            transcript_mode: vs.transcript_mode,
            selected_section: vs.selected_settings_section,
            selected_provider: vs.selected_provider.clone(),
            selected_model: vs.selected_model.clone(),
            selected_subagent_model: vs.selected_subagent_model.clone(),
            model_items: vs.model_items.clone(),
            model_search_keys: vs.model_search_keys.clone(),
            workspace_readiness: vs.conversation_workspace.readiness.clone(),
            entity: vs.entity.clone(),
            on_transcript_mode: Some(Box::new({
                let cb = cbs.transcript_mode.clone();
                move |mode, app| cb(mode, app)
            })),
            on_open_project: Some(Box::new({
                let cb = cbs.open_project.clone();
                move |window, app| cb(window, app)
            })),
            on_trust_project: Some(to_boxed(cbs.trust_project.clone())),
            on_open_context: Some(to_boxed(cbs.open_context.clone())),
        }))
}

// ── Chat column ───────────────────────────────────────────────────────────

fn chat_column(agent: &AgentWindow, vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    div()
        .id("chat-column")
        .relative()
        .size_full()
        .flex()
        .flex_col()
        .min_w(px(Tokens::CENTER_MIN_WIDTH))
        .min_h(px(0.0))
        .overflow_hidden()
        .bg(Tokens::main_bg())
        .when(!vs.conversation_workspace.readiness.is_ready(), |col| {
            col.child(render_workspace_readiness_banner(vs, cbs))
        })
        .when(!vs.conversation_workspace.active_todos.is_empty(), |col| {
            col.child(todo_progress_header(
                &vs.conversation_workspace.active_todos,
                vs.conversation_workspace.todo_strip_expanded,
                to_boxed(cbs.toggle_todo_strip.clone()),
            ))
        })
        .child(thread_layer(vs))
        .child(composer_layer(agent, vs, cbs))
}

// ── Thread layer ──────────────────────────────────────────────────────────

fn render_workspace_readiness_banner(vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    let readiness = &vs.conversation_workspace.readiness;
    let show_trust = readiness.has_project && !readiness.project_trusted;
    let show_context = readiness.has_project;

    div()
        .px(Tokens::thread_padding_x())
        .pt(Tokens::spacing_3())
        .child(
            div()
                .max_w(px(Tokens::THREAD_MAX_WIDTH))
                .child(workspace_readiness_card(WorkspaceReadinessCardProps {
                    readiness: vs.conversation_workspace.readiness.clone(),
                    on_open_settings: Some(cbs.open_settings.clone()),
                    on_open_project: Some(cbs.open_project.clone()),
                    on_trust_project: if show_trust {
                        Some(cbs.trust_project.clone())
                    } else {
                        None
                    },
                    on_open_context: if show_context {
                        Some(cbs.open_context.clone())
                    } else {
                        None
                    },
                })),
        )
}

fn thread_layer(vs: &ViewState) -> impl IntoElement {
    div()
        .id("chat-thread-layer")
        .flex_1()
        .min_h(px(0.0))
        .relative()
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .child(thread::render_thread(
                    vs.conversation_workspace.thread_view.clone(),
                )),
        )
}

// ── Composer layer ────────────────────────────────────────────────────────

fn composer_layer(agent: &AgentWindow, vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    let composer_vm = &vs.conversation_workspace.composer;
    let context_usage: ContextUsageProps = composer_vm.context_usage.to_props();
    let (pending_actions, sticky_approval) = agent.pending_composer_actions(vs.entity.clone());

    div()
        .id("chat-composer-layer")
        .absolute()
        .bottom_0()
        .left_0()
        .right_0()
        .child(composer::render_composer(ComposerProps {
            has_text: composer_vm.has_text,
            input_expanded: composer_vm.input_expanded,
            on_send: Some(Box::new({
                let cb = cbs.send.clone();
                move |window, app| cb(window, app)
            })),
            is_running: composer_vm.is_running,
            on_cancel: Some(Box::new({
                let cb = cbs.cancel.clone();
                move |app| cb(app)
            })),
            input_entity: composer_vm.input_entity.clone(),
            selected_mode: composer_vm.selected_mode.clone(),
            context_usage,
            pending_actions,
            sticky_approval,
            composer_dimmed: composer_vm.dimmed,
            composer_disabled: composer_vm.disabled,
            entity: vs.entity.clone(),
            selected_branch: composer_vm.selected_branch.clone(),
            branch_items: composer_vm.branch_items.clone(),
            selected_model: composer_vm.selected_model.clone(),
            model_items: composer_vm.model_items.clone(),
            model_search_keys: composer_vm.model_search_keys.clone(),
            pending_image_attachments: composer_vm.pending_image_attachments.clone(),
            composer_error: composer_vm.composer_error.clone(),
        }))
}

fn docked_inspector_props(
    vs: &ViewState,
    cbs: &Cbs,
    dock: DockPlacement,
    width: f32,
    show_left_border: bool,
    show_top_border: bool,
    show_close_button: bool,
) -> DockedInspectorProps {
    let inspector = &vs.conversation_workspace.inspector;
    let terminal = &vs.conversation_workspace.terminal;

    DockedInspectorProps {
        dock,
        width,
        tabs: inspector.tabs.clone(),
        store: inspector.store.clone(),
        selection: inspector.selection.clone(),
        selected_subagent: match dock {
            DockPlacement::Right => inspector.selected_subagent_right.clone(),
            DockPlacement::Bottom => inspector.selected_subagent_bottom.clone(),
        },
        context_recap: inspector.context_recap.clone(),
        on_select_tab: Some(cbs.select_inspector_tab.clone()),
        on_close_tab: Some(cbs.close_inspector_tab.clone()),
        on_new_tab: None,
        on_reorder_tab: Some(cbs.reorder_inspector_tab.clone()),
        on_move_tab_to_dock: Some(cbs.move_inspector_tab_to_dock.clone()),
        terminal_panel: Some(TerminalPanelProps {
            tabs: terminal.tabs.clone(),
            terminal_view: terminal.terminal_view.clone(),
            on_new_tab: Some(cbs.new_terminal_tab.clone()),
            on_close_tab: Some(cbs.close_terminal_tab.clone()),
            on_select_tab: Some(cbs.select_terminal_tab.clone()),
            on_reorder_tabs: Some(cbs.reorder_terminal_tab.clone()),
        }),
        android_session: inspector.android_session.clone(),
        diff_panel: {
            let mut props = inspector.diff_panel_props(vs.entity.clone(), show_close_button);
            props.patch_apply_enabled &= !vs.safety_mode.auto_applies_patches();
            props
        },
        show_left_border,
        show_top_border,
    }
}

// ── Inspector content ─────────────────────────────────────────────────────

fn inspector_content(_agent: &AgentWindow, vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    let _profile = crate::shared::render_profile::span("inspector_content");
    div()
        .size_full()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .overflow_hidden()
        .child(artifact_inspector::render_docked_inspector(
            docked_inspector_props(
                vs,
                cbs,
                DockPlacement::Right,
                vs.conversation_workspace.inspector.right_dock_width,
                true,
                false,
                true,
            ),
        ))
        .with_animation("inspector-fade", Motion::fade_in(), |el, d| el.opacity(d))
}

fn bottom_dock_content(_agent: &AgentWindow, vs: &ViewState, cbs: &Cbs) -> impl IntoElement {
    artifact_inspector::render_docked_inspector(docked_inspector_props(
        vs,
        cbs,
        DockPlacement::Bottom,
        0.0,
        false,
        true,
        false,
    ))
}

fn top_bar_title(vs: &ViewState) -> String {
    match vs.screen {
        AppScreen::Chat => {
            if vs.conversation_workspace.title.is_empty() {
                "New thread".into()
            } else {
                vs.conversation_workspace.title.clone()
            }
        }
        AppScreen::Search => "Search".into(),
        AppScreen::Extensions => "Extensions".into(),
        AppScreen::Automations => "Automations".into(),
        AppScreen::Settings => "Settings".into(),
    }
}

fn placeholder_screen(
    title: &'static str,
    body: &'static str,
    detail: &'static str,
    action: Option<(&'static str, Rc<dyn Fn(&mut gpui::App)>)>,
) -> impl IntoElement {
    use crate::shared::components::buttons::btn_outline;

    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(Tokens::main_bg())
        .p(Tokens::spacing_6())
        .child(
            div()
                .max_w(px(560.0))
                .w_full()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_3())
                .child(
                    div()
                        .text_size(Tokens::text_xl())
                        .line_height(Tokens::text_xl_leading())
                        .text_color(Tokens::text_primary())
                        .child(title),
                )
                .child(
                    div()
                        .text_size(Tokens::text_md())
                        .line_height(Tokens::text_md_leading())
                        .text_color(Tokens::text_secondary())
                        .child(body),
                )
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .line_height(Tokens::text_sm_leading())
                        .text_color(Tokens::text_tertiary())
                        .child(detail),
                )
                .when_some(action, |el, (label, cb)| {
                    el.child(
                        btn_outline("placeholder-action", label)
                            .on_click(move |_, _, app: &mut gpui::App| cb(app)),
                    )
                }),
        )
}
