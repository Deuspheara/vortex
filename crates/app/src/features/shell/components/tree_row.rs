//! Shared sidebar tree rows — flat list items for projects and sessions.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui_component::Icon;

use crate::features::shell::components::sidebar_row_menu::{
    SidebarRowMenuItem, sidebar_overflow_menu,
};
use crate::features::shell::layout::SidebarView;
use crate::features::shell::state::{
    ConversationId, Project, ProjectId, SidebarDropTarget, SidebarSession,
};
use crate::shared::components::buttons::btn_icon_sm;
use crate::tokens::icons;
use crate::tokens::motion::{
    Motion, sidebar_accent_in, sidebar_glow_pulse, sidebar_text_cascade_in, sidebar_text_fade_in,
};
use crate::tokens::{Tokens, element_key};

const SESSION_META_WIDTH: f32 = 36.0;
const PROJECT_META_WIDTH: f32 = 64.0;
use crate::ui::agent_window::AgentWindow;

use gpui::{
    AnimationExt, AppContext, Context, ElementId, Entity, FontWeight, IntoElement, MouseButton,
    Render, SharedString, TextAlign, Window, div, prelude::*, px,
};

fn sidebar_row_label_color(_selected: bool) -> gpui::Hsla {
    Tokens::text_primary()
}

fn sidebar_session_selected_bg() -> gpui::Hsla {
    Tokens::surface_hover().blend(Tokens::text_primary().opacity(0.03))
}

fn session_title_opacity(_indent_level: u32, _selected: bool) -> f32 {
    1.0
}

fn session_meta_opacity(indent_level: u32, is_menu_open: bool) -> f32 {
    if is_menu_open {
        0.0
    } else {
        let _ = indent_level;
        0.82
    }
}

fn sidebar_row_beam(active: bool, key: &str) -> impl IntoElement {
    let beam = div()
        .absolute()
        .top(px(6.0))
        .bottom(px(6.0))
        .left_0()
        .w(px(2.0))
        .rounded(Tokens::radius_full())
        .bg(Tokens::sidebar_accent_beam_gradient());

    if active {
        sidebar_accent_in(beam, element_key("sidebar-row-beam", key)).into_any_element()
    } else {
        div().into_any_element()
    }
}

fn sidebar_row_wash(active: bool, key: &str, phase: f32) -> impl IntoElement {
    let wash = div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom_0()
        .bg(Tokens::sidebar_row_wash_gradient());

    if active {
        sidebar_glow_pulse(wash, element_key("sidebar-row-wash", key), 0.3, 0.22, phase)
            .into_any_element()
    } else {
        div().into_any_element()
    }
}

/// Drag payload for reordering / moving sidebar sessions.
#[derive(Clone)]
pub struct DragSession {
    pub conversation_id: ConversationId,
    pub title: String,
}

#[derive(Clone)]
pub struct SessionRowViewModel {
    pub conv_id: ConversationId,
    pub title: String,
    pub updated_at: String,
    pub selected: bool,
    pub indent_level: u32,
    pub cascade_index: usize,
    pub row_id: ElementId,
    pub wrap_id: ElementId,
    pub title_id: ElementId,
    pub overflow_id: ElementId,
    pub menu_key: String,
    pub group_name: SharedString,
}

impl SessionRowViewModel {
    pub fn new(
        session: &SidebarSession,
        selected: bool,
        indent_level: u32,
        cascade_index: usize,
    ) -> Self {
        let id = &session.id.0;
        Self {
            conv_id: session.id.clone(),
            title: session.title.clone(),
            updated_at: session.updated_at.clone(),
            selected,
            indent_level,
            cascade_index,
            row_id: ElementId::from(SharedString::from(format!("session-{id}"))),
            wrap_id: ElementId::from(SharedString::from(format!("session-wrap-{id}"))),
            title_id: element_key("session-title", id),
            overflow_id: element_key("session-overflow", id),
            menu_key: format!("session-{id}"),
            group_name: SharedString::from(format!("session-row-{id}")),
        }
    }
}

