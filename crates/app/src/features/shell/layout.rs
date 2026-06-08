//! Sidebar explorer — isolated entity so thread/status updates don't rebuild the tree.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{Context, Entity, IntoElement, Pixels, Render, Size, Window, div, prelude::*, px, size};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::{VirtualListScrollHandle, v_virtual_list};

use crate::features::shell::components::tree_row::{
    ProjectRowViewModel, SessionRowViewModel, expanded_for_search, filter_projects,
    filter_sidebar_sessions, project_expand_key, project_row, project_show_all_key,
    project_show_more_row, section_label, session_row,
};
use crate::features::shell::sidebar_helpers::{
    render_app_nav, render_projects_section_header, render_sidebar_footer,
};
use crate::features::shell::state::{
    AppNavItem, ConversationId, ExpandedItems, Project, ProjectId, SidebarDropTarget,
    SidebarSession,
};
use crate::tokens::Tokens;
use crate::tokens::motion::{sidebar_expand_in, sidebar_row_in};
use crate::ui::agent_window::AgentWindow;
use crate::window::AppScreen;

#[derive(Clone)]
enum SidebarRow {
    RecentHeader,
    ProjectsHeader {
        first: bool,
    },
    Session {
        row: SessionRowViewModel,
    },
    Project {
        row: ProjectRowViewModel,
    },
    ProjectShowMore {
        project_id: ProjectId,
        remaining: usize,
    },
    ProjectAppendDrop {
        project_id: ProjectId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SidebarRowShape {
    RecentHeader,
    ProjectsHeader,
    Session {
        conv_id: ConversationId,
        indent_level: u32,
        drop_before: bool,
    },
    Project {
        project_id: ProjectId,
    },
    ProjectShowMore {
        project_id: ProjectId,
        remaining: usize,
    },
    ProjectAppendDrop {
        project_id: ProjectId,
        active: bool,
    },
}

const PROJECT_SESSION_PREVIEW_COUNT: usize = 5;

pub struct SidebarView {
    agent: Entity<AgentWindow>,
    projects: Vec<Project>,
    sessions: Vec<SidebarSession>,
    selected_conversation_id: Option<ConversationId>,
    expanded_items: ExpandedItems,
    collapsed: bool,
    screen: AppScreen,
    drop_target: Option<SidebarDropTarget>,
    open_action_menu: Option<String>,
    search_input: Entity<InputState>,
    project_by_id: HashMap<ProjectId, usize>,
    session_by_id: HashMap<ConversationId, usize>,
    unassigned_ids: Vec<ConversationId>,
    visible_rows: Vec<SidebarRow>,
    row_sizes: Rc<Vec<Size<Pixels>>>,
    scroll_handle: VirtualListScrollHandle,
    rows_dirty: bool,
    cached_search_query: String,
}

impl SidebarView {
    pub fn new(
        agent: Entity<AgentWindow>,
        search_input: Entity<InputState>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(&search_input, move |view, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                view.rows_dirty = true;
                cx.notify();
            }
        })
        .detach();

        Self {
            agent,
            projects: Vec::new(),
            sessions: Vec::new(),
            selected_conversation_id: None,
            expanded_items: ExpandedItems::new(),
            collapsed: false,
            screen: AppScreen::Chat,
            drop_target: None,
            open_action_menu: None,
            search_input,
            project_by_id: HashMap::new(),
            session_by_id: HashMap::new(),
            unassigned_ids: Vec::new(),
            visible_rows: Vec::new(),
            row_sizes: Rc::new(Vec::new()),
            scroll_handle: VirtualListScrollHandle::new(),
            rows_dirty: true,
            cached_search_query: String::new(),
        }
    }

