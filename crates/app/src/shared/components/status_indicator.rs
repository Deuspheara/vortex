//! Stateless status indicator dot + label.

use gpui::{IntoElement, div, prelude::*, px};

use crate::features::shell::state::AgentStatus;
use crate::tokens::Tokens;

/// A coloured dot + text label for the agent status bar.
#[allow(dead_code)]
pub fn status_indicator(status: &Option<AgentStatus>) -> impl IntoElement {
    let (color, label) = match status {
        Some(AgentStatus::Idle) => (Tokens::text_tertiary(), "Idle"),
        Some(AgentStatus::Thinking) => (Tokens::accent(), "Thinking..."),
        Some(AgentStatus::RunningTool) => (Tokens::success(), "Running tool..."),
        Some(AgentStatus::WaitingApproval) => (Tokens::warning(), "Awaiting approval"),
        Some(AgentStatus::Completed) => (Tokens::success(), "Completed"),
        Some(AgentStatus::Failed) => (Tokens::danger(), "Failed"),
        None => (Tokens::text_tertiary(), ""),
    };

    div()
        .flex()
        .items_center()
        .gap(Tokens::spacing_1p5())
        .child(div().size(px(6.0)).rounded_full().bg(color))
        .child(
            div()
                .text_size(Tokens::text_status())
                .text_color(Tokens::text_secondary())
                .child(label),
        )
}
