//! Sidebar row action menu — hover ⋯ trigger + right-click popover.

use std::rc::Rc;

use gpui::{Corner, ElementId, FontWeight, IntoElement, SharedString, div, prelude::*, px};
use gpui_component::Icon;
use gpui_component::IconName;
use gpui_component::popover::Popover;

use crate::shared::components::buttons::btn_icon_sm;
use crate::tokens::icons;
use crate::tokens::{Tokens, element_key};

#[derive(Clone)]
pub struct SidebarRowMenuItem {
    pub label: String,
    pub icon: IconName,
    pub destructive: bool,
    pub action: Rc<dyn Fn(&mut gpui::Window, &mut gpui::App)>,
}

fn sidebar_menu_panel(items: Vec<SidebarRowMenuItem>) -> impl IntoElement {
    div()
        .w(px(188.0))
        .rounded(Tokens::radius_lg())
        .bg(Tokens::surface_overlay())
        .border_1()
        .border_color(Tokens::border())
        .shadow_md()
        .p(Tokens::spacing_1())
        .flex()
        .flex_col()
        .gap(px(2.0))
        .children(items.into_iter().enumerate().map(|(index, item)| {
            let label = item.label.clone();
            let action = item.action.clone();
            let icon = item.icon.clone();
            let destructive = item.destructive;

            let text_color = if destructive {
                Tokens::danger()
            } else {
                Tokens::text_secondary()
            };

            let icon_color = if destructive {
                Tokens::danger()
            } else {
                Tokens::text_tertiary()
            };

            div()
                .id(element_key("sidebar-menu-item", &index.to_string()))
                .h(px(30.0))
                .px(Tokens::spacing_2())
                .rounded(Tokens::radius_sm())
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .cursor_pointer()
                .text_color(text_color)
                .hover(|s| {
                    s.bg(if destructive {
                        Tokens::surface_hover().blend(Tokens::danger().opacity(0.12))
                    } else {
                        Tokens::surface_hover()
                    })
                })
                .on_click(move |_, window, app: &mut gpui::App| {
                    action(window, app);
                })
                .child(Icon::new(icon).size(px(14.0)).text_color(icon_color))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_size(Tokens::text_sm())
                        .font_weight(FontWeight::NORMAL)
                        .child(label),
                )
                .into_any_element()
        }))
}

/// Popover menu anchored to the trailing ⋯ button.
///
/// The containing row should reserve a fixed trailing width for this trigger
/// rather than absolutely overlaying it on top of the title.
pub fn sidebar_overflow_menu(
    menu_id: impl Into<ElementId>,
    group_name: impl Into<SharedString>,
    items: Rc<dyn Fn() -> Vec<SidebarRowMenuItem>>,
    is_open: bool,
    on_open_change: Rc<dyn Fn(bool, &mut gpui::App)>,
) -> impl IntoElement {
    let menu_id_str = menu_id.into();
    let group_name = group_name.into();
    let group_for_hover = group_name.clone();
    let group_for_trigger = group_name.clone();
    let items_for_content = items.clone();

    Popover::new(menu_id_str)
        .anchor(Corner::TopRight)
        .appearance(false)
        // Avoid same-click dismiss via overlay; gpui-component dropdowns use false here.
        .overlay_closable(false)
        .open(is_open)
        .on_open_change(move |open: &bool, _window, app| {
            on_open_change(*open, app);
        })
        .trigger(
            btn_icon_sm(
                element_key("sidebar-overflow", group_for_trigger.as_ref()),
                icons::MORE_HORIZONTAL,
            )
            .w(px(28.0))
            .h(px(22.0))
            .rounded(Tokens::radius_sm())
            .opacity(if is_open { 1.0 } else { 0.0 })
            .text_color(if is_open {
                Tokens::text_secondary()
            } else {
                Tokens::text_tertiary()
            })
            .when(is_open, |el| {
                el.bg(Tokens::surface_hover())
                    .text_color(Tokens::text_primary())
            })
            .when(!is_open, |el| {
                el.group_hover(group_for_hover, |s| s.opacity(1.0))
            }),
        )
        .content(move |_, _, _| sidebar_menu_panel(items_for_content()))
}