    pub fn sync(
        &mut self,
        projects: Vec<Project>,
        sessions: Vec<SidebarSession>,
        selected_conversation_id: Option<ConversationId>,
        expanded_items: ExpandedItems,
        collapsed: bool,
        screen: AppScreen,
        cx: &mut Context<Self>,
    ) {
        if self.projects == projects
            && self.sessions == sessions
            && self.selected_conversation_id == selected_conversation_id
            && self.expanded_items == expanded_items
            && self.collapsed == collapsed
            && self.screen == screen
        {
            return;
        }
        self.project_by_id = projects
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.clone(), i))
            .collect();
        // Pre-compute session index and unassigned list so render never builds a
        // HashMap on the hot path.
        self.session_by_id = sessions
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id.clone(), i))
            .collect();
        let assigned: HashSet<_> = projects
            .iter()
            .flat_map(|p| p.conversations.iter())
            .collect();
        self.unassigned_ids = sessions
            .iter()
            .filter_map(|s| {
                if assigned.contains(&s.id) {
                    None
                } else {
                    Some(s.id.clone())
                }
            })
            .collect();
        self.projects = projects;
        self.sessions = sessions;
        self.selected_conversation_id = selected_conversation_id;
        self.expanded_items = expanded_items;
        self.collapsed = collapsed;
        self.screen = screen;
        self.rows_dirty = true;
        cx.notify();
    }

    pub fn set_drop_target(&mut self, target: Option<SidebarDropTarget>, cx: &mut Context<Self>) {
        if self.drop_target != target {
            self.drop_target = target;
            if !self.rows_dirty {
                self.refresh_row_sizes();
            }
            cx.notify();
        }
    }

    pub fn clear_drop_target(&mut self, cx: &mut Context<Self>) {
        self.set_drop_target(None, cx);
    }

    pub fn set_open_action_menu(&mut self, key: Option<String>, cx: &mut Context<Self>) {
        if self.open_action_menu != key {
            self.open_action_menu = key;
            cx.notify();
        }
    }

    pub fn close_action_menu(&mut self, cx: &mut Context<Self>) {
        self.set_open_action_menu(None, cx);
    }
}

impl Render for SidebarView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _profile = crate::shared::render_profile::span("SidebarView::render");
        if self.collapsed {
            return div()
                .id("sidebar-collapsed")
                .w(px(0.0))
                .h(px(0.0))
                .overflow_hidden();
        }

        div()
            .id("sidebar")
            .flex_shrink_0()
            .w_full()
            .min_w(px(Tokens::SIDEBAR_MIN_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(Tokens::sidebar_bg())
            .border_r_1()
            .border_color(Tokens::sidebar_border())
            .overflow_hidden()
            .child(self.render_expanded(cx))
    }
}

impl SidebarView {
    fn render_expanded(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let search_query = self.search_input.read(cx).value().to_string();
        self.ensure_visible_rows(&search_query);
        let sidebar_clear = cx.entity().clone();

        div()
            .flex()
            .flex_col()
            .h_full()
            .child(render_app_nav(
                self.agent.clone(),
                screen_to_nav(self.screen),
            ))
            .child(
                div()
                    .id("sidebar-scroll")
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .on_mouse_up(gpui::MouseButton::Left, move |_, _, app: &mut gpui::App| {
                        sidebar_clear.update(app, |view, cx| {
                            view.clear_drop_target(cx);
                        });
                    })
                    .child(
                        v_virtual_list(
                            cx.entity().clone(),
                            "sidebar-virtual-list",
                            Rc::clone(&self.row_sizes),
                            |this, range, _window, cx| this.render_visible(range, cx),
                        )
                        .track_scroll(&self.scroll_handle)
                        .flex_1()
                        .min_h(px(0.0))
                        .w_full()
                        .px(Tokens::sidebar_padding())
                        .pb(Tokens::spacing_2()),
                    ),
            )
            .child(render_sidebar_footer(
                self.agent.clone(),
                self.screen == AppScreen::Settings,
            ))
    }

