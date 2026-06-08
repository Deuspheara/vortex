//! Section headers for sidebar, settings, and similar lists.

use gpui::{FontWeight, IntoElement, div, prelude::*, px};

use crate::tokens::Tokens;

/// Sidebar-style section label (e.g. "PROJECTS", "RECENT").
pub fn sidebar_section_label(title: &str, first: bool) -> impl IntoElement {
    let title = title.to_string();
    div()
        .pt(if first {
            Tokens::spacing_2()
        } else {
            Tokens::spacing_4()
        })
        .pb(Tokens::spacing_1())
        .child(
            div()
                .h(px(Tokens::ROW_HEIGHT_SM))
                .flex()
                .items_center()
                .text_size(Tokens::text_xs())
                .font_weight(FontWeight::MEDIUM)
                .text_color(Tokens::sidebar_text_muted())
                .opacity(0.82)
                .child(title),
        )
}

/// Sidebar section label with a trailing action (e.g. "+" for projects).
pub fn sidebar_section_label_with_action(
    title: &str,
    first: bool,
    trailing: impl IntoElement,
) -> impl IntoElement {
    let title = title.to_string();
    div()
        .pt(if first {
            Tokens::spacing_3()
        } else {
            Tokens::spacing_4()
        })
        .pb(Tokens::spacing_1())
        .child(
            div()
                .w_full()
                .h(px(Tokens::ROW_HEIGHT_SM))
                .flex()
                .items_center()
                .justify_between()
                .gap(Tokens::spacing_2())
                .overflow_hidden()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(Tokens::text_xs())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Tokens::sidebar_text_muted())
                        .opacity(0.74)
                        .child(title),
                )
                .child(trailing),
        )
}

/// Settings panel section label (accent bar + title).
pub fn settings_section_label(title: &str) -> impl IntoElement {
    let title = title.to_string();
    div()
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .child(
            div()
                .w(px(3.0))
                .h(px(14.0))
                .rounded(Tokens::radius_full())
                .bg(Tokens::accent()),
        )
        .child(
            div()
                .text_size(Tokens::text_xs())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(Tokens::accent())
                .child(title),
        )
}
