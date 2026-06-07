//! Animation presets and helpers for consistent motion across the UI.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, ElementId, FontWeight, IntoElement, SharedString, div,
    ease_in_out, ease_out_quint, prelude::*, px,
};

use super::design_tokens::Tokens;

/// Braille spinner frames (Unicode progress indicator).
pub const BRAILLE_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn sidebar_snappy_ease(t: f32) -> f32 {
    let inv = 1.0 - t;
    1.0 - inv * inv * inv * inv
}

fn sidebar_expand_ease(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn braille_frame(delta: f32) -> &'static str {
    let idx = (delta * BRAILLE_FRAMES.len() as f32) as usize % BRAILLE_FRAMES.len();
    BRAILLE_FRAMES[idx]
}

/// Animated braille spinner — shared by streaming cursor and activity rows.
pub fn braille_spinner(key: &str, animate: bool) -> AnyElement {
    let base = || {
        div()
            .text_size(Tokens::text_base())
            .font_family("monospace")
            .text_color(Tokens::accent())
    };

    if animate {
        base()
            .with_animation(
                element_key("braille-spin", key),
                Animation::new(Duration::from_millis(200)).repeat(),
                |el, delta| el.child(braille_frame(delta).to_string()),
            )
            .into_any_element()
    } else {
        base()
            .child(BRAILLE_FRAMES[0].to_string())
            .into_any_element()
    }
}

/// Shared animation durations and easing presets.
pub struct Motion;

impl Motion {
    pub fn fade_in() -> Animation {
        Animation::new(Duration::from_millis(220)).with_easing(ease_out_quint())
    }

    #[allow(dead_code)]
    pub fn expand() -> Animation {
        Animation::new(Duration::from_millis(200)).with_easing(ease_in_out)
    }

    #[allow(dead_code)]
    pub fn sidebar() -> Animation {
        Animation::new(Duration::from_millis(250)).with_easing(ease_in_out)
    }

    pub fn sidebar_row() -> Animation {
        Animation::new(Duration::from_millis(180)).with_easing(sidebar_snappy_ease)
    }

    pub fn sidebar_expand() -> Animation {
        Animation::new(Duration::from_millis(260)).with_easing(sidebar_expand_ease)
    }

    #[allow(dead_code)]
    pub fn panel() -> Animation {
        Animation::new(Duration::from_millis(280)).with_easing(ease_out_quint())
    }

    pub fn page() -> Animation {
        Animation::new(Duration::from_millis(320)).with_easing(ease_out_quint())
    }
}

/// Stable element id from a prefix + dynamic key (avoids `Box::leak`).
pub fn element_key(prefix: &'static str, key: &str) -> ElementId {
    ElementId::from(SharedString::from(format!("{prefix}-{key}")))
}

/// Fade wrapped content in on mount.
pub fn fade_in(content: impl IntoElement, id: impl Into<ElementId>) -> impl IntoElement {
    div()
        .child(content)
        .with_animation(id, Motion::fade_in(), |el, delta| el.opacity(delta))
}

/// Fade + subtle upward slide for expanding groups.
#[allow(dead_code)]
pub fn expand_in(content: impl IntoElement, id: impl Into<ElementId>) -> impl IntoElement {
    div()
        .child(content)
        .with_animation(id, Motion::expand(), |el, delta| {
            el.opacity(delta).mt(px(6.0 * (1.0 - delta)))
        })
}

/// Staggered fade for list items.
pub fn stagger_fade_in(
    content: impl IntoElement,
    id: impl Into<ElementId>,
    index: usize,
) -> impl IntoElement {
    let delay_ms = (index as u64).min(6) * 35;
    let duration = Duration::from_millis(180 + delay_ms);
    div().child(content).with_animation(
        id,
        Animation::new(duration).with_easing(ease_out_quint()),
        |el, delta| el.opacity(delta).mt(px(8.0 * (1.0 - delta))),
    )
}

