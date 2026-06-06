use std::path::PathBuf;

use agent_protocol::{
    AndroidActionPhase, AndroidActionTrace, AndroidActionVisualization, AndroidObservation,
    AndroidPointPx, AndroidSessionState,
};
use android_device::ScreenTransform;
use gpui::{FontWeight, IntoElement, ObjectFit, StyledImage, div, img, prelude::*, px};
use gpui_component::scroll::ScrollableElement;

use crate::tokens::Tokens;

const PHONE_VIEW_WIDTH: f32 = 260.0;
const PHONE_VIEW_HEIGHT: f32 = 540.0;

pub fn render_android_simulator_panel(session: AndroidSessionState) -> impl IntoElement {
    div()
        .id("android-simulator-panel")
        .size_full()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .bg(Tokens::panel_bg())
        .child(header(&session))
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scrollbar()
                .p(Tokens::spacing_3())
                .flex()
                .flex_col()
                .gap(Tokens::spacing_4())
                .child(phone_surface(
                    session.latest_observation.clone(),
                    session.current_action.clone(),
                ))
                .child(current_action(session.current_action.clone()))
                .child(action_timeline(session.recent_actions.clone())),
        )
}

fn header(session: &AndroidSessionState) -> impl IntoElement {
    let device = session
        .device
        .as_ref()
        .map(|device| device.name.clone().unwrap_or_else(|| device.serial.clone()))
        .unwrap_or_else(|| "No device".into());
    let app = session
        .current_app
        .as_deref()
        .or(session.current_activity.as_deref())
        .unwrap_or("No app");

    div()
        .id("android-simulator-header")
        .px(Tokens::spacing_3())
        .py(Tokens::spacing_2())
        .border_b_1()
        .border_color(Tokens::border_subtle())
        .flex()
        .flex_col()
        .gap(Tokens::spacing_1())
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(Tokens::spacing_2())
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .font_weight(FontWeight::MEDIUM)
                        .child(device),
                )
                .child(status_pill(&session.status)),
        )
        .child(
            div()
                .text_size(Tokens::text_xs())
                .text_color(Tokens::text_tertiary())
                .child(format!("{app} · {:?}", session.control_mode)),
        )
}

fn status_pill(status: &str) -> impl IntoElement {
    div()
        .px(Tokens::spacing_2())
        .py(Tokens::spacing_0p5())
        .rounded(Tokens::radius_full())
        .bg(Tokens::surface_active())
        .text_size(Tokens::text_xs())
        .text_color(Tokens::text_secondary())
        .child(status.to_string())
}

fn phone_surface(
    observation: Option<AndroidObservation>,
    action: Option<AndroidActionVisualization>,
) -> impl IntoElement {
    let image_path = observation
        .as_ref()
        .and_then(|obs| obs.screenshot_ref.as_deref())
        .and_then(screenshot_path);
    let targets = observation
        .as_ref()
        .map(|obs| obs.visible_targets.clone())
        .unwrap_or_default();
    let screen = observation.as_ref().map(|obs| obs.screen);
    let transform = screen.map(|screen| {
        ScreenTransform::new(
            screen.width,
            screen.height,
            PHONE_VIEW_WIDTH,
            PHONE_VIEW_HEIGHT,
        )
    });

    div()
        .id("android-phone-surface")
        .w_full()
        .flex()
        .justify_center()
        .child(
            div()
                .relative()
                .w(px(PHONE_VIEW_WIDTH))
                .h(px(PHONE_VIEW_HEIGHT))
                .overflow_hidden()
                .rounded(px(28.0))
                .border_1()
                .border_color(Tokens::border())
                .bg(Tokens::main_bg())
                .when_some(image_path, |el, path| {
                    el.child(img(path).w_full().h_full().object_fit(ObjectFit::Contain))
                })
                .when(observation.is_none(), |el| {
                    el.child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(Tokens::text_sm())
                            .text_color(Tokens::text_tertiary())
                            .child("No Android observation"),
                    )
                })
                .children(targets.into_iter().take(12).filter_map(move |node| {
                    let transform = transform?;
                    let top_left = transform.device_to_view(AndroidPointPx {
                        x: node.bounds.left,
                        y: node.bounds.top,
                    });
                    let bottom_right = transform.device_to_view(AndroidPointPx {
                        x: node.bounds.right,
                        y: node.bounds.bottom,
                    });
                    let left = top_left.x;
                    let top = top_left.y;
                    let width = bottom_right.x - top_left.x;
                    let height = bottom_right.y - top_left.y;
                    Some(
                        div()
                            .absolute()
                            .left(px(left))
                            .top(px(top))
                            .w(px(width.max(1.0)))
                            .h(px(height.max(1.0)))
                            .border_1()
                            .border_color(Tokens::accent().alpha(0.55))
                            .rounded(Tokens::radius_xs())
                            .into_any_element(),
                    )
                }))
                .when_some(action, |el, action| {
                    el.child(action_overlay(action, screen))
                }),
        )
}

