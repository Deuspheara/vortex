//! Flat error row for run / provider failures.

use crate::shared::components::buttons::{btn_ghost_label, btn_outline};
use crate::tokens::{Tokens, icons};
use gpui::{FontWeight, IntoElement, div, prelude::*, px};
use gpui_component::Icon;

pub struct ErrorCardProps {
    pub title: String,
    pub message: String,
    pub retryable: bool,
    pub on_open_settings: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    pub on_retry: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
}

pub fn error_card(props: ErrorCardProps) -> impl IntoElement {
    div()
        .id("error-card")
        .w_full()
        .max_w(px(Tokens::THREAD_MAX_WIDTH))
        .py(Tokens::spacing_2())
        .child(
            div()
                .w_full()
                .border_l_2()
                .border_color(Tokens::warning())
                .pl(Tokens::spacing_2())
                .py(Tokens::spacing_1())
                .flex()
                .flex_col()
                .gap(Tokens::spacing_2())
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(Tokens::spacing_2())
                        .child(
                            Icon::new(icons::TRIANGLE_ALERT)
                                .size(Tokens::text_sm())
                                .text_color(Tokens::warning()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(Tokens::spacing_0p5())
                                .child(
                                    div()
                                        .text_size(Tokens::text_sm())
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(Tokens::text_primary())
                                        .child(props.title),
                                )
                                .child(
                                    div()
                                        .text_size(Tokens::text_xs())
                                        .text_color(Tokens::text_secondary())
                                        .child(props.message),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(Tokens::spacing_2())
                        .when_some(props.on_open_settings, |el, cb| {
                            el.child(
                                btn_ghost_label("error-open-settings", "Open settings")
                                    .on_click(move |_, _, app: &mut gpui::App| cb(app)),
                            )
                        })
                        .when(props.retryable, |el| {
                            el.when_some(props.on_retry, |el, cb| {
                                el.child(
                                    btn_outline("error-retry", "Retry")
                                        .on_click(move |_, _, app: &mut gpui::App| cb(app)),
                                )
                            })
                        }),
                ),
        )
}
