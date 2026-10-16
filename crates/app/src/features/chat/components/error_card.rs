//! Flat failure/status rows with one aligned text and action column.

use crate::tokens::{Tokens, element_key, icons};
use gpui::{ElementId, FontWeight, IntoElement, div, prelude::*, px};
use gpui_component::button::{Button, ButtonRounded, ButtonVariants};
use gpui_component::{Icon, Sizable, Size};

pub struct ErrorCardProps {
    pub id: String,
    pub title: String,
    pub message: String,
    pub retryable: bool,
    pub stopped: bool,
    pub on_open_settings: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    pub on_retry: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
}

// Button's default label typography is independent of the parent text style.
// Use an explicitly styled child so actions keep the transcript's 14 px scale.
fn error_action(id: ElementId, label: &'static str) -> Button {
    Button::new(id)
        .outline()
        .with_size(Size::Small)
        .h(px(Tokens::ROW_HEIGHT_MD))
        .px(Tokens::spacing_2())
        .rounded(ButtonRounded::Size(Tokens::radius_sm()))
        .bg(Tokens::surface())
        .border_color(Tokens::border_strong())
        .text_color(Tokens::text_primary())
        .cursor_pointer()
        .child(
            div()
                .font_family(Tokens::ui_font_family())
                .font_weight(FontWeight::MEDIUM)
                .text_color(Tokens::text_primary())
                .text_size(Tokens::text_sm())
                .line_height(Tokens::text_sm_leading())
                .child(label),
        )
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
        .py(Tokens::spacing_1())
        .flex()
        .items_start()
        .gap(Tokens::spacing_1p5())
        .font_family(Tokens::ui_font_family())
        .text_size(Tokens::text_sm())
        .line_height(Tokens::text_sm_leading())
        .child(
            div()
                .w(Tokens::spacing_6())
                .h(Tokens::text_sm_leading())
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
                .flex()
                .flex_col()
                .items_start()
                .gap(Tokens::spacing_1())
                .child(
                    div()
                        .w_full()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(tone)
                        .child(props.title),
                )
                .when(!props.message.is_empty(), |el| {
                    el.child(
                        div()
                            .w_full()
                            .text_color(Tokens::text_primary())
                            .child(props.message),
                    )
                })
                .when(has_actions, |el| {
                    el.child(
                        div()
                            .mt(Tokens::spacing_1())
                            .flex()
                            .items_center()
                            .gap(Tokens::spacing_2())
                            .when_some(props.on_open_settings, |el, cb| {
                                el.child(
                                    error_action(
                                        element_key("error-open-settings", &props.id),
                                        "Open settings",
                                    )
                                    .on_click(move |_, _, app: &mut gpui::App| cb(app)),
                                )
                            })
                            .when(props.retryable, |el| {
                                el.when_some(props.on_retry, |el, cb| {
                                    el.child(
                                        error_action(
                                            element_key("error-retry", &props.id),
                                            "Retry",
                                        )
                                        .on_click(move |_, _, app: &mut gpui::App| cb(app)),
                                    )
                                })
                            }),
                    )
                }),
        )
}
