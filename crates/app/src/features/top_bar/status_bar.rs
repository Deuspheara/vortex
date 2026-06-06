//! Status bar layout — bottom chrome bar.
//!
//! Stateless: accepts display data.

use gpui::{FontWeight, IntoElement, div, prelude::*, px};

use crate::features::shell::state::AgentStatus;
use crate::shared::components::status_indicator::status_indicator;
use crate::tokens::Tokens;

/// Props for the status bar.
#[allow(dead_code)]
pub struct StatusBarProps {
    pub model: String,
    pub token_usage: String,
    pub agent_status: Option<AgentStatus>,
}

/// Renders the bottom status bar.
#[allow(dead_code)]
pub fn render_status_bar(props: StatusBarProps) -> impl IntoElement {
    div()
        .id("status-bar")
        .h(px(Tokens::STATUS_BAR_HEIGHT))
        .w_full()
        .flex()
        .items_center()
        .px(Tokens::spacing_3())
        .gap(Tokens::spacing_3())
        .bg(Tokens::chrome())
        .border_t_1()
        .border_color(Tokens::divider())
        .child(
            div()
                .flex()
                .items_center()
                .gap(Tokens::spacing_1p5())
                .child(status_indicator(&props.agent_status))
                .child(
                    div()
                        .text_size(Tokens::text_status())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Tokens::text_secondary())
                        .child(props.model.clone()),
                ),
        )
        .child(
            div()
                .text_size(Tokens::text_status())
                .text_color(Tokens::text_tertiary())
                .child(format!("{} tokens", props.token_usage)),
        )
        .child(div().flex_1())
}