#[derive(Clone)]
pub struct ProjectRowViewModel {
    pub project_id: ProjectId,
    pub name: String,
    pub count: usize,
    pub expanded: bool,
    pub cascade_index: usize,
    pub row_id: ElementId,
    pub toggle_id: ElementId,
    pub name_id: ElementId,
    pub new_conv_id: ElementId,
    pub overflow_id: ElementId,
    pub menu_key: String,
    pub group_name: SharedString,
}

impl ProjectRowViewModel {
    pub fn new(project: &Project, expanded: bool, cascade_index: usize) -> Self {
        let id = &project.id.0;
        Self {
            project_id: project.id.clone(),
            name: project.name.clone(),
            count: project.conversations.len(),
            expanded,
            cascade_index,
            row_id: ElementId::from(SharedString::from(format!("project-{id}"))),
            toggle_id: element_key("project-toggle", id),
            name_id: element_key("project-name", id),
            new_conv_id: element_key("new-conv", id),
            overflow_id: element_key("project-overflow", id),
            menu_key: format!("project-{id}"),
            group_name: SharedString::from(format!("project-row-{id}")),
        }
    }
}

impl Render for DragSession {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("drag-session")
            .cursor_grab()
            .h(px(Tokens::ROW_HEIGHT_MD))
            .px(Tokens::spacing_2())
            .rounded(Tokens::radius_xs())
            .bg(Tokens::surface_active())
            .opacity(0.85)
            .flex()
            .items_center()
            .gap(Tokens::spacing_2())
            .child(
                Icon::new(icons::MESSAGE_SQUARE)
                    .size(px(13.0))
                    .text_color(Tokens::accent()),
            )
            .child(
                div()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_size(Tokens::text_sm())
                    .text_color(Tokens::text_primary())
                    .child(self.title.clone()),
            )
    }
}

/// Animated accent line shown between rows during drag.
fn sidebar_drop_divider(active: bool, key: &str) -> impl IntoElement {
    div()
        .id(element_key("sidebar-drop-slot", key))
        .w_full()
        .overflow_hidden()
        .when(active, |el| {
            el.h(px(10.0)).flex().items_center().child(
                div()
                    .id(element_key("sidebar-drop-line", key))
                    .w_full()
                    .h(px(2.0))
                    .rounded(Tokens::radius_full())
                    .with_animation(
                        element_key("sidebar-drop-anim", key),
                        Motion::sidebar_expand(),
                        |line, delta| line.opacity(delta).bg(Tokens::accent()),
                    ),
            )
        })
        .when(!active, |el| el.h(px(0.0)))
}

fn drop_before_session(
    conv_id: &ConversationId,
    drop_target: &Option<SidebarDropTarget>,
) -> impl IntoElement {
    let active = matches!(
        drop_target,
        Some(SidebarDropTarget::BeforeSession(id)) if id == conv_id
    );
    sidebar_drop_divider(active, &conv_id.0)
}

fn drop_append_project(
    project_id: &ProjectId,
    drop_target: &Option<SidebarDropTarget>,
    indent_level: u32,
) -> impl IntoElement {
    let active = matches!(
        drop_target,
        Some(SidebarDropTarget::AppendToProject(id)) if id == project_id
    );
    div()
        .pl(Tokens::tree_indent(indent_level + 1))
        .pr(Tokens::spacing_2())
        .child(sidebar_drop_divider(
            active,
            &format!("append-{}", project_id.0),
        ))
}

/// Key used in `ExpandedItems` for a project folder.
pub fn project_expand_key(project_id: &ProjectId) -> String {
    format!("project-{}", project_id.0)
}

pub fn project_show_all_key(project_id: &ProjectId) -> String {
    format!("project-show-all-{}", project_id.0)
}

fn menu_open_handler(
    sidebar: Entity<SidebarView>,
    menu_key: String,
) -> Rc<dyn Fn(bool, &mut gpui::App)> {
    Rc::new(move |open, app| {
        sidebar.update(app, |view, cx| {
            view.set_open_action_menu(if open { Some(menu_key.clone()) } else { None }, cx);
        });
    })
}