    fn ensure_visible_rows(&mut self, search_query: &str) {
        if !self.rows_dirty && self.cached_search_query == search_query {
            return;
        }

        let profile_start = std::time::Instant::now();
        let search_active = !search_query.trim().is_empty();
        let projects = filter_projects(&self.projects, &self.sessions, search_query);
        let filtered_sessions = filter_sidebar_sessions(&self.sessions, search_query);
        let expanded_items = expanded_for_search(
            &self.projects,
            &self.sessions,
            search_query,
            &self.expanded_items,
        );

        let session_index: Cow<HashMap<ConversationId, usize>> = if search_active {
            let filtered_ids: HashSet<_> = filtered_sessions.iter().map(|s| s.id.clone()).collect();
            let filtered: HashMap<ConversationId, usize> = self
                .session_by_id
                .iter()
                .filter(|(id, _)| filtered_ids.contains(*id))
                .map(|(id, &idx)| (id.clone(), idx))
                .collect();
            Cow::Owned(filtered)
        } else {
            Cow::Borrowed(&self.session_by_id)
        };

        let unassigned: Vec<&SidebarSession> = if search_active {
            let assigned: HashSet<_> = projects
                .iter()
                .flat_map(|p| p.conversations.iter())
                .collect();
            filtered_sessions
                .iter()
                .filter(|s| !assigned.contains(&s.id))
                .collect()
        } else {
            self.unassigned_ids
                .iter()
                .filter_map(|id| self.session_by_id.get(id).map(|&idx| &self.sessions[idx]))
                .collect()
        };

        let mut rows = Vec::new();
        if !unassigned.is_empty() {
            rows.push(SidebarRow::RecentHeader);
            let selected = self.selected_conversation_id.as_ref();
            rows.extend(unassigned.into_iter().map(|session| SidebarRow::Session {
                row: SessionRowViewModel::new(session, selected == Some(&session.id), 0),
            }));
        }

        rows.push(SidebarRow::ProjectsHeader {
            first: rows.is_empty(),
        });

        for project in projects {
            let key = project_expand_key(&project.id);
            let expanded = expanded_items.contains(&key);
            rows.push(SidebarRow::Project {
                row: ProjectRowViewModel::new(&project, expanded),
            });

            if expanded {
                let selected = self.selected_conversation_id.as_ref();
                let show_all =
                    search_active || expanded_items.contains(&project_show_all_key(&project.id));
                let mut project_sessions: Vec<&SidebarSession> = project
                    .conversations
                    .iter()
                    .filter_map(|cid| session_index.get(cid).map(|&idx| &self.sessions[idx]))
                    .collect();

                if !show_all && project_sessions.len() > PROJECT_SESSION_PREVIEW_COUNT {
                    if let Some(selected_id) = selected {
                        if let Some(selected_ix) = project_sessions
                            .iter()
                            .position(|session| &session.id == selected_id)
                        {
                            if selected_ix >= PROJECT_SESSION_PREVIEW_COUNT {
                                let mut preview =
                                    project_sessions[..PROJECT_SESSION_PREVIEW_COUNT - 1].to_vec();
                                preview.push(project_sessions[selected_ix]);
                                project_sessions = preview;
                            } else {
                                project_sessions.truncate(PROJECT_SESSION_PREVIEW_COUNT);
                            }
                        } else {
                            project_sessions.truncate(PROJECT_SESSION_PREVIEW_COUNT);
                        }
                    } else {
                        project_sessions.truncate(PROJECT_SESSION_PREVIEW_COUNT);
                    }
                }

                let visible_count = project_sessions.len();
                rows.extend(
                    project_sessions
                        .into_iter()
                        .map(|session| SidebarRow::Session {
                            row: SessionRowViewModel::new(
                                session,
                                selected == Some(&session.id),
                                1,
                            ),
                        }),
                );

                let remaining = project.conversations.len().saturating_sub(visible_count);
                if expanded && !search_active && remaining > 0 {
                    rows.push(SidebarRow::ProjectShowMore {
                        project_id: project.id.clone(),
                        remaining,
                    });
                }
            }

            rows.push(SidebarRow::ProjectAppendDrop {
                project_id: project.id.clone(),
            });
        }

        self.apply_row_state_diff(rows);
        self.cached_search_query = search_query.to_string();
        self.rows_dirty = false;
        crate::shared::render_profile::record(
            "SidebarView::ensure_visible_rows",
            profile_start.elapsed(),
            self.visible_rows.len() as u64,
        );
    }

