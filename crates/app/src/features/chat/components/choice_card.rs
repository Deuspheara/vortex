use gpui::{FontWeight, IntoElement, div, prelude::*};
use gpui_component::Icon;

use crate::features::shell::state::{ChoiceMeta, ChoiceOption};
use crate::shared::components::buttons::btn_deny;
use crate::tokens::{Tokens, element_key, icons};

pub fn choice_card(
    id: &str,
    prompt: &str,
    options: &[ChoiceOption],
    meta: &ChoiceMeta,
    selected: Option<&str>,
    resolved: bool,
    on_select: impl Fn(String, &mut gpui::App) + Clone + 'static,
    on_cancel: impl Fn(&mut gpui::App) + Clone + 'static,
) -> impl IntoElement {
    let cancel = on_cancel.clone();
    div()
        .id(element_key("choice-card", id))
        .w_full()
        .px(Tokens::spacing_2p5())
        .py(Tokens::spacing_2())
        .rounded(Tokens::radius_md())
        .bg(Tokens::surface())
        .border_1()
        .border_color(Tokens::border_subtle())
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .child(
            div()
                .flex()
                .items_start()
                .gap(Tokens::spacing_2())
                .child(Icon::new(icons::QUESTION).size(Tokens::text_sm()))
                .child(
                    div()
                        .flex_1()
                        .min_w(gpui::px(0.0))
                        .flex()
                        .flex_col()
                        .gap(Tokens::spacing_0p5())
                        .child(
                            div()
                                .text_size(Tokens::text_xs())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Tokens::text_tertiary())
                                .child(
                                    meta.summary
                                        .clone()
                                        .unwrap_or_else(|| "Decision needed".into()),
                                ),
                        )
                        .child(
                            div()
                                .text_size(Tokens::text_sm())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Tokens::text_primary())
                                .child(prompt.to_string()),
                        )
                        .when_some(meta.blocking_reason.clone(), |el, reason| {
                            el.child(
                                div()
                                    .text_size(Tokens::text_xs())
                                    .text_color(Tokens::text_faint())
                                    .child(reason),
                            )
                        }),
                )
                .when(!resolved, |el| {
                    el.child(
                        btn_deny(element_key("choice-cancel", id), "Cancel run").on_click(
                            move |_, _, app: &mut gpui::App| {
                                cancel(app);
                            },
                        ),
                    )
                }),
        )
        .children(options.iter().map(|option| {
            let option_id = option.id.clone();
            let is_selected = selected == Some(option_id.as_str());
            let recommended = option.recommended
                || meta
                    .recommended_option_id
                    .as_ref()
                    .is_some_and(|id| id == &option.id);
            let cb = on_select.clone();
            div()
                .id(element_key("choice-option", &format!("{id}-{}", option.id)))
                .w_full()
                .px(Tokens::spacing_2())
                .py(Tokens::spacing_1())
                .rounded(Tokens::radius_sm())
                .border_1()
                .border_color(if is_selected {
                    Tokens::accent()
                } else {
                    Tokens::border_subtle()
                })
                .when(!resolved, |el| {
                    el.cursor_pointer().hover(|s| s.bg(Tokens::surface_hover()))
                })
                .when(is_selected, |el| el.bg(Tokens::surface_active()))
                .on_click(move |_, _, app: &mut gpui::App| {
                    if !resolved {
                        cb(option_id.clone(), app);
                    }
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(Tokens::spacing_2())
                        .child(
                            div()
                                .flex_1()
                                .min_w(gpui::px(0.0))
                                .text_size(Tokens::text_sm())
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Tokens::text_primary())
                                .child(option.label.clone()),
                        )
                        .when(recommended, |el| {
                            el.child(
                                div()
                                    .px(Tokens::spacing_1p5())
                                    .py(Tokens::spacing_0p5())
                                    .rounded(Tokens::radius_full())
                                    .bg(Tokens::accent().opacity(0.12))
                                    .text_size(Tokens::text_xs())
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Tokens::accent())
                                    .child("Recommended"),
                            )
                        }),
                )
                .when_some(option.description.clone(), |el, description| {
                    el.child(
                        div()
                            .pt(Tokens::spacing_0p5())
                            .text_size(Tokens::text_xs())
                            .text_color(Tokens::text_faint())
                            .child(description),
                    )
                })
                .into_any_element()
        }))
        .when(meta.allow_custom, |el| {
            el.child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_faint())
                    .child("Custom responses can be sent from the composer."),
            )
        })
}