/// Sidebar content fade when expanding from collapsed rail.
#[allow(dead_code)]
pub fn sidebar_content_in(content: impl IntoElement) -> impl IntoElement {
    div()
        .h_full()
        .w_full()
        .flex()
        .flex_col()
        .child(content)
        .with_animation("sidebar-content-in", Motion::sidebar(), |el, delta| {
            el.opacity(delta)
        })
}

/// Sidebar row entrance with a quick pickup and softer settle.
pub fn sidebar_row_in(content: impl IntoElement, id: impl Into<ElementId>) -> impl IntoElement {
    div().child(content).with_animation(id, Motion::sidebar_row(), |el, delta| {
        el.opacity(delta).mt(px(6.0 * (1.0 - delta)))
    })
}

/// Reveal for nested rows that appear on project expand.
pub fn sidebar_expand_in(content: impl IntoElement, id: impl Into<ElementId>) -> impl IntoElement {
    div()
        .overflow_hidden()
        .child(content)
        .with_animation(id, Motion::sidebar_expand(), |el, delta| {
            el.opacity(delta).mt(px(8.0 * (1.0 - delta)))
        })
}

/// Slide + fade for panels (diff, drawer).
#[allow(dead_code)]
pub fn panel_slide_in(content: impl IntoElement, id: impl Into<ElementId>) -> impl IntoElement {
    div()
        .child(content)
        .with_animation(id, Motion::panel(), |el, delta| {
            el.opacity(delta).ml(px(12.0 * (1.0 - delta)))
        })
}

/// Settings / full-page content entrance.
pub fn page_fade_in(content: impl IntoElement) -> impl IntoElement {
    div()
        .h_full()
        .w_full()
        .min_h(px(0.0))
        .overflow_hidden()
        .child(content)
        .with_animation("settings-page-in", Motion::page(), |el, delta| {
            el.opacity(delta).mt(px(10.0 * (1.0 - delta)))
        })
}

/// One activity line — spinner only while `running`; muted when idle.
///
/// Pass `animate: false` while scrolling to avoid repaint storms.
///
/// Keep this pulse state-driven. A repeating `with_animation(...)` requests a new
/// frame continuously in GPUI, which is too expensive inside the thread list.
pub fn activity_action_line(
    action: &str,
    detail: Option<&str>,
    running: bool,
    _animate: bool,
    key: &str,
    _index: usize,
) -> impl IntoElement {
    let action_owned = action.to_string();
    let detail_owned = detail.map(str::to_string);
    let (verb, remainder) = action_verb_and_remainder(&action_owned);
    let (action_color, action_weight) = if running {
        (Tokens::text_primary(), FontWeight::MEDIUM)
    } else {
        (Tokens::text_secondary(), FontWeight::MEDIUM)
    };

    div()
        .id(element_key("activity-line", key))
        .h(px(Tokens::ROW_HEIGHT_SM))
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .when(running, |el| {
            el.child(
                div()
                    .flex_shrink_0()
                    .w(px(12.0))
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(Tokens::text_base())
                            .font_family("monospace")
                            .text_color(Tokens::accent())
                            .child(BRAILLE_FRAMES[0]),
                    ),
            )
        })
        .child(
            div().min_w(px(0.0)).overflow_hidden().child(
                div()
                    .id(element_key("activity-action-text", key))
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .flex()
                    .items_center()
                    .gap(px(0.0))
                    .font_family(Tokens::ui_font_family())
                    .text_size(Tokens::text_sm())
                    .line_height(Tokens::text_sm_leading())
                    .font_weight(action_weight)
                    .text_color(action_color)
                    .hover(|s| s.text_color(Tokens::text_primary()))
                    .child(div().opacity(0.72).child(verb))
                    .when_some(remainder, |el, rest| {
                        el.child(div().opacity(if running { 0.72 } else { 0.68 }).child(rest))
                    }),
            ),
        )
        .when_some(detail_owned.as_ref(), |el, d| {
            el.child(
                div()
                    .text_size(Tokens::text_sm())
                    .line_height(Tokens::text_sm_leading_compact())
                    .font_family(Tokens::ui_font_family())
                    .text_color(Tokens::text_faint())
                    .opacity(0.62)
                    .truncate()
                    .child(d.clone()),
            )
        })
}

