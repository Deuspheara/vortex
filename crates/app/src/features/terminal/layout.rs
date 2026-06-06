//! Terminal content — project-scoped terminal surface with session tabs.

use gpui::{Entity, IntoElement, div, prelude::*, px};
use std::rc::Rc;

use crate::features::terminal::components::terminal_view::TerminalView;
use crate::shared::components::buttons::btn_icon_sm;
use crate::shared::components::tab_bar::{TabBarProps, TabItem, tab_bar};
use crate::tokens::Tokens;
use crate::tokens::icons;

#[derive(Clone)]
pub struct TerminalTabVm {
    pub id: u64,
    pub label: String,
    pub selected: bool,
}

pub struct TerminalPanelProps {
    pub tabs: Vec<TerminalTabVm>,
    pub terminal_view: Option<Entity<TerminalView>>,
    pub on_new_tab: Option<Rc<dyn Fn(&mut gpui::App)>>,
    pub on_close_tab: Option<Rc<dyn Fn(u64, &mut gpui::App)>>,
    pub on_select_tab: Option<Rc<dyn Fn(u64, &mut gpui::App)>>,
    pub on_reorder_tabs: Option<Rc<dyn Fn(u64, u64, &mut gpui::App)>>,
}

pub fn render_terminal_panel(props: TerminalPanelProps) -> impl IntoElement {
    div()
        .id("terminal-panel-content")
        .h_full()
        .w_full()
        .flex()
        .flex_col()
        .bg(Tokens::panel_bg())
        .overflow_hidden()
        .child(render_session_strip(&props))
        .child(
            div()
                .id("bottom-panel-body")
                .flex_1()
                .min_h(px(0.0))
                .overflow_hidden()
                .relative()
                .child(if let Some(terminal_view) = props.terminal_view {
                    div()
                        .id("bottom-panel-terminal")
                        .size_full()
                        .child(terminal_view)
                        .into_any_element()
                } else {
                    div()
                        .id("bottom-panel-terminal-placeholder")
                        .size_full()
                        .p(Tokens::spacing_3())
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(Tokens::text_sm())
                        .text_color(Tokens::text_secondary())
                        .child("Open a project to use the terminal.")
                        .into_any_element()
                }),
        )
        .into_any_element()
}

fn render_session_strip(props: &TerminalPanelProps) -> impl IntoElement {
    div()
        .id("terminal-panel-header")
        .flex()
        .border_b_1()
        .border_color(Tokens::border_subtle())
        .child(
            div()
                .h(px(Tokens::ROW_HEIGHT_MD))
                .px(Tokens::spacing_2())
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .child(div().flex_1().min_w(px(0.0)).child(render_tab_row(props)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(Tokens::spacing_1())
                        .when_some(props.on_new_tab.clone(), |el, cb| {
                            el.child(
                                btn_icon_sm("terminal-header-new-tab", icons::PLUS)
                                    .on_click(move |_, _, app| cb(app)),
                            )
                        }),
                ),
        )
}

fn render_tab_row(props: &TerminalPanelProps) -> impl IntoElement {
    let selected_id = props
        .tabs
        .iter()
        .find(|t| t.selected)
        .map(|t| t.id)
        .unwrap_or(0);

    let tabs: Vec<TabItem> = props
        .tabs
        .iter()
        .map(|t| TabItem {
            id: t.id,
            label: t.label.clone(),
            icon: None,
        })
        .collect();

    let on_close = if props.tabs.len() > 1 {
        props.on_close_tab.clone()
    } else {
        None
    };

    tab_bar(TabBarProps {
        id: "terminal",
        tabs,
        selected_id,
        on_select: props.on_select_tab.clone(),
        on_close,
        on_add: None,
        on_reorder: props.on_reorder_tabs.clone(),
        drag_kind: Some(crate::shared::components::tab_bar::DraggedTabKind::TerminalSession),
    })
}