fn session_menu_items(
    conv_id: ConversationId,
    title: &str,
    entity: Entity<AgentWindow>,
    sidebar: Entity<SidebarView>,
) -> Rc<dyn Fn() -> Vec<SidebarRowMenuItem>> {
    let delete_entity = entity;
    let delete_sidebar = sidebar;
    let delete_id = conv_id;
    let delete_title = title.to_string();
    Rc::new(move || {
        let delete_entity = delete_entity.clone();
        let delete_sidebar = delete_sidebar.clone();
        let delete_id = delete_id.clone();
        let delete_title = delete_title.clone();
        vec![SidebarRowMenuItem {
            label: "Delete chat".into(),
            icon: icons::DELETE,
            destructive: true,
            action: Rc::new(move |window, app| {
                delete_sidebar.update(app, |view, cx| view.close_action_menu(cx));
                delete_entity.update(app, |view, cx| {
                    view.confirm_delete_conversation(
                        delete_id.clone(),
                        delete_title.clone(),
                        window,
                        cx,
                    );
                });
            }),
        }]
    })
}

fn project_menu_items(
    project_id: ProjectId,
    project_name: &str,
    session_count: usize,
    entity: Entity<AgentWindow>,
    sidebar: Entity<SidebarView>,
) -> Rc<dyn Fn() -> Vec<SidebarRowMenuItem>> {
    let new_entity = entity.clone();
    let new_sidebar = sidebar.clone();
    let new_project_id = project_id.clone();
    let delete_entity = entity;
    let delete_sidebar = sidebar;
    let delete_id = project_id;
    let delete_name = project_name.to_string();
    Rc::new(move || {
        let new_entity = new_entity.clone();
        let new_sidebar = new_sidebar.clone();
        let new_project_id = new_project_id.clone();
        let delete_entity = delete_entity.clone();
        let delete_sidebar = delete_sidebar.clone();
        let delete_id = delete_id.clone();
        let delete_name = delete_name.clone();
        vec![
            SidebarRowMenuItem {
                label: "Start chat".into(),
                icon: icons::PLUS,
                destructive: false,
                action: Rc::new(move |_window, app| {
                    new_sidebar.update(app, |view, cx| view.close_action_menu(cx));
                    new_entity.update(app, |view, cx| {
                        view.new_conversation_in_project(new_project_id.clone(), cx);
                    });
                }),
            },
            SidebarRowMenuItem {
                label: "Remove project".into(),
                icon: icons::DELETE,
                destructive: true,
                action: Rc::new(move |window, app| {
                    delete_sidebar.update(app, |view, cx| view.close_action_menu(cx));
                    delete_entity.update(app, |view, cx| {
                        view.confirm_delete_project(
                            delete_id.clone(),
                            delete_name.clone(),
                            session_count,
                            window,
                            cx,
                        );
                    });
                }),
            },
        ]
    })
}

/// Section header label (e.g. "PROJECTS").
pub fn section_label(text: &str, first: bool) -> impl IntoElement {
    crate::shared::components::section_label::sidebar_section_label(text, first)
}

