//! Compact segmented tab button for inspector and review surfaces.

use std::rc::Rc;

use gpui::{FontWeight, IntoElement, div, prelude::*, px};
use gpui_component::Icon;
use gpui_component::IconName;

use crate::tokens::Tokens;
use crate::tokens::element_key;

pub fn segmented_tab(
    id_prefix: &'static str,
    icon: IconName,
    label: &'static str,
    selected: bool,
    on_click: Option<Rc<dyn Fn(&mut gpui::App)>>,
) -> impl IntoElement {
    div()
        .id(element_key(id_prefix, label))
        .h(px(Tokens::ROW_HEIGHT_SM))
        .px(Tokens::spacing_2())
        .rounded(Tokens::radius_xs())
        .flex()
        .items_center()
        .gap(Tokens::spacing_1())
        .cursor_pointer()
        .when(selected, |s| s.bg(Tokens::surface_active()))
        .when(!selected, |s| s.hover(|h| h.bg(Tokens::surface_hover())))
        .child(Icon::new(icon).size(px(14.0)).text_color(if selected {
            Tokens::accent()
        } else {
            Tokens::text_tertiary()
        }))
        .child(
            div()
                .text_size(Tokens::text_sm())
                .font_weight(if selected {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if selected {
                    Tokens::text_primary()
                } else {
                    Tokens::text_secondary()
                })
                .child(label.to_string()),
        )
        .when_some(on_click, |el, cb| {
            el.on_click(move |_, _, app: &mut gpui::App| cb(app))
        })
}