fn action_overlay(
    action: AndroidActionVisualization,
    screen: Option<agent_protocol::AndroidSizePx>,
) -> impl IntoElement {
    let point = action.to.or(action.from);
    let (left, top) = match (point, screen) {
        (Some(point), Some(screen)) => {
            let transform = ScreenTransform::new(
                screen.width,
                screen.height,
                PHONE_VIEW_WIDTH,
                PHONE_VIEW_HEIGHT,
            );
            let point = transform.device_to_view(point);
            (
                point.x.clamp(0.0, PHONE_VIEW_WIDTH),
                point.y.clamp(0.0, PHONE_VIEW_HEIGHT),
            )
        }
        _ => (130.0, 270.0),
    };
    let color = match action.phase {
        AndroidActionPhase::Failed => Tokens::danger(),
        AndroidActionPhase::WaitingForUi => Tokens::warning(),
        _ => Tokens::accent(),
    };

    div()
        .absolute()
        .left(px(left - 10.0))
        .top(px(top - 10.0))
        .size(px(20.0))
        .rounded(Tokens::radius_full())
        .border_1()
        .border_color(color)
        .bg(color.alpha(0.25))
        .child(
            div()
                .absolute()
                .left(px(-8.0))
                .top(px(-8.0))
                .size(px(36.0))
                .rounded(Tokens::radius_full())
                .border_1()
                .border_color(color.alpha(0.35)),
        )
}

fn current_action(action: Option<AndroidActionVisualization>) -> impl IntoElement {
    let Some(action) = action else {
        return section("Current action", vec![muted_row("Idle".into())]).into_any_element();
    };
    section(
        "Current action",
        vec![
            text_row(action.label),
            action
                .reason
                .map(|reason| muted_row(format!("reason: {reason}")))
                .unwrap_or_else(|| muted_row("reason: not provided".into())),
            action
                .confidence
                .map(|confidence| muted_row(format!("confidence: {confidence}")))
                .unwrap_or_else(|| muted_row(format!("phase: {:?}", action.phase))),
        ],
    )
    .into_any_element()
}

fn action_timeline(actions: Vec<AndroidActionTrace>) -> impl IntoElement {
    if actions.is_empty() {
        return section(
            "Journey / Action Timeline",
            vec![muted_row("No actions recorded".into())],
        )
        .into_any_element();
    }
    section(
        "Journey / Action Timeline",
        actions
            .into_iter()
            .rev()
            .take(8)
            .map(|action| {
                let target = action.target.unwrap_or_else(|| "screen".into());
                text_row(format!(
                    "{} · {} · {}",
                    action.action, target, action.status
                ))
            })
            .collect(),
    )
    .into_any_element()
}

fn section(title: &str, rows: Vec<gpui::AnyElement>) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .child(
            div()
                .text_size(Tokens::text_xs())
                .text_color(Tokens::text_tertiary())
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_ascii_uppercase()),
        )
        .children(rows)
}

fn text_row(text: String) -> gpui::AnyElement {
    div()
        .min_h(px(Tokens::ROW_HEIGHT_MD))
        .px(Tokens::spacing_2())
        .py(Tokens::spacing_1())
        .rounded(Tokens::radius_xs())
        .bg(Tokens::surface_hover().alpha(0.35))
        .text_size(Tokens::text_sm())
        .text_color(Tokens::text_secondary())
        .child(text)
        .into_any_element()
}

fn muted_row(text: String) -> gpui::AnyElement {
    div()
        .text_size(Tokens::text_xs())
        .text_color(Tokens::text_tertiary())
        .child(text)
        .into_any_element()
}

fn screenshot_path(value: &str) -> Option<PathBuf> {
    value
        .strip_prefix("artifact://android/")
        .or(Some(value))
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}