/// A flat, clickable session row in the sidebar tree.
pub fn session_row(
    row: SessionRowViewModel,
    drop_target: &Option<SidebarDropTarget>,
    open_action_menu: &Option<String>,
    entity: Entity<AgentWindow>,
    sidebar: Entity<SidebarView>,
) -> impl IntoElement {
    let drag_id = row.conv_id.clone();
    let drag_title = row.title.clone();
    let click_id = row.conv_id.clone();
    let drop_target_id = row.conv_id.clone();
    let hover_target_id = row.conv_id.clone();
    let entity_drop = entity.clone();
    let entity_sidebar = sidebar.clone();
    let entity_hover = sidebar.clone();
    let entity_click = entity.clone();
    let menu_items = session_menu_items(
        row.conv_id.clone(),
        &row.title,
        entity.clone(),
        sidebar.clone(),
    );
    let is_menu_open = open_action_menu.as_deref() == Some(row.menu_key.as_str());
    let on_menu_open_change = menu_open_handler(sidebar.clone(), row.menu_key.clone());
    let menu_key_for_right = row.menu_key.clone();
    let sidebar_for_right = sidebar.clone();
    let group_name = row.group_name.clone();
    let title = row.title.clone();
    let updated_at = row.updated_at.clone();
    let conv_id = row.conv_id.clone();
    let selected = row.selected;
    let hover_group = group_name.clone();
    let title_opacity = session_title_opacity(row.indent_level, selected);
    let meta_opacity = session_meta_opacity(row.indent_level, is_menu_open);
    let cascade_index = row.cascade_index;

    let row_body = div()
        .id(row.row_id.clone())
        .relative()
        .w_full()
        .min_w(px(0.0))
        .h(px(Tokens::ROW_HEIGHT_MD))
        .pl(Tokens::tree_indent(row.indent_level + 1))
        .pr(Tokens::spacing_2())
        .overflow_hidden()
        .rounded(Tokens::radius_xs())
        .flex()
        .items_center()
        .group(group_name.clone())
        .cursor_pointer()
        .when(selected, |el| el.bg(sidebar_session_selected_bg()))
        .when(!selected, |el| {
            el.hover(|s| s.bg(Tokens::sidebar_hover_bg()))
        })
        .on_click(move |_, _, app: &mut gpui::App| {
            entity_click.update(app, |view, cx| {
                view.select_conversation(click_id.clone(), cx);
            });
        })
        .on_mouse_up(MouseButton::Right, move |_, _, app: &mut gpui::App| {
            sidebar_for_right.update(app, |view, cx| {
                view.set_open_action_menu(Some(menu_key_for_right.clone()), cx);
            });
        })
        .on_drag(
            DragSession {
                conversation_id: drag_id,
                title: drag_title,
            },
            |drag, _, _, cx| cx.new(|_| drag.clone()),
        )
        .drag_over::<DragSession>(move |style, drag, _, app: &mut gpui::App| {
            if drag.conversation_id != hover_target_id {
                let sidebar = entity_hover.clone();
                let target = SidebarDropTarget::BeforeSession(hover_target_id.clone());
                app.defer(move |app| {
                    sidebar.update(app, |view, cx| {
                        view.set_drop_target(Some(target), cx);
                    });
                });
            }
            style
        })
        .on_drop(move |drag: &DragSession, _, app: &mut gpui::App| {
            if drag.conversation_id == drop_target_id {
                return;
            }
            entity_drop.update(app, |view, cx| {
                view.reposition_conversation(
                    drag.conversation_id.clone(),
                    drop_target_id.clone(),
                    cx,
                );
            });
            entity_sidebar.update(app, |view, cx| {
                view.clear_drop_target(cx);
            });
        })
        .child(sidebar_row_wash(selected, &row.menu_key, 0.0))
        .child(sidebar_row_beam(selected, &row.menu_key))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .flex()
                .items_center()
                .child(sidebar_text_cascade_in(
                    div()
                        .id(row.title_id.clone())
                        .flex_1()
                        .min_w(px(0.0))
                        .max_w_full()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_size(Tokens::text_sm())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(sidebar_row_label_color(selected))
                        .opacity(title_opacity)
                        .when(!selected, |el| {
                            el.group_hover(hover_group, |s| {
                                s.text_color(Tokens::sidebar_text_hover())
                            })
                        })
                        .child(title),
                    element_key("session-title-cascade", &row.conv_id.0),
                    cascade_index,
                )),
        )
        .child(
            div()
                .relative()
                .w(px(SESSION_META_WIDTH))
                .h_full()
                .flex_shrink_0()
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .left_0()
                        .flex()
                        .items_center()
                        .justify_end()
                        .child(sidebar_text_cascade_in(
                            div()
                                .w_full()
                                .min_w(px(0.0))
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .text_size(Tokens::text_xs())
                                .text_color(Tokens::text_primary())
                                .text_align(TextAlign::Right)
                                .opacity(meta_opacity)
                                .when(!is_menu_open, |el| {
                                    el.group_hover(group_name.clone(), |s| s.opacity(0.0))
                                })
                                .child(updated_at),
                            element_key("session-meta-cascade", &row.conv_id.0),
                            cascade_index.saturating_add(1),
                        )),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .flex()
                        .items_center()
                        .justify_end()
                        .child(sidebar_overflow_menu(
                            row.overflow_id.clone(),
                            group_name.clone(),
                            menu_items,
                            is_menu_open,
                            on_menu_open_change,
                        )),
                ),
        );

    div()
        .id(row.wrap_id.clone())
        .w_full()
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .child(drop_before_session(&conv_id, drop_target))
        .child(row_body)
}

