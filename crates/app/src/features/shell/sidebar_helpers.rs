//! Sidebar layout helpers — chrome and footer.
//!
//! Tree rendering lives in `sidebar_view` (isolated entity).

use gpui::{Entity, IntoElement, div, prelude::*, px};
use gpui_component::Icon;

use crate::features::shell::layout::SidebarView;
use crate::features::shell::state::AppNavItem;
use crate::shared::components::buttons::btn_icon_sm;
use crate::shared::components::flat_list_row::flat_list_row;
use crate::tokens::Tokens;
use crate::tokens::icons;
use crate::ui::agent_window::AgentWindow;

/// Embeds the sidebar view entity in the agent window.
pub fn render_sidebar(sidebar_view: Entity<SidebarView>) -> impl IntoElement {
    sidebar_view
}

pub(crate) fn render_app_nav(
    entity: Entity<AgentWindow>,
    selected: AppNavItem,
) -> impl IntoElement {
    div()
        .px(Tokens::spacing_2())
        .pt(Tokens::spacing_2())
        .pb(Tokens::spacing_2())
        .flex()
        .flex_col()
        .gap(Tokens::spacing_0p5())
        .border_b_1()
        .border_color(Tokens::sidebar_border())
        .child(app_nav_row(
            "sidebar-new-chat",
            icons::PENCIL,
            "New chat",
            false,
            {
                let entity = entity.clone();
                move |app| {
                    entity.update(app, |view, cx| view.new_conversation_from_nav(cx));
                }
            },
        ))
        .child(app_nav_row(
            "sidebar-nav-chat",
            icons::MESSAGE_SQUARE,
            "Threads",
            selected == AppNavItem::Chat,
            {
                let entity = entity.clone();
                move |app| {
                    entity.update(app, |view, cx| view.open_chat(cx));
                }
            },
        ))
        .child(app_nav_row(
            "sidebar-nav-search",
            icons::SEARCH,
            "Search",
            selected == AppNavItem::Search,
            {
                let entity = entity.clone();
                move |app| {
                    entity.update(app, |view, cx| view.open_search(cx));
                }
            },
        ))
        .child(app_nav_row(
            "sidebar-nav-extensions",
            icons::APP_WINDOW,
            "Extensions",
            selected == AppNavItem::Extensions,
            {
                let entity = entity.clone();
                move |app| {
                    entity.update(app, |view, cx| view.open_extensions(cx));
                }
            },
        ))
        .child(app_nav_row(
            "sidebar-nav-automations",
            icons::HISTORY,
            "Automations",
            selected == AppNavItem::Automations,
            {
                let entity = entity.clone();
                move |app| {
                    entity.update(app, |view, cx| view.open_automations(cx));
                }
            },
        ))
}

pub(crate) fn render_projects_section_header(
    first: bool,
    entity: Entity<AgentWindow>,
) -> impl IntoElement {
    crate::shared::components::section_label::sidebar_section_label_with_action(
        "PROJECTS",
        first,
        btn_icon_sm("open-project-from-projects", icons::PLUS).on_click(
            move |_, window, app: &mut gpui::App| {
                entity.update(app, |view, cx| view.open_project_folder(window, cx));
            },
        ),
    )
}

pub(crate) fn render_sidebar_footer(
    entity: Entity<AgentWindow>,
    selected: bool,
) -> impl IntoElement {
    let entity_click = entity;
    div()
        .id("sidebar-settings")
        .flex_shrink_0()
        .border_t_1()
        .border_color(Tokens::sidebar_border())
        .bg(Tokens::sidebar_bg())
        .p(Tokens::spacing_2())
        .child(flat_list_row(
            "sidebar-settings-row",
            Tokens::ROW_HEIGHT_MD,
            Tokens::spacing_2(),
            Tokens::spacing_2(),
            selected,
            true,
            Some(move |app: &mut gpui::App| {
                entity_click.update(app, |view, cx| view.open_settings(cx));
            }),
            div()
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .child(
                    Icon::new(icons::SETTINGS)
                        .size(px(14.0))
                        .text_color(Tokens::sidebar_text()),
                )
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .text_color(Tokens::sidebar_text())
                        .child("Settings"),
                ),
        ))
}

fn app_nav_row(
    id: &'static str,
    icon: gpui_component::IconName,
    label: &'static str,
    selected: bool,
    on_click: impl Fn(&mut gpui::App) + 'static,
) -> impl IntoElement {
    flat_list_row(
        id,
        Tokens::ROW_HEIGHT_MD,
        Tokens::sidebar_padding(),
        Tokens::sidebar_padding(),
        selected,
        true,
        Some(on_click),
        div()
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
                    .child(label),
            ),
    )
}
