use crate::features::shell::state::{
    PlanArtifact, PlanProgressCounts, TODO_ROW_H, TodoEntry, TodoState,
};
use crate::shared::components::flat_list_row::flat_list_row;
use crate::shared::components::markdown_preview::markdown_preview;
use crate::tokens::{Tokens, element_key, icons};
use gpui::{AnyElement, FontWeight, IntoElement, div, prelude::*, px};
use gpui_component::Icon;

const TODO_CIRCLE_PX: f32 = 16.0;
const TODO_CHECK_ICON_PX: f32 = 10.0;

/// Compact sticky todo progress header (evolved todo strip).
pub fn todo_progress_header(
    items: &[TodoEntry],
    expanded: bool,
    on_toggle_strip: impl Fn(&mut gpui::App) + 'static,
) -> impl IntoElement {
    todo_strip(items, expanded, on_toggle_strip)
}

/// Compact sticky todo strip for the top of the chat column.
pub fn todo_strip(
    items: &[TodoEntry],
    expanded: bool,
    on_toggle_strip: impl Fn(&mut gpui::App) + 'static,
) -> impl IntoElement {
    let total = items.len();
    let (current_ix, current_label) = current_todo_focus(items);
    let summary = format!("{current_ix}/{total} todos · {current_label}");
    let toggle = on_toggle_strip;

    div()
        .id("todo-strip")
        .w_full()
        .flex_shrink_0()
        .px(Tokens::spacing_3())
        .py(Tokens::spacing_1())
        .border_b_1()
        .border_color(Tokens::border_subtle())
        .flex()
        .flex_col()
        .gap(Tokens::spacing_1())
        .child(
            div()
                .id("todo-strip-summary")
                .h(px(Tokens::ROW_HEIGHT_MD))
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .child(Icon::new(icons::CHECKLIST).size(Tokens::text_sm()))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(Tokens::text_sm())
                        .text_color(Tokens::text_secondary())
                        .overflow_hidden()
                        .child(summary),
                )
                .child(
                    div()
                        .id("todo-strip-toggle")
                        .flex_shrink_0()
                        .size(px(Tokens::ROW_HEIGHT_SM))
                        .rounded(Tokens::radius_xs())
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(Tokens::surface_hover()))
                        .on_click(move |_, _, app: &mut gpui::App| toggle(app))
                        .child(
                            Icon::new(if expanded {
                                icons::CHEVRON_DOWN
                            } else {
                                icons::CHEVRON_RIGHT
                            })
                            .size(Tokens::text_sm())
                            .text_color(Tokens::text_tertiary()),
                        ),
                ),
        )
        .when(expanded, |el| {
            el.children(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| todo_row(item, index))
                    .collect::<Vec<_>>(),
            )
        })
}

fn current_todo_focus(items: &[TodoEntry]) -> (usize, String) {
    let total = items.len();
    if total == 0 {
        return (0, "No tasks".into());
    }
    if let Some((ix, item)) = items
        .iter()
        .enumerate()
        .find(|(_, t)| matches!(t.state, TodoState::InProgress))
    {
        return (ix + 1, item.content.clone());
    }
    if let Some((ix, item)) = items
        .iter()
        .enumerate()
        .find(|(_, t)| matches!(t.state, TodoState::Pending))
    {
        return (ix + 1, item.content.clone());
    }
    let done = items
        .iter()
        .filter(|t| matches!(t.state, TodoState::Completed))
        .count();
    (
        done.min(total),
        items
            .last()
            .map(|t| t.content.clone())
            .unwrap_or_else(|| "All done".into()),
    )
}

/// Full live execution checklist.
#[allow(dead_code)]
pub fn todo_checklist(id: &str, items: &[TodoEntry]) -> impl IntoElement {
    let done = items
        .iter()
        .filter(|t| matches!(t.state, TodoState::Completed))
        .count();
    div()
        .id(element_key("todo-list", id))
        .w_full()
        .px(Tokens::spacing_2())
        .py(Tokens::spacing_1())
        .flex()
        .flex_col()
        .gap(Tokens::spacing_0p5())
        .child(
            div()
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .child(Icon::new(icons::CHECKLIST).size(Tokens::text_sm()))
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Tokens::text_tertiary())
                        .child(format!("TODO · {done}/{} done", items.len())),
                ),
        )
        .children(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| todo_row(item, index))
                .collect::<Vec<_>>(),
        )
}