/// Disclosure row for a project folder.
pub fn project_row(
    row: ProjectRowViewModel,
    open_action_menu: &Option<String>,
    entity: Entity<AgentWindow>,
    sidebar: Entity<SidebarView>,
) -> impl IntoElement {
    let project_id = row.project_id.clone();
    let name = row.name.clone();
    let count = row.count;
    let entity_toggle = entity.clone();
    let entity_new = entity.clone();
    let new_project_id = row.project_id.clone();
    let hover_project_id = row.project_id.clone();
    let entity_hover = sidebar.clone();
    let drop_project_id = row.project_id.clone();
    let entity_drop = entity.clone();
    let entity_sidebar = sidebar.clone();
    let menu_items = project_menu_items(
        row.project_id.clone(),
        &name,
        count,
        entity.clone(),
        sidebar.clone(),
    );
    let is_menu_open = open_action_menu.as_deref() == Some(row.menu_key.as_str());
    let on_menu_open_change = menu_open_handler(sidebar.clone(), row.menu_key.clone());
    let menu_key_for_right = row.menu_key.clone();
    let sidebar_for_right = sidebar.clone();
    let group_name = row.group_name.clone();
    let hover_group = group_name.clone();
    let cascade_index = row.cascade_index;

    div()
        .id(row.row_id.clone())
        .relative()
        .w_full()
        .min_w(px(0.0))
        .h(px(Tokens::ROW_HEIGHT_MD))
        .pl(Tokens::spacing_1())
        .pr(Tokens::spacing_2())
        .overflow_hidden()
        .rounded(Tokens::radius_xs())
        .flex()
        .items_center()
        .gap(Tokens::spacing_1p5())
        .group(group_name.clone())
        .cursor_pointer()
        .when(row.expanded, |el| {
            el.bg(Tokens::sidebar_hover_bg().opacity(0.52))
        })
        .when(!row.expanded, |el| {
            el.hover(|s| s.bg(Tokens::sidebar_hover_bg().opacity(0.72)))
        })
        .when(row.expanded, |el| {
            el.hover(|s| s.bg(Tokens::sidebar_hover_bg().opacity(0.82)))
        })
        .on_mouse_up(MouseButton::Right, move |_, _, app: &mut gpui::App| {
            sidebar_for_right.update(app, |view, cx| {
                view.set_open_action_menu(Some(menu_key_for_right.clone()), cx);
            });
        })
        .drag_over::<DragSession>(move |style, _, _, app: &mut gpui::App| {
            let sidebar = entity_hover.clone();
            let target = SidebarDropTarget::AppendToProject(hover_project_id.clone());
            app.defer(move |app| {
                sidebar.update(app, |view, cx| {
                    view.set_drop_target(Some(target), cx);
                });
            });
            style
        })
        .on_drop(move |drag: &DragSession, _, app: &mut gpui::App| {
            entity_drop.update(app, |view, cx| {
                view.move_conversation_to_project(
                    drag.conversation_id.clone(),
                    drop_project_id.clone(),
                    cx,
                );
            });
            entity_sidebar.update(app, |view, cx| {
                view.clear_drop_target(cx);
            });
        })
        .child(
            div()
                .id(row.toggle_id.clone())
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .flex()
                .items_center()
                .gap(Tokens::spacing_1p5())
                .cursor_pointer()
                .on_click(move |_, _, app: &mut gpui::App| {
                    entity_toggle.update(app, |view, cx| {
                        view.toggle_project(project_id.clone(), cx);
                    });
                })
                .child(
                    Icon::new(icons::FOLDER)
                        .size(px(14.0))
                        .flex_shrink_0()
                        .text_color(Tokens::sidebar_text())
                        .opacity(0.66),
                )
                .child(sidebar_text_fade_in(
                    div()
                        .id(row.name_id.clone())
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(Tokens::text_sm())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Tokens::text_primary())
                        .opacity(0.72)
                        .group_hover(hover_group, |s| s.text_color(Tokens::sidebar_text_hover()))
                        .child(name),
                    element_key("project-name-cascade", &row.project_id.0),
                    cascade_index,
                )),
        )
        .child(
            div()
                .relative()
                .w(px(PROJECT_META_WIDTH))
                .h_full()
                .flex_shrink_0()
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .left_0()
                        .flex()
                        .items_center()
                        .justify_end()
                        .child(sidebar_text_fade_in(
                            div()
                                .w_full()
                                .min_w(px(0.0))
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .text_size(Tokens::text_xs())
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Tokens::sidebar_text_muted())
                                .text_align(TextAlign::Right)
                                .opacity(if is_menu_open { 0.0 } else { 0.62 })
                                .when(!is_menu_open, |el| {
                                    el.group_hover(group_name.clone(), |s| s.opacity(0.0))
                                })
                                .child(count.to_string()),
                            element_key("project-meta-cascade", &row.project_id.0),
                            cascade_index.saturating_add(1),
                        )),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(Tokens::spacing_0p5())
                        .opacity(if is_menu_open { 1.0 } else { 0.0 })
                        .when(!is_menu_open, |el| {
                            el.group_hover(group_name.clone(), |s| s.opacity(1.0))
                        })
                        .child(btn_icon_sm(row.new_conv_id.clone(), icons::PLUS).on_click(
                            move |_, _, app: &mut gpui::App| {
                                entity_new.update(app, |view, cx| {
                                    view.new_conversation_in_project(new_project_id.clone(), cx);
                                });
                            },
                        ))
                        .child(sidebar_overflow_menu(
                            row.overflow_id.clone(),
                            group_name.clone(),
                            menu_items,
                            is_menu_open,
                            on_menu_open_change,
                        )),
                ),
        )
}

