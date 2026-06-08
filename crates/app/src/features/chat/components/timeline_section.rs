//! Flat phase section header for the agent timeline.

use gpui::{FontWeight, IntoElement, div, prelude::*, px};

use crate::features::agent_activity::state::ActivityPhase;
use crate::features::chat::state::phase_label;
use crate::tokens::Tokens;

pub fn timeline_section_header(phase: ActivityPhase) -> impl IntoElement {
    let label = phase_label(phase);
    div()
        .id("timeline-section-header")
        .w_full()
        .max_w(px(Tokens::THREAD_MAX_WIDTH))
        .pt(Tokens::spacing_2())
        .pb(Tokens::spacing_1())
        .child(
            div()
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .child(div().w(px(20.0)).h(px(1.0)).bg(Tokens::timeline_border()))
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Tokens::text_tertiary())
                        .child(label.to_uppercase()),
                )
                .child(div().flex_1().h(px(1.0)).bg(Tokens::timeline_border())),
        )
}
