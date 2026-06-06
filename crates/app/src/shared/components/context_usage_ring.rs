//! Circular context-window usage indicator for the composer toolbar.

use gpui::{
    AnyView, App, Hsla, IntoElement, PathBuilder, Pixels, Point, Window, canvas, div, point,
    prelude::*, px,
};
use gpui_component::tooltip::Tooltip;

use crate::features::shell::state::AgentStatus;
use crate::tokens::Tokens;

const RING_SIZE: f32 = 18.0;
const STROKE_WIDTH: f32 = 2.0;

/// Display data for the context usage ring and its hover tooltip.
pub struct ContextUsageProps {
    pub used: f32,
    pub max: f32,
    pub usage_label: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub model: String,
    pub agent_status: Option<AgentStatus>,
    pub estimated_cost: Option<String>,
    pub index_status: Option<String>,
    pub read_cache_summary: Option<String>,
    pub page_cache_summary: Option<String>,
}

/// Renders a compact circular progress ring showing context fill level.
pub fn context_usage_ring(props: ContextUsageProps) -> impl IntoElement {
    let fraction = if props.max > 0.0 {
        (props.used / props.max).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let progress_color = usage_color(fraction);
    let tooltip_label = props.usage_label.clone();
    let tooltip_model = props.model.clone();
    let tooltip_status = props.agent_status.clone();
    let tooltip_cost = props.estimated_cost.clone();
    let tooltip_input = props.input_tokens;
    let tooltip_output = props.output_tokens;
    let tooltip_cache_read = props.cache_read_tokens;
    let tooltip_cache_write = props.cache_write_tokens;
    let tooltip_index_status = props.index_status.clone();
    let tooltip_read_cache = props.read_cache_summary.clone();
    let tooltip_page_cache = props.page_cache_summary.clone();

    div()
        .id("context-usage-ring")
        .size(px(RING_SIZE))
        .flex_shrink_0()
        .cursor_pointer()
        .tooltip(move |window, cx| {
            context_usage_tooltip(
                &tooltip_label,
                tooltip_input,
                tooltip_output,
                tooltip_cache_read,
                tooltip_cache_write,
                &tooltip_model,
                &tooltip_status,
                &tooltip_cost,
                tooltip_index_status.clone(),
                tooltip_read_cache.clone(),
                tooltip_page_cache.clone(),
                window,
                cx,
            )
        })
        .child(
            canvas(
                move |_, _, _| fraction,
                move |bounds, fraction, window, _| {
                    let size = f32::from(bounds.size.width).min(f32::from(bounds.size.height));
                    let center = bounds.center();
                    let radius = (size - STROKE_WIDTH) / 2.0;
                    let track = Tokens::border_subtle();

                    stroke_arc(window, center, radius, -90.0, 269.5, track, STROKE_WIDTH);
                    if fraction > 0.0 {
                        stroke_arc(
                            window,
                            center,
                            radius,
                            -90.0,
                            -90.0 + fraction * 359.0,
                            progress_color,
                            STROKE_WIDTH,
                        );
                    }
                },
            )
            .size(px(RING_SIZE)),
        )
}

fn context_usage_tooltip(
    usage_label: &str,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    model: &str,
    status: &Option<AgentStatus>,
    estimated_cost: &Option<String>,
    index_status: Option<String>,
    read_cache_summary: Option<String>,
    page_cache_summary: Option<String>,
    window: &mut Window,
    cx: &mut App,
) -> AnyView {
    let usage_label = usage_label.to_string();
    let model = model.to_string();
    let estimated_cost = estimated_cost.clone();
    let status_label = match status {
        Some(AgentStatus::Idle) => "Idle",
        Some(AgentStatus::Thinking) => "Thinking",
        Some(AgentStatus::RunningTool) => "Running tool",
        Some(AgentStatus::WaitingApproval) => "Awaiting approval",
        Some(AgentStatus::Completed) => "Completed",
        Some(AgentStatus::Failed) => "Failed",
        None => "Unknown",
    };
    let input_line = format_token_line("Input", input_tokens);
    let output_line = format_token_line("Output", output_tokens);
    let cache_read_line = format_token_line("Cache read", cache_read_tokens);
    let cache_write_line = format_token_line("Cache write", cache_write_tokens);
    let show_cache = cache_read_tokens > 0 || cache_write_tokens > 0;

    Tooltip::element(move |_, _| {
        div()
            .flex()
            .flex_col()
            .gap(Tokens::spacing_1())
            .child(
                div()
                    .text_size(Tokens::text_xs())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(Tokens::text_primary())
                    .child("Context window"),
            )
            .child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_secondary())
                    .child(format!("{usage_label} prompt tokens")),
            )
            .child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_tertiary())
                    .child(input_line.clone()),
            )
            .child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_tertiary())
                    .child(output_line.clone()),
            )
            .when(show_cache, |el| {
                el.child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_tertiary())
                        .child(cache_read_line.clone()),
                )
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_tertiary())
                        .child(cache_write_line.clone()),
                )
            })
            .when(estimated_cost.is_some(), |el| {
                let cost = estimated_cost.clone().unwrap_or_default();
                el.child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_secondary())
                        .child(format!("Run cost: {cost}")),
                )
            })
            .child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_tertiary())
                    .child(format!("Model: {model}")),
            )
            .child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_tertiary())
                    .child(format!("Status: {status_label}")),
            )
            .when(index_status.is_some(), |el| {
                el.child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_tertiary())
                        .child(index_status.clone().unwrap_or_default()),
                )
            })
            .when(read_cache_summary.is_some(), |el| {
                el.child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_tertiary())
                        .child(read_cache_summary.clone().unwrap_or_default()),
                )
            })
            .when(page_cache_summary.is_some(), |el| {
                el.child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_tertiary())
                        .child(page_cache_summary.clone().unwrap_or_default()),
                )
            })
    })
    .build(window, cx)
}

