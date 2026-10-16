//! Context evidence rows in the thread timeline.

use gpui::{App, FontWeight, IntoElement, div, prelude::*, px};

use crate::features::agent_activity::components::{
    ActivityRowVisual, activity_header_row_with_visual,
};
use crate::features::shell::state::{ActivityGroupPos, ContextEntryKind, ContextTraceEntry};
use crate::tokens::{Tokens, element_key};

pub fn render_context_trace_header_row(
    item_id: &str,
    entries: &[ContextTraceEntry],
    expanded: bool,
    animate: bool,
    group_pos: Option<ActivityGroupPos>,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    activity_header_row_with_visual(
        ActivityRowVisual {
            row_key: "context-trace-row",
            header_key: "context-trace-header",
            item_id,
            running: false,
            show_loading: false,
            animate,
            group_pos,
        },
        "Used context".to_string(),
        Some(context_trace_summary(entries)),
        crate::shared::components::collapsible_row::disclosure_chevron(expanded).into_any_element(),
        on_toggle,
    )
}

pub fn render_context_trace_line_row(item_id: &str, entry: &ContextTraceEntry) -> impl IntoElement {
    div()
        .id(element_key("context-trace-line", item_id))
        .w_full()
        .h(px(Tokens::ROW_HEIGHT_MD))
        .pl(Tokens::spacing_7())
        .pr(Tokens::spacing_2())
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .child(
            div()
                .w_full()
                .min_w(px(0.0))
                .h(px(Tokens::ROW_HEIGHT_MD - 4.0))
                .pr(Tokens::spacing_2())
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .rounded(Tokens::radius_xs())
                .child(context_kind_badge(entry.kind))
                .child(
                    div()
                        .min_w(px(0.0))
                        .flex_1()
                        .overflow_hidden()
                        .truncate()
                        .text_size(Tokens::text_sm())
                        .line_height(Tokens::text_sm_leading_compact())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Tokens::text_secondary())
                        .child(entry.label.clone()),
                )
                .when_some(entry.detail.clone(), |el, detail| {
                    el.child(
                        div()
                            .max_w(px(220.0))
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .truncate()
                            .text_size(Tokens::text_xs())
                            .text_color(Tokens::text_tertiary())
                            .child(detail),
                    )
                })
                .child(
                    div()
                        .max_w(px(260.0))
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .truncate()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::activity_meta_text())
                        .child(entry.reason.clone()),
                ),
        )
}

pub fn context_trace_summary(entries: &[ContextTraceEntry]) -> String {
    let mut counts = [0usize; 6];
    for entry in entries {
        counts[kind_index(entry.kind)] += 1;
    }

    let parts: Vec<String> = [
        ContextEntryKind::RepoMap,
        ContextEntryKind::FileSlice,
        ContextEntryKind::Symbol,
        ContextEntryKind::Search,
        ContextEntryKind::Command,
        ContextEntryKind::Rule,
    ]
    .into_iter()
    .filter_map(|kind| {
        let count = counts[kind_index(kind)];
        (count > 0).then(|| format!("{count} {}", kind_noun(kind, count)))
    })
    .collect();

    if parts.is_empty() {
        "No evidence recorded".to_string()
    } else {
        parts.join(" · ")
    }
}

fn context_kind_badge(kind: ContextEntryKind) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .min_w(px(48.0))
        .text_size(Tokens::text_xs())
        .font_weight(FontWeight::MEDIUM)
        .text_color(Tokens::text_tertiary())
        .child(kind_badge_label(kind))
}

fn kind_index(kind: ContextEntryKind) -> usize {
    match kind {
        ContextEntryKind::RepoMap => 0,
        ContextEntryKind::FileSlice => 1,
        ContextEntryKind::Symbol => 2,
        ContextEntryKind::Search => 3,
        ContextEntryKind::Command => 4,
        ContextEntryKind::Rule => 5,
    }
}

fn kind_noun(kind: ContextEntryKind, count: usize) -> &'static str {
    match (kind, count == 1) {
        (ContextEntryKind::RepoMap, true) => "repo map",
        (ContextEntryKind::RepoMap, false) => "repo maps",
        (ContextEntryKind::FileSlice, true) => "file slice",
        (ContextEntryKind::FileSlice, false) => "file slices",
        (ContextEntryKind::Symbol, true) => "symbol",
        (ContextEntryKind::Symbol, false) => "symbols",
        (ContextEntryKind::Search, true) => "search",
        (ContextEntryKind::Search, false) => "searches",
        (ContextEntryKind::Command, true) => "command",
        (ContextEntryKind::Command, false) => "commands",
        (ContextEntryKind::Rule, true) => "rule",
        (ContextEntryKind::Rule, false) => "rules",
    }
}

fn kind_badge_label(kind: ContextEntryKind) -> &'static str {
    match kind {
        ContextEntryKind::RepoMap => "Map",
        ContextEntryKind::FileSlice => "File",
        ContextEntryKind::Symbol => "Symbol",
        ContextEntryKind::Search => "Search",
        ContextEntryKind::Command => "Command",
        ContextEntryKind::Rule => "Rule",
    }
}
