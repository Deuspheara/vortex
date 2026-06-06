//! Mode chip row — compact action mode selector under the composer pill.
//!
//! Renders a row of lightweight chips: Plan, Ask, Edit, Apply, Agent.
//! The selected chip is text-first: subtle background, clear text weight,
//! and red-accented Apply/Agent states without decorative icons.
//! Hover tooltips explain what each mode does.

use std::rc::Rc;

use agent_protocol::AgentMode;
use gpui::{FontWeight, Hsla, IntoElement, SharedString, div, prelude::*, px};
use gpui_component::tooltip::Tooltip;

use crate::tokens::Tokens;

/// A single mode option rendered as a chip.
struct ModeChip {
    mode: AgentMode,
    label: &'static str,
    tooltip_title: &'static str,
    tooltip_body: &'static str,
    /// Color used for the selected chip's bg, border, and text.
    selected_color: fn() -> Hsla,
}

/// Available mode chips in display order.
/// Each chip gets a distinct accent color when selected.
const MODE_CHIPS: &[ModeChip] = &[
    ModeChip {
        mode: AgentMode::PlanOnly,
        label: "Plan",
        tooltip_title: "Plan",
        tooltip_body: "Read-only planning — research the codebase and draft an implementation plan.",
        selected_color: Tokens::text_primary,
    },
    ModeChip {
        mode: AgentMode::ChatOnly,
        label: "Ask",
        tooltip_title: "Ask",
        tooltip_body: "Chat only — answers questions without tools, edits, or commands.",
        selected_color: Tokens::text_primary,
    },
    ModeChip {
        mode: AgentMode::SuggestPatch,
        label: "Edit",
        tooltip_title: "Edit",
        tooltip_body: "Suggest edits — propose diffs and previews without applying changes.",
        selected_color: Tokens::warning,
    },
    ModeChip {
        mode: AgentMode::ApplyWithApproval,
        label: "Apply",
        tooltip_title: "Apply with approval",
        tooltip_body: "Default mode — runs tools and applies patches after you approve them.",
        selected_color: Tokens::danger,
    },
    ModeChip {
        mode: AgentMode::FullAccessDangerous,
        label: "Agent",
        tooltip_title: "Agent",
        tooltip_body: "Full access — auto-runs tools and applies changes with minimal gates.",
        selected_color: Tokens::danger,
    },
];

pub struct ModeChipsProps {
    pub selected_mode: AgentMode,
    pub on_select: Rc<dyn Fn(AgentMode, &mut gpui::App) + 'static>,
}

/// Lightweight chip row for selecting the agent action mode.
/// Placed under the composer pill with 8px spacing.
/// Chips are 26px tall, pill-shaped (radius_full), with 6px between them.
pub fn mode_chips(props: ModeChipsProps) -> impl IntoElement {
    div().id("composer-mode-chips").child(
        div()
            .id("mode-chips-row")
            .flex()
            .items_center()
            .gap(Tokens::spacing_1p5())
            .children(MODE_CHIPS.iter().map(|chip| {
                let is_selected = props.selected_mode == chip.mode;
                let mode = chip.mode.clone();
                let on_select = props.on_select.clone();
                render_chip(chip, is_selected, mode, on_select)
            })),
    )
}

fn render_chip(
    chip: &ModeChip,
    is_selected: bool,
    mode: AgentMode,
    on_select: Rc<dyn Fn(AgentMode, &mut gpui::App) + 'static>,
) -> impl IntoElement {
    let accent = (chip.selected_color)();
    let bg = if is_selected {
        accent.opacity(if chip.label == "Apply" || chip.label == "Agent" {
            0.12
        } else {
            0.08
        })
    } else {
        Hsla::default()
    };
    let border = if is_selected {
        accent.opacity(if chip.label == "Apply" || chip.label == "Agent" {
            0.22
        } else {
            0.14
        })
    } else {
        Tokens::border_subtle()
    };
    let text_color = if is_selected {
        if chip.label == "Apply" || chip.label == "Agent" {
            Tokens::text_primary()
        } else {
            accent
        }
    } else {
        Tokens::text_tertiary()
    };

    div()
        .id(SharedString::from(format!(
            "mode-chip-{}",
            chip.label.to_lowercase()
        )))
        .h(px(Tokens::COMPOSER_MODE_CHIP_HEIGHT))
        .px(Tokens::spacing_2())
        .rounded(Tokens::radius_full())
        .bg(bg)
        .border_1()
        .border_color(border)
        .flex()
        .items_center()
        .cursor_pointer()
        .tooltip({
            let title = chip.tooltip_title.to_string();
            let body = chip.tooltip_body.to_string();
            move |window, cx| mode_chip_tooltip(&title, &body, window, cx)
        })
        .hover(|s| {
            if is_selected {
                s
            } else {
                s.bg(Tokens::surface_hover()).border_color(Tokens::border())
            }
        })
        .on_click(move |_, _, app: &mut gpui::App| {
            on_select(mode.clone(), app);
        })
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(if is_selected {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::MEDIUM
                })
                .text_color(text_color)
                .child(chip.label),
        )
}

const MODE_CHIP_TOOLTIP_WIDTH: f32 = 220.0;

fn mode_chip_tooltip(
    title: &str,
    body: &str,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) -> gpui::AnyView {
    let title = title.to_string();
    let body = body.to_string();
    let width = px(MODE_CHIP_TOOLTIP_WIDTH);

    Tooltip::element(move |_, _| {
        div()
            .id("mode-chip-tooltip")
            .w(width)
            .flex()
            .flex_col()
            .gap(Tokens::spacing_0p5())
            .overflow_hidden()
            .child(
                div()
                    .w_full()
                    .whitespace_normal()
                    .text_size(Tokens::text_sm())
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(Tokens::text_primary())
                    .child(title.clone()),
            )
            .child(
                div()
                    .w_full()
                    .whitespace_normal()
                    .text_size(Tokens::text_xs())
                    .line_height(Tokens::text_sm_leading())
                    .text_color(Tokens::text_secondary())
                    .child(body.clone()),
            )
    })
    .w(width)
    .build(window, cx)
}
