//! Top bar layout — conversation title, overflow menu, and dock actions.
//!
//! Stateless: accepts data and callback closures.

use std::rc::Rc;

use gpui::{
    AnyElement, Corner, Entity, FontWeight, IntoElement, SharedString, div, prelude::*, px,
};
use gpui_component::Icon;
use gpui_component::IconName;
use gpui_component::popover::Popover;

use crate::shared::components::buttons::{btn_ghost_icon, btn_icon_sm, btn_outline};
use crate::shared::components::dropdown::{
    DropdownAnchor, DropdownItem, PickerDropdownProps, picker_dropdown,
};
use crate::shared::components::panel_controls::{PanelControlClusterProps, panel_control_cluster};
use crate::shared::state::WorkspaceReadiness;
use crate::shared::state::model_catalog::DEFAULT_PROVIDER;
use crate::shared::state::model_catalog::provider_options;
use crate::tokens::Tokens;
use crate::tokens::icons;
use crate::ui::agent_window::AgentWindow;

pub struct TopBarProps {
    pub conversation_title: String,
    pub sidebar_collapsed: bool,
    pub inspector_open: bool,
    pub terminal_panel_open: bool,
    pub show_panel_controls: bool,
    pub active_theme: String,
    pub themes: Vec<SharedString>,
    pub selected_provider: String,
    pub dark_mode: bool,
    pub workspace_readiness: WorkspaceReadiness,
    pub workspace_readiness_label: Option<String>,
    pub entity: Entity<AgentWindow>,
    pub on_toggle_sidebar: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    pub on_toggle_inspector: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    pub on_toggle_terminal: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    pub on_new_chat: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    pub on_title_copy: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    pub on_new_terminal_tab: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    pub on_workspace_status: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    pub on_open_settings: Option<Rc<dyn Fn(&mut gpui::App)>>,
}

pub fn render_top_bar(props: TopBarProps) -> impl IntoElement {
    let title = props.conversation_title.clone();
    div()
        .id("top-bar")
        .h(px(Tokens::TOP_BAR_HEIGHT))
        .w_full()
        .flex()
        .items_center()
        .bg(Tokens::chrome())
        .border_b_1()
        .border_color(Tokens::divider())
        .pl(px(Tokens::TOP_BAR_TRAFFIC_LIGHT_INSET))
        .pr(Tokens::spacing_3())
        .gap(Tokens::spacing_2())
        .when(props.show_panel_controls, |el| {
            el.child(panel_control_cluster(PanelControlClusterProps {
                sidebar_collapsed: props.sidebar_collapsed,
                inspector_open: props.inspector_open,
                terminal_open: props.terminal_panel_open,
                on_toggle_sidebar: props.on_toggle_sidebar,
                on_toggle_inspector: props.on_toggle_inspector,
                on_toggle_terminal: props.on_toggle_terminal,
            }))
        })
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .px(Tokens::spacing_2())
                .child(render_title_block(title))
                .child(render_title_menu(
                    props.conversation_title,
                    props.on_new_chat,
                    props.on_title_copy,
                    props.on_open_settings.clone(),
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .flex_shrink_0()
                .child(render_dock_actions(props.on_new_terminal_tab))
                .when_some(props.workspace_readiness_label, |el, label| {
                    el.child(render_workspace_status(label, props.on_workspace_status))
                })
                .child(render_provider_status(
                    &props.selected_provider,
                    &props.workspace_readiness,
                    props.on_open_settings.clone(),
                ))
                .child(render_provider_picker(
                    &props.selected_provider,
                    props.entity.clone(),
                ))
                .child(render_theme_picker(
                    props.active_theme,
                    props.themes,
                    props.dark_mode,
                    props.entity.clone(),
                )),
        )
}

fn render_title_block(title: String) -> impl IntoElement {
    div().flex_1().min_w(px(0.0)).flex().items_center().child(
        div()
            .text_size(Tokens::text_md())
            .font_weight(FontWeight::MEDIUM)
            .text_color(Tokens::topbar_title_active())
            .overflow_hidden()
            .text_ellipsis()
            .child(title),
    )
}

fn render_title_menu(
    title: String,
    on_new_chat: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    on_title_copy: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    on_open_settings: Option<Rc<dyn Fn(&mut gpui::App)>>,
) -> impl IntoElement {
    let mut items = vec![
        TopBarMenuItem {
            label: "Copy title".into(),
            icon: icons::COPY,
            enabled: on_title_copy.is_some(),
            action: on_title_copy.map(Rc::from),
        },
        TopBarMenuItem {
            label: "Start new chat".into(),
            icon: icons::PLUS,
            enabled: on_new_chat.is_some(),
            action: on_new_chat.map(Rc::from),
        },
    ];
    if let Some(action) = on_open_settings {
        items.push(TopBarMenuItem {
            label: "Open settings".into(),
            icon: icons::SETTINGS,
            enabled: true,
            action: Some(action),
        });
    }

    Popover::new("topbar-title-menu")
        .anchor(Corner::BottomRight)
        .appearance(false)
        .overlay_closable(true)
        .trigger(
            btn_icon_sm("topbar-title-menu-trigger", icons::MORE_HORIZONTAL)
                .tooltip(format!("Conversation actions for {title}")),
        )
        .content(move |_, _, _| top_bar_menu_panel(items.clone()))
}

#[derive(Clone)]
struct TopBarMenuItem {
    label: String,
    icon: IconName,
    enabled: bool,
    action: Option<Rc<dyn Fn(&mut gpui::App)>>,
}

fn top_bar_menu_panel(items: Vec<TopBarMenuItem>) -> impl IntoElement {
    div()
        .w(px(200.0))
        .rounded(Tokens::radius_lg())
        .bg(Tokens::surface_overlay())
        .border_1()
        .border_color(Tokens::border())
        .shadow_md()
        .p(Tokens::spacing_1p5())
        .flex()
        .flex_col()
        .gap(Tokens::spacing_0p5())
        .children(items.into_iter().enumerate().map(|(index, item)| {
            let action = item.action.clone();
            let enabled = item.enabled;
            div()
                .id(("topbar-menu-item", index))
                .h(px(Tokens::ROW_HEIGHT_MD))
                .px(Tokens::spacing_2())
                .rounded(Tokens::radius_sm())
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .when(enabled, |el| {
                    el.cursor_pointer()
                        .hover(|s| s.bg(Tokens::surface_hover()))
                        .on_click(move |_, _, app: &mut gpui::App| {
                            if let Some(action) = &action {
                                action(app);
                            }
                        })
                })
                .opacity(if enabled { 1.0 } else { 0.45 })
                .child(
                    Icon::new(item.icon)
                        .size(px(14.0))
                        .text_color(Tokens::text_tertiary()),
                )
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .text_color(Tokens::text_secondary())
                        .child(item.label),
                )
                .into_any_element()
        }))
}