pub fn project_show_more_row(
    project_id: ProjectId,
    remaining: usize,
    entity: Entity<AgentWindow>,
) -> impl IntoElement {
    div()
        .id(element_key("project-show-more", &project_id.0))
        .w_full()
        .h(px(Tokens::ROW_HEIGHT_MD))
        .pl(Tokens::tree_indent(2))
        .pr(Tokens::spacing_2())
        .rounded(Tokens::radius_sm())
        .flex()
        .items_center()
        .cursor_pointer()
        .text_size(Tokens::text_sm())
        .text_color(Tokens::sidebar_text_muted())
        .opacity(0.64)
        .hover(|s| s.bg(Tokens::sidebar_hover_bg().opacity(0.55)))
        .on_click(move |_, _, app: &mut gpui::App| {
            entity.update(app, |view, cx| {
                view.toggle_project_show_all(project_id.clone(), cx);
            });
        })
        .child(if remaining == 1 {
            "Show 1 more".to_string()
        } else {
            format!("Show {remaining} more")
        })
}

/// Drop zone appended after a project's sessions (insert at end).
pub fn project_append_drop_zone(
    project_id: &ProjectId,
    drop_target: &Option<SidebarDropTarget>,
    entity: Entity<AgentWindow>,
    sidebar: Entity<SidebarView>,
) -> impl IntoElement {
    let target_project_id = project_id.clone();
    let hover_project_id = project_id.clone();
    let entity_drop = entity;
    let entity_hover = sidebar.clone();
    let entity_sidebar = sidebar;

    div()
        .id(element_key("project-append", &project_id.0))
        .min_w(px(0.0))
        .min_h(px(6.0))
        .drag_over::<DragSession>(move |style, _, _, app: &mut gpui::App| {
            let sidebar = entity_hover.clone();
            let target = SidebarDropTarget::AppendToProject(hover_project_id.clone());
            app.defer(move |app| {
                sidebar.update(app, |view, cx| {
                    view.set_drop_target(Some(target), cx);
                });
            });
            style
        })
        .on_drop(move |drag: &DragSession, _, app: &mut gpui::App| {
            entity_drop.update(app, |view, cx| {
                view.move_conversation_to_project(
                    drag.conversation_id.clone(),
                    target_project_id.clone(),
                    cx,
                );
            });
            entity_sidebar.update(app, |view, cx| {
                view.clear_drop_target(cx);
            });
        })
        .child(drop_append_project(project_id, drop_target, 1))
}

