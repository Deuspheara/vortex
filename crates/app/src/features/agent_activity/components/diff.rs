//! Diff summary rows in the thread timeline.

use gpui::{App, FontWeight, IntoElement, div, prelude::*};

use crate::features::shell::state::ActivityGroupPos;
use crate::shared::components::collapsible_row::{activity_group_wrap, timeline_row};
use crate::tokens::{Tokens, element_key};

pub fn render_diff_header_row(
    item_id: &str,
    files_changed: usize,
    additions: usize,
    deletions: usize,
    group_pos: Option<ActivityGroupPos>,
    on_toggle: impl Fn(&mut App) + 'static,
    on_review: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let stats = format!("+{additions} −{deletions}");
    let summary = format!(
        "Prepared changes in {files_changed} file{} · {stats}",
        if files_changed == 1 { "" } else { "s" }
    );

    activity_group_wrap(
        timeline_row(
            element_key("diff-header", item_id),
            div()
                .text_size(Tokens::text_sm())
                .font_weight(FontWeight::MEDIUM)
                .text_color(Tokens::text_secondary())
                .hover(|s| s.text_color(Tokens::text_primary()))
                .child(summary)
                .into_any_element(),
            div()
                .id(element_key("review", item_id))
                .flex_shrink_0()
                .cursor_pointer()
                .px(Tokens::spacing_1())
                .py(Tokens::spacing_0p5())
                .rounded(Tokens::radius_xs())
                .text_size(Tokens::text_xs())
                .text_color(Tokens::text_tertiary())
                .hover(|s| s.text_color(Tokens::text_primary()))
                .on_click(move |_, _, app: &mut App| on_review(app))
                .child("Open diff")
                .into_any_element(),
            move |_, _, app: &mut App| on_toggle(app),
        ),
        group_pos,
    )
}

pub fn render_diff_file_line_row(
    item_id: &str,
    path: &str,
    additions: usize,
    deletions: usize,
) -> impl IntoElement {
    div()
        .id(element_key("diff-file-line", item_id))
        .w_full()
        .pl(Tokens::spacing_1())
        .child(file_stat_line(path, additions, deletions))
}

fn file_stat_line(path: &str, additions: usize, deletions: usize) -> impl IntoElement {
    let path = path.to_string();
    div()
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .child(
            div()
                .text_size(Tokens::text_code())
                .font_family("monospace")
                .text_color(Tokens::text_faint())
                .child(path),
        )
        .when(additions > 0, |el| {
            el.child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_secondary())
                    .child(format!("+{additions}")),
            )
        })
        .when(deletions > 0, |el| {
            el.child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::danger())
                    .child(format!("−{deletions}")),
            )
        })
}
