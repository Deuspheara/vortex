//! Shared flat list row — AGENTS.md hover/selected recipe.

use gpui::{App, ElementId, IntoElement, Pixels, div, prelude::*, px};

use crate::tokens::Tokens;

/// Flat navigational row: optional indent, hover, and selection.
pub fn flat_list_row(
    id: impl Into<ElementId>,
    height: f32,
    pl: Pixels,
    pr: Pixels,
    selected: bool,
    sidebar_hover: bool,
    on_click: Option<impl Fn(&mut App) + 'static>,
    child: impl IntoElement,
) -> impl IntoElement {
    let hover = if sidebar_hover {
        Tokens::sidebar_hover_bg()
    } else {
        Tokens::surface_hover()
    };

    div()
        .id(id)
        .w_full()
        .min_w(px(0.0))
        .h(px(height))
        .pl(pl)
        .pr(pr)
        .overflow_hidden()
        .rounded(Tokens::radius_xs())
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .when(on_click.is_some(), |el| el.cursor_pointer())
        .when(selected, |el| el.bg(Tokens::surface_active()))
        .when(!selected && on_click.is_some(), |el| {
            el.hover(|s| s.bg(hover))
        })
        .when(!selected && on_click.is_none(), |el| {
            el.hover(|s| s.bg(hover))
        })
        .when_some(on_click, |el, on_click| {
            el.on_click(move |_, _, app: &mut App| on_click(app))
        })
        .child(child)
}