fn render_workspace_status(
    label: String,
    on_click: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
) -> impl IntoElement {
    let button = btn_outline("topbar-workspace-readiness", label);
    if let Some(cb) = on_click {
        button.on_click(move |_, _, app: &mut gpui::App| cb(app))
    } else {
        button
    }
}

fn render_provider_status(
    selected: &str,
    readiness: &WorkspaceReadiness,
    on_click: Option<Rc<dyn Fn(&mut gpui::App)>>,
) -> impl IntoElement {
    let label = if readiness.provider_connected {
        format!(
            "{} ready",
            if selected.is_empty() {
                DEFAULT_PROVIDER
            } else {
                selected
            }
        )
    } else if readiness.uses_mock_provider {
        "Connect provider".to_string()
    } else {
        "Finish provider setup".to_string()
    };

    let button = btn_outline("topbar-provider-status", label);
    if let Some(cb) = on_click {
        button.on_click(move |_, _, app: &mut gpui::App| cb(app))
    } else {
        button
    }
}

fn render_dock_actions(
    on_new_terminal_tab: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
) -> impl IntoElement {
    div().flex().items_center().gap(Tokens::spacing_1()).child(
        btn_ghost_icon("topbar-new-terminal-tab", icons::TERMINAL)
            .tooltip("New terminal tab")
            .when_some(on_new_terminal_tab, |button, cb| {
                button.on_click(move |_, _, app: &mut gpui::App| cb(app))
            }),
    )
}

fn render_provider_picker(selected: &str, entity: Entity<AgentWindow>) -> AnyElement {
    let selected = if selected.is_empty() {
        DEFAULT_PROVIDER
    } else {
        selected
    };
    let items: Vec<DropdownItem> = provider_options()
        .into_iter()
        .map(|opt| DropdownItem {
            label: opt.name.to_string(),
            icon: Some(opt.icon),
        })
        .collect();

    let current_icon = provider_options()
        .into_iter()
        .find(|opt| opt.name == selected)
        .map(|opt| opt.icon);

    picker_dropdown(PickerDropdownProps {
        id: "topbar-provider".into(),
        label: selected.to_string(),
        items,
        selected: Some(selected.to_string()),
        anchor: DropdownAnchor::Below,
        menu_min_width: 140.0,
        trigger_icon: current_icon,
        searchable: false,
        search_texts: None,
        search_placeholder: None,
        on_select: Rc::new(move |_index, selected, app| {
            entity.update(app, |view, cx| {
                view.on_provider_selected(selected, cx);
            });
        }),
    })
    .into_any_element()
}

fn render_theme_picker(
    active_theme: String,
    themes: Vec<SharedString>,
    dark_mode: bool,
    entity: Entity<AgentWindow>,
) -> AnyElement {
    let theme_icon = if dark_mode { icons::MOON } else { icons::SUN };
    let items: Vec<DropdownItem> = themes
        .into_iter()
        .map(|name| DropdownItem {
            label: name.to_string(),
            icon: None,
        })
        .collect();

    picker_dropdown(PickerDropdownProps {
        id: "theme-picker".into(),
        label: active_theme.clone(),
        items,
        selected: Some(active_theme),
        anchor: DropdownAnchor::Below,
        menu_min_width: 200.0,
        trigger_icon: Some(theme_icon),
        searchable: false,
        search_texts: None,
        search_placeholder: None,
        on_select: Rc::new(move |_, theme, app| {
            entity.update(app, |view, cx| {
                view.apply_color_theme(&theme, None, cx);
            });
        }),
    })
    .into_any_element()
}
