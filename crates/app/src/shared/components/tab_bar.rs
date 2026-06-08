//! Reusable tab bar component — used by terminal bottom panel and diff panel file tabs.
//!
//! Flat tabs at rest; the selected tab gets a stronger text treatment plus an
//! underline. Unselected tabs stay quiet and only pick up a hover wash.

use std::rc::Rc;

use crate::features::shell::state::DockPlacement;
use crate::features::workspace_layout::state::WorkspaceItemId;
use crate::shared::components::buttons::btn_icon_sm;
use crate::tokens::icons;
use crate::tokens::{Tokens, element_key};
use gpui::{Context, FontWeight, IntoElement, Render, SharedString, Window, div, prelude::*, px};
use gpui_component::Icon;
use gpui_component::IconName;

#[derive(Clone)]
pub(crate) struct DraggedTab {
    pub group_id: SharedString,
    pub kind: DraggedTabKind,
    pub tab_id: u64,
    pub workspace_item: WorkspaceItemId,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DraggedTabKind {
    InspectorDock(DockPlacement),
    TerminalSession,
}

pub(crate) fn dock_tab_drag_placement(drag: &DraggedTab) -> Option<DockPlacement> {
    match drag.kind {
        DraggedTabKind::InspectorDock(dock) => Some(dock),
        DraggedTabKind::TerminalSession => None,
    }
}

pub(crate) fn workspace_item_for_drag_kind(kind: &DraggedTabKind, tab_id: u64) -> WorkspaceItemId {
    match kind {
        DraggedTabKind::InspectorDock(_) => WorkspaceItemId::inspector_tab(tab_id),
        DraggedTabKind::TerminalSession => WorkspaceItemId::terminal_session(tab_id),
    }
}

impl Render for DraggedTab {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("dragged-tab")
            .cursor_grab()
            .h(px(Tokens::ROW_HEIGHT_SM))
            .px(Tokens::spacing_2())
            .rounded(Tokens::radius_xs())
            .bg(Tokens::surface_active())
            .opacity(0.9)
            .flex()
            .items_center()
            .child(
                div()
                    .max_w(px(180.0))
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_primary())
                    .child(self.label.clone()),
            )
    }
}

/// A single tab item.
pub struct TabItem {
    pub id: u64,
    pub label: String,
    pub icon: Option<IconName>,
}

/// Props for the tab bar.
pub struct TabBarProps {
    pub id: &'static str,
    pub tabs: Vec<TabItem>,
    pub selected_id: u64,

    pub on_select: Option<Rc<dyn Fn(u64, &mut gpui::App)>>,
    pub on_close: Option<Rc<dyn Fn(u64, &mut gpui::App)>>,
    pub on_add: Option<Rc<dyn Fn(&mut gpui::App)>>,
    pub on_reorder: Option<Rc<dyn Fn(u64, u64, &mut gpui::App)>>,
    pub drag_kind: Option<DraggedTabKind>,
}

/// Render a tab bar.
pub fn tab_bar(props: TabBarProps) -> impl IntoElement {
    let id = props.id;
    let selected_id = props.selected_id;
    let on_select = props.on_select;
    let on_close = props.on_close;
    let on_add = props.on_add;
    let on_reorder = props.on_reorder;
    let drag_kind = props.drag_kind;

    div()
        .id(element_key(id, "tab-bar"))
        .w_full()
        .min_w(px(0.0))
        .overflow_x_scroll()
        .child(
            div()
                .flex()
                .items_center()
                .gap(Tokens::spacing_0p5())
                .py(Tokens::spacing_1())
                .min_w(px(0.0))
                .flex_nowrap()
                .children(props.tabs.iter().map(|tab| {
                    render_tab(
                        id,
                        tab,
                        selected_id,
                        on_select.clone(),
                        on_close.clone(),
                        on_reorder.clone(),
                        drag_kind.clone(),
                    )
                }))
                .when_some(on_add, |el, cb| {
                    el.child(
                        btn_icon_sm(element_key(id, "add-tab"), icons::PLUS)
                            .on_click(move |_, _, app| cb(app)),
                    )
                }),
        )
}