    fn refresh_row_sizes(&mut self) {
        let row_sizes = self
            .visible_rows
            .iter()
            .map(|row| size(px(Tokens::SIDEBAR_MAX_WIDTH), self.row_height(row)))
            .collect();
        self.row_sizes = Rc::new(row_sizes);
    }

    fn row_shape(&self, row: &SidebarRow) -> SidebarRowShape {
        match row {
            SidebarRow::RecentHeader => SidebarRowShape::RecentHeader,
            SidebarRow::ProjectsHeader { .. } => SidebarRowShape::ProjectsHeader,
            SidebarRow::Session { row } => SidebarRowShape::Session {
                conv_id: row.conv_id.clone(),
                indent_level: row.indent_level,
                drop_before: matches!(
                    self.drop_target,
                    Some(SidebarDropTarget::BeforeSession(ref id)) if id == &row.conv_id
                ),
            },
            SidebarRow::Project { row } => SidebarRowShape::Project {
                project_id: row.project_id.clone(),
            },
            SidebarRow::ProjectShowMore {
                project_id,
                remaining,
            } => SidebarRowShape::ProjectShowMore {
                project_id: project_id.clone(),
                remaining: *remaining,
            },
            SidebarRow::ProjectAppendDrop { project_id } => SidebarRowShape::ProjectAppendDrop {
                project_id: project_id.clone(),
                active: matches!(
                    self.drop_target,
                    Some(SidebarDropTarget::AppendToProject(ref id)) if id == project_id
                ),
            },
        }
    }

    fn apply_row_state_diff(&mut self, rows: Vec<SidebarRow>) {
        let old_shapes: Vec<_> = self
            .visible_rows
            .iter()
            .map(|row| self.row_shape(row))
            .collect();
        let new_shapes: Vec<_> = rows.iter().map(|row| self.row_shape(row)).collect();

        let mut prefix = 0;
        let shared = old_shapes.len().min(new_shapes.len());
        while prefix < shared && old_shapes[prefix] == new_shapes[prefix] {
            prefix += 1;
        }

        let mut suffix = 0;
        while suffix < old_shapes.len().saturating_sub(prefix)
            && suffix < new_shapes.len().saturating_sub(prefix)
            && old_shapes[old_shapes.len() - 1 - suffix]
                == new_shapes[new_shapes.len() - 1 - suffix]
        {
            suffix += 1;
        }

        let old_sizes = self.row_sizes.as_ref();
        let mut row_sizes = Vec::with_capacity(rows.len());
        for row_ix in 0..rows.len() {
            let size = if row_ix < prefix {
                old_sizes.get(row_ix).copied().unwrap_or_else(|| {
                    size(
                        px(Tokens::SIDEBAR_MAX_WIDTH),
                        self.row_height(&rows[row_ix]),
                    )
                })
            } else if row_ix >= rows.len().saturating_sub(suffix) {
                let old_ix = old_shapes.len() - (rows.len() - row_ix);
                old_sizes.get(old_ix).copied().unwrap_or_else(|| {
                    size(
                        px(Tokens::SIDEBAR_MAX_WIDTH),
                        self.row_height(&rows[row_ix]),
                    )
                })
            } else {
                size(
                    px(Tokens::SIDEBAR_MAX_WIDTH),
                    self.row_height(&rows[row_ix]),
                )
            };
            row_sizes.push(size);
        }

        self.visible_rows = rows;
        self.row_sizes = Rc::new(row_sizes);
    }

    fn render_visible(
        &mut self,
        range: std::ops::Range<usize>,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let start = std::time::Instant::now();
        let row_count = range.len() as u64;
        let rows = range
            .map(|row_ix| match self.visible_rows.get(row_ix).cloned() {
                Some(row) => self.render_row(row, cx),
                None => div()
                    .w_full()
                    .h(px(Tokens::ROW_HEIGHT_MD))
                    .into_any_element(),
            })
            .collect();
        crate::shared::render_profile::record(
            "SidebarView::render_visible",
            start.elapsed(),
            row_count,
        );
        rows
    }