fn format_token_line(label: &str, count: u64) -> String {
    format!("{label}: {}", format_compact_tokens(count))
}

fn format_compact_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn usage_color(fraction: f32) -> Hsla {
    if fraction >= 0.9 {
        Tokens::danger()
    } else if fraction >= 0.7 {
        Tokens::warning()
    } else {
        Tokens::accent()
    }
}

fn polar_point(center: Point<Pixels>, radius: f32, degrees: f32) -> Point<Pixels> {
    let rad = degrees.to_radians();
    let cx = f32::from(center.x);
    let cy = f32::from(center.y);
    point(px(cx + radius * rad.cos()), px(cy + radius * rad.sin()))
}

fn stroke_arc(
    window: &mut Window,
    center: Point<Pixels>,
    radius: f32,
    start_deg: f32,
    end_deg: f32,
    color: Hsla,
    width: f32,
) {
    if (end_deg - start_deg).abs() < 0.01 {
        return;
    }

    let start = polar_point(center, radius, start_deg);
    let end = polar_point(center, radius, end_deg);
    let sweep = end_deg - start_deg;
    let large_arc = sweep.abs() > 180.0;

    let mut builder = PathBuilder::stroke(px(width));
    builder.move_to(start);
    builder.arc_to(
        point(px(radius), px(radius)),
        px(0.0),
        large_arc,
        sweep > 0.0,
        end,
    );

    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

/// Parse a display string like `"12.4K / 200K"` into numeric used/max values.
pub fn parse_token_usage(raw: &str) -> (f32, f32, String) {
    let mut parts = raw.split('/').map(str::trim);
    let used_label = parts.next().unwrap_or("0").to_string();
    let max_label = parts.next().unwrap_or("200K").to_string();
    (
        parse_token_amount(&used_label),
        parse_token_amount(&max_label),
        format!("{used_label} / {max_label}"),
    )
}

fn parse_token_amount(raw: &str) -> f32 {
    let raw = raw.trim();
    if raw.is_empty() {
        return 0.0;
    }

    let upper = raw.to_uppercase();
    if let Some(num) = upper.strip_suffix('K') {
        num.trim().parse::<f32>().unwrap_or(0.0) * 1_000.0
    } else if let Some(num) = upper.strip_suffix('M') {
        num.trim().parse::<f32>().unwrap_or(0.0) * 1_000_000.0
    } else {
        raw.parse::<f32>().unwrap_or(0.0)
    }
}
