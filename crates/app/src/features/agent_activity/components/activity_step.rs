use gpui::{FontWeight, IntoElement, div, prelude::*, px};
use gpui_component::Icon;

use crate::features::agent_activity::state::ActivityPhase;
use crate::features::shell::state::{SessionStep, StepStatus};
use crate::shared::components::step_icon::step_icon;
use crate::shared::state::ToolCatalog;
use crate::tokens::{Tokens, element_key};

#[allow(dead_code)]
pub fn phase_label(phase: &ActivityPhase) -> &'static str {
    match phase {
        ActivityPhase::Explore => "EXPLORE",
        ActivityPhase::Edit => "EDIT",
        ActivityPhase::Run => "RUN",
        ActivityPhase::Review => "REVIEW",
    }
}

#[allow(dead_code)]
pub fn activity_step(catalog: &ToolCatalog, step: &SessionStep) -> impl IntoElement {
    let (icon_name, icon_color) = step_icon(catalog, step);
    div()
        .id(element_key(
            "activity-step",
            &format!("{}-{}", step.item_ix, step.depth),
        ))
        .h(px(Tokens::ROW_HEIGHT_SM))
        .w_full()
        .pl(if step.depth > 0 {
            Tokens::spacing_3()
        } else {
            Tokens::spacing_0p5()
        })
        .pr(Tokens::spacing_1())
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .child(
            Icon::new(icon_name)
                .size(px(14.0))
                .text_color(icon_color)
                .flex_shrink_0(),
        )
        .child(
            div()
                .text_size(Tokens::text_sm())
                .font_weight(FontWeight::MEDIUM)
                .text_color(step_label_color(step))
                .truncate()
                .child(step.label.clone()),
        )
        .when_some(step.detail.clone(), |el, detail| {
            el.child(
                div()
                    .text_size(Tokens::text_xs())
                    .font_family("monospace")
                    .text_color(Tokens::tool_path_text())
                    .truncate()
                    .child(detail),
            )
        })
}

#[allow(dead_code)]
fn step_label_color(step: &SessionStep) -> gpui::Hsla {
    match step.status {
        StepStatus::Failed => Tokens::danger(),
        StepStatus::Running => Tokens::accent(),
        StepStatus::Done => match step.phase {
            ActivityPhase::Edit => Tokens::warning(),
            ActivityPhase::Review => Tokens::success(),
            _ => Tokens::text_secondary(),
        },
    }
}