/// Muted placeholder row (e.g. empty scheduled section).
#[allow(dead_code)]
pub fn empty_state_row(icon: gpui_component::IconName, text: &str) -> impl IntoElement {
    let text = text.to_string();
    div()
        .h(px(Tokens::ROW_HEIGHT_MD))
        .px(Tokens::spacing_2())
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .child(
            Icon::new(icon)
                .size(px(14.0))
                .text_color(Tokens::sidebar_text()),
        )
        .child(
            div()
                .text_size(Tokens::text_sm())
                .text_color(Tokens::sidebar_text())
                .child(text),
        )
}

/// Sessions not assigned to any project.
pub fn unassigned_sessions<'a>(
    projects: &'a [Project],
    sessions: &'a [SidebarSession],
) -> Vec<&'a SidebarSession> {
    let assigned: HashSet<_> = projects
        .iter()
        .flat_map(|p| p.conversations.iter())
        .collect();
    sessions
        .iter()
        .filter(|session| !assigned.contains(&session.id))
        .collect()
}

/// Filter sidebar sessions by a case-insensitive query on title.
pub fn filter_sidebar_sessions(sessions: &[SidebarSession], query: &str) -> Vec<SidebarSession> {
    let query = query.trim();
    if query.is_empty() {
        return sessions.to_vec();
    }
    let q = query.to_lowercase();
    sessions
        .iter()
        .filter(|session| session.title.to_lowercase().contains(&q))
        .cloned()
        .collect()
}

/// Filter projects — keep projects whose name matches or contain a matching session.
pub fn filter_projects(
    projects: &[Project],
    sessions: &[SidebarSession],
    query: &str,
) -> Vec<Project> {
    let query = query.trim();
    if query.is_empty() {
        return projects.to_vec();
    }
    let q = query.to_lowercase();
    let matching_session_ids: HashSet<_> = sessions
        .iter()
        .filter(|session| session.title.to_lowercase().contains(&q))
        .map(|session| session.id.clone())
        .collect();

    projects
        .iter()
        .filter(|p| {
            p.name.to_lowercase().contains(&q)
                || p.conversations
                    .iter()
                    .any(|id| matching_session_ids.contains(id))
        })
        .cloned()
        .collect()
}

/// When searching, auto-expand projects that have visible matches.
pub fn expanded_for_search(
    projects: &[Project],
    sessions: &[SidebarSession],
    query: &str,
    base: &HashSet<String>,
) -> HashSet<String> {
    let query = query.trim();
    if query.is_empty() {
        return base.clone();
    }

    let q = query.to_lowercase();
    let session_titles: HashMap<ConversationId, &str> = sessions
        .iter()
        .map(|session| (session.id.clone(), session.title.as_str()))
        .collect();
    let mut expanded = base.clone();
    for project in projects {
        let has_match = project.name.to_lowercase().contains(&q)
            || project.conversations.iter().any(|cid| {
                session_titles
                    .get(cid)
                    .is_some_and(|title| title.to_lowercase().contains(&q))
            });
        if has_match {
            expanded.insert(project_expand_key(&project.id));
        }
    }
    expanded
}