fn action_verb_and_remainder(action: &str) -> (String, Option<String>) {
    let trimmed = action.trim();
    if trimmed.is_empty() {
        return (String::new(), None);
    }
    if let Some((verb, rest)) = trimmed.split_once(' ') {
        let remainder = rest.trim();
        (
            verb.to_string(),
            (!remainder.is_empty()).then(|| format!(" {remainder}")),
        )
    } else {
        (trimmed.to_string(), None)
    }
}

/// Fade-in for a newly appeared activity line.
#[allow(dead_code)]
pub fn activity_line_in(
    content: impl IntoElement,
    id: impl Into<ElementId>,
    index: usize,
) -> impl IntoElement {
    let delay_ms = (index as u64).min(8) * 45;
    div().child(content).with_animation(
        id,
        Animation::new(Duration::from_millis(280 + delay_ms)).with_easing(ease_out_quint()),
        |el, delta| el.opacity(delta),
    )
}

/// Gentle repeating opacity pulse for the active action label.
#[allow(dead_code)]
pub fn text_pulse(
    content: impl IntoElement,
    id: impl Into<ElementId>,
    animate: bool,
) -> AnyElement {
    if !animate {
        return div().child(content).into_any_element();
    }
    div()
        .child(content)
        .with_animation(
            id,
            Animation::new(Duration::from_millis(1400)).repeat(),
            |el, delta| {
                let wave = (delta * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                el.opacity(0.45 + 0.55 * wave)
            },
        )
        .into_any_element()
}

/// Opacity + slide entrance for todo strip rows.
pub fn todo_row_in(
    content: impl IntoElement,
    id: impl Into<ElementId>,
    index: usize,
) -> impl IntoElement {
    stagger_fade_in(content, id, index)
}

/// Pop-in for a todo completing — green fill + checkmark.
pub fn todo_complete_in(content: impl IntoElement, id: impl Into<ElementId>) -> impl IntoElement {
    div().child(content).with_animation(
        id,
        Animation::new(Duration::from_millis(340)).with_easing(ease_out_quint()),
        |el, delta| el.opacity(delta).mt(px(4.0 * (1.0 - delta))),
    )
}

/// Gentle pulse on the in-progress todo ring.
pub fn todo_progress_pulse(content: impl IntoElement, id: impl Into<ElementId>) -> AnyElement {
    div()
        .child(content)
        .with_animation(
            id,
            Animation::new(Duration::from_millis(1100)).repeat(),
            |el, delta| {
                let wave = (delta * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                el.opacity(0.55 + 0.45 * wave)
            },
        )
        .into_any_element()
}

/// Fade-in for newly revealed todos when expanding the strip.
#[allow(dead_code)]
pub fn todo_expand_in(content: impl IntoElement, id: impl Into<ElementId>) -> impl IntoElement {
    expand_in(content, id)
}

/// Opacity-only entrance for virtual-list thread rows (no translate — keeps scroll smooth).
#[allow(dead_code)]
pub fn thread_row_in(content: impl IntoElement, id: impl Into<ElementId>) -> impl IntoElement {
    div().child(content).with_animation(
        id,
        Animation::new(Duration::from_millis(160)).with_easing(ease_out_quint()),
        |el, delta| el.opacity(delta),
    )
}

#[cfg(test)]
mod tests {
    use super::action_verb_and_remainder;

    #[test]
    fn splits_action_into_verb_and_remainder() {
        assert_eq!(
            action_verb_and_remainder("Writing crates/app/src/main.rs"),
            (
                "Writing".to_string(),
                Some(" crates/app/src/main.rs".to_string())
            )
        );
    }

    #[test]
    fn handles_single_word_action() {
        assert_eq!(
            action_verb_and_remainder("Planning"),
            ("Planning".to_string(), None)
        );
    }
}