fn render_tab(
    bar_id: &'static str,
    tab: &TabItem,
    selected_id: u64,
    on_select: Option<Rc<dyn Fn(u64, &mut gpui::App)>>,
    on_close: Option<Rc<dyn Fn(u64, &mut gpui::App)>>,
    on_reorder: Option<Rc<dyn Fn(u64, u64, &mut gpui::App)>>,
    drag_kind: Option<DraggedTabKind>,
) -> impl IntoElement {
    let tab_id = tab.id;
    let is_selected = tab_id == selected_id;
    let label = tab.label.clone();
    let icon = tab.icon.clone();
    let hover_bar_id = SharedString::from(bar_id);

    let mut tab_div = div()
        .id(element_key(bar_id, &tab_id.to_string()))
        .h(px(Tokens::ROW_HEIGHT_SM))
        .px(Tokens::spacing_2())
        .max_w(px(180.0))
        .border_b_1()
        .border_color(if is_selected {
            Tokens::accent().opacity(0.92)
        } else {
            Tokens::surface_hover().alpha(0.0)
        })
        .flex()
        .items_center()
        .gap(Tokens::spacing_1())
        .flex_shrink_0()
        .cursor_pointer()
        .when(is_selected, |el| el.text_color(Tokens::text_primary()))
        .when(!is_selected, |el| {
            el.rounded(Tokens::radius_xs())
                .hover(|s| s.bg(Tokens::surface_hover().opacity(0.6)))
        })
        .when_some(on_select, |el, cb| {
            el.on_click(move |_, _, app: &mut gpui::App| cb(tab_id, app))
        })
        .when_some(drag_kind.clone(), |el, kind| {
            let workspace_item = workspace_item_for_drag_kind(&kind, tab_id);
            let drag_payload = DraggedTab {
                group_id: SharedString::from(bar_id),
                kind,
                tab_id,
                workspace_item,
                label: label.clone(),
            };
            el.on_drag(drag_payload, |drag, _, _, cx| cx.new(|_| drag.clone()))
                .drag_over::<DraggedTab>(move |style, drag, _, _| {
                    if drag.group_id == hover_bar_id && drag.tab_id != tab_id {
                        style.bg(Tokens::surface_hover())
                    } else {
                        style
                    }
                })
        })
        .when(on_reorder.is_some() && drag_kind.is_some(), |el| {
            let on_reorder = on_reorder.clone();
            el.on_drop(move |drag: &DraggedTab, _, app: &mut gpui::App| {
                if drag.group_id.as_ref() == bar_id && drag.tab_id != tab_id {
                    if let Some(ref cb) = on_reorder {
                        cb(drag.tab_id, tab_id, app);
                    }
                }
            })
        });

    if let Some(icon) = icon {
        tab_div = tab_div.child(Icon::new(icon).size(px(13.0)).text_color(if is_selected {
            Tokens::text_primary()
        } else {
            Tokens::text_tertiary()
        }));
    }

    tab_div = tab_div.child(
        div()
            .min_w(px(0.0))
            .overflow_hidden()
            .text_ellipsis()
            .text_size(Tokens::text_xs())
            .font_weight(if is_selected {
                FontWeight::MEDIUM
            } else {
                FontWeight::NORMAL
            })
            .text_color(if is_selected {
                Tokens::text_primary()
            } else {
                Tokens::text_secondary()
            })
            .child(label),
    );

    if on_close.is_some() {
        tab_div = tab_div.child(
            btn_icon_sm(
                element_key(bar_id, &format!("close-{tab_id}")),
                icons::X_MARK,
            )
            .on_click({
                let on_close = on_close.clone();
                move |_, _, app: &mut gpui::App| {
                    if let Some(ref cb) = on_close {
                        cb(tab_id, app);
                    }
                }
            }),
        );
    }

    tab_div
}
