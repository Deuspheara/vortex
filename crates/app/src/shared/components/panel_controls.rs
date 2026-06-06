//! Codex-style layout panel controls — sidebar, inspector, bottom terminal.

use gpui::{IntoElement, div, prelude::*, px};
use gpui_component::IconName;

use crate::shared::components::buttons::btn_ghost_icon;
use crate::tokens::Tokens;
use crate::tokens::icons;

pub struct PanelControlClusterProps {
    pub sidebar_collapsed: bool,
    pub inspector_open: bool,
    pub terminal_open: bool,
    pub on_toggle_sidebar: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    pub on_toggle_inspector: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    pub on_toggle_terminal: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
}

pub fn panel_control_cluster(props: PanelControlClusterProps) -> impl IntoElement {
    let sidebar_icon = if props.sidebar_collapsed {
        icons::PANEL_LEFT
    } else {
        icons::PANEL_LEFT_CLOSE
    };
    let right_icon = if props.inspector_open {
        icons::PANEL_RIGHT_CLOSE
    } else {
        icons::PANEL_RIGHT
    };
    let bottom_icon = if props.terminal_open {
        icons::PANEL_BOTTOM_OPEN
    } else {
        icons::PANEL_BOTTOM
    };

    div()
        .id("panel-control-cluster")
        .flex()
        .items_center()
        .rounded(Tokens::radius_sm())
        .border_1()
        .border_color(Tokens::border_subtle())
        .bg(Tokens::surface())
        .child(panel_toggle(
            "toggle-sidebar",
            sidebar_icon,
            !props.sidebar_collapsed,
            props.on_toggle_sidebar,
        ))
        .child(panel_divider())
        .child(panel_toggle(
            "toggle-inspector",
            right_icon,
            props.inspector_open,
            props.on_toggle_inspector,
        ))
        .child(panel_divider())
        .child(panel_toggle(
            "toggle-terminal",
            bottom_icon,
            props.terminal_open,
            props.on_toggle_terminal,
        ))
}

fn panel_divider() -> impl IntoElement {
    div()
        .w(px(1.0))
        .h(px(Tokens::ROW_HEIGHT_SM))
        .bg(Tokens::divider())
}

fn panel_toggle(
    id: &'static str,
    icon: IconName,
    active: bool,
    on_click: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
) -> impl IntoElement {
    let mut button = btn_ghost_icon(id, icon)
        .h(px(Tokens::ROW_HEIGHT_SM))
        .w(px(Tokens::ROW_HEIGHT_LG));

    if active {
        button = button.bg(Tokens::surface_active());
    }

    if let Some(cb) = on_click {
        button.on_click(move |_, _, app: &mut gpui::App| cb(app))
    } else {
        button
    }
}
