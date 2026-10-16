//! Sidebar layout helpers — chrome and footer.
//!
//! Tree rendering lives in `sidebar_view` (isolated entity).

use gpui::{Entity, FontWeight, IntoElement, div, prelude::*, px};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{Icon, Sizable, Size};

use crate::features::shell::layout::SidebarView;
use crate::features::shell::state::AppNavItem;
use crate::shared::components::buttons::btn_ghost_icon;
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
    _selected: AppNavItem,
    search_input: Entity<InputState>,
) -> impl IntoElement {
    let focus_search = search_input.clone();
    div()
        .px(Tokens::sidebar_padding())
        .pt(Tokens::spacing_2())
        .pb(Tokens::spacing_4())
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .child(
            div()
                .id("sidebar-search-field")
                .capture_any_mouse_down(move |event, window, app| {
                    if event.button == gpui::MouseButton::Left {
                        focus_search.update(app, |input, cx| input.focus(window, cx));
                    }
                })
                .flex_1()
                .min_w(px(0.0))
                .h(px(Tokens::ROW_HEIGHT_LG))
                .px(Tokens::spacing_2())
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .bg(Tokens::search_bg())
                .border_1()
                .border_color(Tokens::search_border())
                .rounded(Tokens::radius_md())
                .hover(|s| s.border_color(Tokens::text_tertiary()))
                .child(
                    Icon::new(icons::SEARCH)
                        .size(Tokens::spacing_4())
                        .text_color(Tokens::text_secondary()),
                )
                .child(
                    Input::new(&search_input)
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(Tokens::text_sm())
                        .with_size(Size::Small)
                        .appearance(false)
                        .cleanable(true),
                ),
        )
        .child(
            Button::new("sidebar-new-chat")
                .icon(icons::new_thread())
                .h(px(Tokens::ROW_HEIGHT_LG))
                .w(px(Tokens::ROW_HEIGHT_LG))
                .rounded(Tokens::radius_sm())
                .ghost()
                .compact()
                .tooltip("New chat")
                .on_click(move |_, _, app| {
                    entity.update(app, |view, cx| view.new_conversation_from_nav(cx));
                }),
        )
}

pub(crate) fn render_projects_section_header(
    _first: bool,
    entity: Entity<AgentWindow>,
) -> impl IntoElement {
    div()
        .w_full()
        .h(px(Tokens::ROW_HEIGHT_LG))
        .px(Tokens::spacing_2())
        .flex()
        .items_center()
        .justify_between()
        .gap(Tokens::spacing_2())
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .text_size(Tokens::text_xs())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(Tokens::sidebar_text_muted())
                .child("PROJECTS"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(Tokens::spacing_1())
                .child(
                    btn_ghost_icon("filter-projects", icons::SEARCH)
                        .tooltip("Search workspace")
                        .on_click({
                            let entity = entity.clone();
                            move |_, _, app: &mut gpui::App| {
                                entity.update(app, |view, cx| view.open_search(cx));
                            }
                        }),
                )
                .child(
                    btn_ghost_icon("open-project-from-projects", icons::PLUS)
                        .tooltip("Open project")
                        .h(px(Tokens::ROW_HEIGHT_SM))
                        .w(px(Tokens::ROW_HEIGHT_SM))
                        .on_click(move |_, window, app: &mut gpui::App| {
                            entity.update(app, |view, cx| {
                                view.open_project_folder(window, cx);
                            });
                        }),
                ),
        )
}

pub(crate) fn render_sidebar_footer(
    entity: Entity<AgentWindow>,
    selected: bool,
) -> impl IntoElement {
    let entity_click = entity.clone();
    let extensions_entity = entity.clone();
    let automations_entity = entity;
    div()
        .id("sidebar-settings")
        .flex()
        .items_center()
        .flex_shrink_0()
        .bg(Tokens::sidebar_bg())
        .px(Tokens::spacing_2())
        .py(Tokens::spacing_2())
        .child(
            div().flex_1().min_w(px(0.0)).child(flat_list_row(
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
                            .size(Tokens::spacing_4())
                            .text_color(Tokens::sidebar_text_muted()),
                    )
                    .child(
                        div()
                            .text_size(Tokens::text_sm())
                            .text_color(Tokens::sidebar_text())
                            .child("Settings"),
                    ),
            )),
        )
        .child(
            btn_ghost_icon("sidebar-extensions", icons::APP_WINDOW)
                .tooltip("Extensions")
                .on_click(move |_, _, app| {
                    extensions_entity.update(app, |view, cx| view.open_extensions(cx))
                }),
        )
        .child(
            btn_ghost_icon("sidebar-automations", icons::HISTORY)
                .tooltip("Automations")
                .on_click(move |_, _, app| {
                    automations_entity.update(app, |view, cx| view.open_automations(cx))
                }),
        )
}