    fn render_row(&mut self, row: SidebarRow, cx: &mut Context<Self>) -> gpui::AnyElement {
        let _profile = crate::shared::render_profile::span("SidebarView::render_row");
        let sidebar = cx.entity().clone();
        let entity = self.agent.clone();
        let drop_target = self.drop_target.clone();
        let open_action_menu = self.open_action_menu.clone();

        match row {
            SidebarRow::RecentHeader => div()
                .w_full()
                .h(self.row_height(&SidebarRow::RecentHeader))
                .flex()
                .items_end()
                .child(section_label("RECENT", true))
                .into_any_element(),
            SidebarRow::ProjectsHeader { first } => div()
                .w_full()
                .h(self.row_height(&SidebarRow::ProjectsHeader { first }))
                .flex()
                .items_end()
                .child(render_projects_section_header(first, entity))
                .into_any_element(),
            SidebarRow::Session { row } => {
                let animation_id = row.row_id.clone();
                let indent_level = row.indent_level;
                let content = session_row(row, &drop_target, &open_action_menu, entity, sidebar);
                if indent_level > 0 {
                    sidebar_expand_in(content, animation_id).into_any_element()
                } else {
                    sidebar_row_in(content, animation_id).into_any_element()
                }
            }
            SidebarRow::Project { row } => {
                let animation_id = row.row_id.clone();
                sidebar_row_in(
                    project_row(row, &open_action_menu, entity, sidebar),
                    animation_id,
                )
                .into_any_element()
            }
            SidebarRow::ProjectShowMore {
                project_id,
                remaining,
            } => project_show_more_row(project_id, remaining, entity).into_any_element(),
            SidebarRow::ProjectAppendDrop { project_id } => {
                crate::features::shell::components::tree_row::project_append_drop_zone(
                    &project_id,
                    &drop_target,
                    entity,
                    sidebar,
                )
                .into_any_element()
            }
        }
    }

    fn row_height(&self, row: &SidebarRow) -> Pixels {
        match row {
            SidebarRow::RecentHeader => px(Tokens::ROW_HEIGHT_SM),
            SidebarRow::ProjectsHeader { .. } => px(Tokens::ROW_HEIGHT_LG),
            SidebarRow::Session { row } => {
                let divider = matches!(
                    self.drop_target,
                    Some(SidebarDropTarget::BeforeSession(ref id)) if id == &row.conv_id
                );
                if divider {
                    px(Tokens::ROW_HEIGHT_MD) + sidebar_drop_slot_height()
                } else {
                    px(Tokens::ROW_HEIGHT_MD)
                }
            }
            SidebarRow::Project { .. } => px(Tokens::ROW_HEIGHT_MD),
            SidebarRow::ProjectShowMore { .. } => px(Tokens::ROW_HEIGHT_MD),
            SidebarRow::ProjectAppendDrop { project_id } => {
                let active = matches!(
                    self.drop_target,
                    Some(SidebarDropTarget::AppendToProject(ref id)) if id == project_id
                );
                if active {
                    sidebar_drop_slot_height()
                } else {
                    sidebar_project_append_hitbox_height()
                }
            }
        }
    }
}

fn screen_to_nav(screen: AppScreen) -> AppNavItem {
    match screen {
        AppScreen::Chat => AppNavItem::Chat,
        AppScreen::Search => AppNavItem::Search,
        AppScreen::Extensions => AppNavItem::Extensions,
        AppScreen::Automations => AppNavItem::Automations,
        AppScreen::Settings => AppNavItem::Settings,
    }
}

fn sidebar_drop_slot_height() -> Pixels {
    Tokens::spacing_2() + Tokens::spacing_0p5()
}

fn sidebar_project_append_hitbox_height() -> Pixels {
    Tokens::spacing_1() + Tokens::spacing_0p5()
}