/// Backward-compatible render entry for thread todo rows.
#[allow(dead_code)]
pub fn todo_list(id: &str, items: &[TodoEntry]) -> impl IntoElement {
    todo_checklist(id, items)
}

pub fn plan_artifact(artifact: PlanArtifact) -> impl IntoElement {
    let counts = plan_counts_from_markdown(&artifact.markdown);
    div()
        .id("plan-artifact")
        .w_full()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_3())
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(Tokens::spacing_2())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(Tokens::spacing_2())
                        .child(Icon::new(icons::CHECKLIST).size(Tokens::text_sm()))
                        .child(
                            div()
                                .text_size(Tokens::text_xs())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Tokens::text_tertiary())
                                .child(format!("PLAN · {}", artifact.execution_state.label())),
                        ),
                )
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_faint())
                        .child(artifact.created_at.clone()),
                ),
        )
        .child(
            div()
                .text_size(Tokens::text_xs())
                .text_color(Tokens::text_tertiary())
                .child(counts.summary()),
        )
        .when_some(artifact.source_conversation_id.as_ref(), |el, source| {
            el.child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_faint())
                    .child(format!("Source conversation: {}", source.0)),
            )
        })
        .when_some(artifact.started_at.as_ref(), |el, started_at| {
            el.child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_faint())
                    .child(match artifact.completed_at.as_ref() {
                        Some(completed_at) => {
                            format!("Started {started_at} · Completed {completed_at}")
                        }
                        None => format!("Started {started_at}"),
                    }),
            )
        })
        .child(markdown_preview(&artifact.markdown, true))
}

fn plan_counts_from_markdown(markdown: &str) -> PlanProgressCounts {
    let total = markdown
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("- [ ] ") || trimmed.starts_with("- ")
        })
        .count();
    PlanProgressCounts {
        pending: total,
        ..PlanProgressCounts::default()
    }
}

fn todo_row(item: &TodoEntry, _index: usize) -> AnyElement {
    let strike = matches!(item.state, TodoState::Completed | TodoState::Cancelled);
    let text_color = if strike {
        Tokens::text_faint()
    } else if matches!(item.state, TodoState::InProgress) {
        Tokens::text_primary()
    } else {
        Tokens::text_secondary()
    };
    flat_list_row(
        element_key("todo-row", &item.id),
        TODO_ROW_H,
        Tokens::spacing_1(),
        Tokens::spacing_1(),
        false,
        false,
        None::<fn(&mut gpui::App)>,
        div()
            .flex_1()
            .min_w(px(0.0))
            .flex()
            .items_center()
            .gap(Tokens::spacing_2())
            .child(todo_status_circle(item))
            .child(
                div()
                    .text_size(Tokens::text_sm())
                    .text_color(text_color)
                    .when(matches!(item.state, TodoState::InProgress), |el| {
                        el.font_weight(FontWeight::MEDIUM)
                    })
                    .when(strike, |el| el.line_through())
                    .child(item.content.clone()),
            ),
    )
    .into_any_element()
}

fn todo_status_circle(item: &TodoEntry) -> AnyElement {
    let size = px(TODO_CIRCLE_PX);
    match item.state {
        TodoState::Completed => div()
            .flex_shrink_0()
            .size(size)
            .rounded_full()
            .bg(Tokens::success())
            .flex()
            .items_center()
            .justify_center()
            .child(
                Icon::new(icons::CHECK)
                    .size(px(TODO_CHECK_ICON_PX))
                    .text_color(Tokens::main_bg()),
            )
            .into_any_element(),
        TodoState::InProgress => div()
            .flex_shrink_0()
            .size(size)
            .rounded_full()
            .border_1()
            .border_color(Tokens::accent())
            .into_any_element(),
        TodoState::Cancelled => div()
            .flex_shrink_0()
            .size(size)
            .rounded_full()
            .border_1()
            .border_color(Tokens::border_subtle())
            .opacity(0.55)
            .flex()
            .items_center()
            .justify_center()
            .child(
                Icon::new(icons::X_MARK)
                    .size(px(TODO_CHECK_ICON_PX))
                    .text_color(Tokens::text_faint()),
            )
            .into_any_element(),
        TodoState::Pending => div()
            .flex_shrink_0()
            .size(size)
            .rounded_full()
            .border_1()
            .border_color(Tokens::border_subtle())
            .into_any_element(),
    }
}
