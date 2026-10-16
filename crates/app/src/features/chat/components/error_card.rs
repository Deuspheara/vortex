//! Flat failure/status rows sharing the transcript activity alignment.

use crate::shared::components::buttons::{btn_ghost_label, btn_outline};
use crate::tokens::{Tokens, icons};
use gpui::{FontWeight, IntoElement, div, prelude::*, px};
use gpui_component::Icon;

pub struct ErrorCardProps {
    pub title: String,
    pub message: String,
    pub retryable: bool,
    pub stopped: bool,
    pub on_open_settings: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    pub on_retry: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
}

pub fn error_card(props: ErrorCardProps) -> impl IntoElement {
    let tone = if props.stopped {
        Tokens::text_secondary()
    } else {
        Tokens::danger()
    };
    let has_actions =
        props.on_open_settings.is_some() || (props.retryable && props.on_retry.is_some());
    let icon = if props.stopped {
        Icon::new(icons::X_MARK)
    } else {
        icons::circle_alert()
    };
    div()
        .w_full()
        .min_w(px(0.0))
        .px(Tokens::spacing_0p5())
        .py(Tokens::spacing_0p5())
        .flex()
        .flex_col()
        .text_size(Tokens::text_sm())
        .line_height(Tokens::text_md_leading())
        .child(
            div()
                .w_full()
                .min_h(px(Tokens::TOOL_ROW_HEIGHT))
                .flex()
                .items_start()
                .gap(Tokens::spacing_1p5())
                .child(
                    div()
                        .size(Tokens::spacing_6())
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon.size(Tokens::spacing_4()).text_color(tone)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(tone)
                        .child(props.title),
                ),
        )
        .when(!props.message.is_empty(), |el| {
            el.child(
                div()
                    .ml(Tokens::spacing_7())
                    .py(Tokens::spacing_1())
                    .text_color(Tokens::text_primary().opacity(0.8))
                    .child(props.message),
            )
        })
        .when(has_actions, |el| {
            el.child(
                div()
                    .ml(Tokens::spacing_7())
                    .pb(Tokens::spacing_1())
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
            )
        })
}
