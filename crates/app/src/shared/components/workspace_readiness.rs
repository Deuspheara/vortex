use std::rc::Rc;

use gpui::{FontWeight, IntoElement, Window, div, prelude::*, px};
use gpui_component::Icon;

use crate::shared::components::buttons::{btn_ghost_label, btn_outline};
use crate::shared::state::{ReadinessState, WorkspaceReadiness};
use crate::tokens::{Tokens, icons};

pub struct WorkspaceReadinessCardProps {
    pub readiness: WorkspaceReadiness,
    pub on_open_settings: Option<Rc<dyn Fn(&mut gpui::App)>>,
    pub on_open_project: Option<Rc<dyn Fn(&mut Window, &mut gpui::App)>>,
    pub on_trust_project: Option<Rc<dyn Fn(&mut gpui::App)>>,
    pub on_open_context: Option<Rc<dyn Fn(&mut gpui::App)>>,
}

pub fn workspace_readiness_card(props: WorkspaceReadinessCardProps) -> impl IntoElement {
    let overall_state = props.readiness.overall_state();
    let badge = readiness_badge_label(&props.readiness);
    let title = readiness_title(&props.readiness);
    let subtitle = props.readiness.next_step_label();
    let settings_action = props.on_open_settings.clone();
    let settings_fallback_action = props.on_open_settings;
    let progress = if props.readiness.is_ready() {
        "All core checks are ready for grounded runs.".to_string()
    } else {
        format!(
            "{} of {} checks are ready.",
            props.readiness.ready_count(),
            props.readiness.total_count()
        )
    };

    div()
        .w_full()
        .rounded(Tokens::radius_md())
        .border_1()
        .border_color(border_color(overall_state))
        .bg(Tokens::surface())
        .px(Tokens::spacing_3())
        .py(Tokens::spacing_3())
        .flex()
        .flex_col()
        .gap(Tokens::spacing_3())
        .child(
            div()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_2())
                .child(
                    div().flex().items_center().gap(Tokens::spacing_2()).child(
                        div()
                            .px(Tokens::spacing_2())
                            .py(Tokens::spacing_0p5())
                            .rounded(Tokens::radius_full())
                            .bg(icon_color(overall_state).opacity(0.14))
                            .child(
                                div()
                                    .text_size(Tokens::text_xs())
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(icon_color(overall_state))
                                    .child(badge),
                            ),
                    ),
                )
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(Tokens::spacing_2())
                        .child(
                            div().mt(px(1.0)).child(
                                Icon::new(state_icon(overall_state))
                                    .size(px(14.0))
                                    .text_color(icon_color(overall_state)),
                            ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap(Tokens::spacing_0p5())
                                .child(
                                    div()
                                        .text_size(Tokens::text_sm())
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(Tokens::text_primary())
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .text_size(Tokens::text_xs())
                                        .line_height(Tokens::text_sm_leading())
                                        .text_color(Tokens::text_secondary())
                                        .child(subtitle),
                                )
                                .child(
                                    div()
                                        .text_size(Tokens::text_xs())
                                        .text_color(Tokens::text_tertiary())
                                        .child(progress),
                                ),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_1())
                .children(props.readiness.checks.into_iter().map(render_check)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .flex_wrap()
                .gap(Tokens::spacing_2())
                .when(!props.readiness.provider_connected, |el| {
                    el.when_some(settings_action, |el, cb| {
                        el.child(
                            btn_outline("workspace-readiness-settings", "Connect provider")
                                .icon(icons::SETTINGS)
                                .on_click(move |_, _, app: &mut gpui::App| cb(app)),
                        )
                    })
                })
                .when_some(props.on_open_project, |el, cb| {
                    el.child(
                        btn_ghost_label("workspace-readiness-open-project", "Open project")
                            .icon(icons::OPEN_IDE)
                            .on_click(move |_, window, app: &mut gpui::App| cb(window, app)),
                    )
                })
                .when_some(props.on_trust_project, |el, cb| {
                    el.child(
                        btn_outline("workspace-readiness-trust", "Trust project")
                            .on_click(move |_, _, app: &mut gpui::App| cb(app)),
                    )
                })
                .when_some(props.on_open_context, |el, cb| {
                    el.child(
                        btn_ghost_label("workspace-readiness-context", "Check context")
                            .on_click(move |_, _, app: &mut gpui::App| cb(app)),
                    )
                })
                .when(props.readiness.provider_connected, |el| {
                    el.when_some(settings_fallback_action, |el, cb| {
                        el.child(
                            btn_ghost_label("workspace-readiness-settings", "Settings")
                                .on_click(move |_, _, app: &mut gpui::App| cb(app)),
                        )
                    })
                }),
        )
}

fn readiness_badge_label(readiness: &WorkspaceReadiness) -> &'static str {
    match readiness.overall_state() {
        ReadinessState::Ready => "Ready to run",
        ReadinessState::InProgress => "Preparing workspace",
        ReadinessState::NeedsAttention => "Setup needed",
    }
}

fn readiness_title(readiness: &WorkspaceReadiness) -> String {
    if readiness.is_ready() {
        "You’re ready for grounded runs".to_string()
    } else {
        format!("Before your next run: {}", readiness.summary_label())
    }
}

fn render_check(check: crate::shared::state::WorkspaceReadinessCheck) -> impl IntoElement {
    div()
        .flex()
        .items_start()
        .gap(Tokens::spacing_2())
        .child(
            div()
                .h(px(Tokens::ROW_HEIGHT_SM))
                .flex()
                .items_center()
                .child(
                    div()
                        .size(px(8.0))
                        .rounded_full()
                        .bg(icon_color(check.state)),
                ),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_0p5())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(Tokens::spacing_2())
                        .child(
                            div()
                                .text_size(Tokens::text_xs())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Tokens::text_primary())
                                .child(check.title),
                        )
                        .child(
                            div()
                                .text_size(Tokens::text_xs())
                                .text_color(Tokens::text_tertiary())
                                .child(check.state.label()),
                        ),
                )
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .line_height(Tokens::text_sm_leading())
                        .text_color(Tokens::text_secondary())
                        .child(check.detail),
                ),
        )
}

fn state_icon(state: ReadinessState) -> gpui_component::IconName {
    match state {
        ReadinessState::Ready => icons::SHIELD_CHECK,
        ReadinessState::InProgress => icons::LOADER,
        ReadinessState::NeedsAttention => icons::TRIANGLE_ALERT,
    }
}

fn icon_color(state: ReadinessState) -> gpui::Hsla {
    match state {
        ReadinessState::Ready => Tokens::success(),
        ReadinessState::InProgress => Tokens::accent(),
        ReadinessState::NeedsAttention => Tokens::warning(),
    }
}

fn border_color(state: ReadinessState) -> gpui::Hsla {
    match state {
        ReadinessState::Ready => Tokens::success().opacity(0.45),
        ReadinessState::InProgress => Tokens::accent().opacity(0.45),
        ReadinessState::NeedsAttention => Tokens::warning().opacity(0.45),
    }
}
