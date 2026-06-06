use gpui::{FontWeight, IntoElement, div, prelude::*, px};

use crate::features::shell::state::ChipKind;
use crate::features::shell::state::ContextChip;
use crate::tokens::Tokens;

/// Renders a subtle context pill aligned with the app surface palette.
pub fn context_chip(chip: &ContextChip) -> impl IntoElement {
    let label = chip.label.clone();
    let dot = chip_dot(&chip.kind);

    div()
        .h(px(Tokens::ROW_HEIGHT_XS))
        .px(Tokens::spacing_2())
        .rounded(Tokens::radius_full())
        .bg(Tokens::surface_hover())
        .border_1()
        .border_color(Tokens::border_subtle())
        .flex()
        .items_center()
        .gap(Tokens::spacing_1())
        .child(
            div()
                .w(px(5.0))
                .h(px(5.0))
                .rounded(Tokens::radius_full())
                .bg(dot),
        )
        .child(
            div()
                .text_size(Tokens::text_xs())
                .font_weight(FontWeight::MEDIUM)
                .text_color(Tokens::text_secondary())
                .child(label),
        )
}

fn chip_dot(kind: &ChipKind) -> gpui::Hsla {
    match kind {
        ChipKind::Repo => Tokens::text_tertiary(),
        ChipKind::File => Tokens::text_tertiary(),
        ChipKind::Branch => Tokens::accent(),
        ChipKind::Tool => Tokens::accent(),
    }
}
