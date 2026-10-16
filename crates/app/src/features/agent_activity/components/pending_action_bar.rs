//! Blocking action controls shown directly above the composer.

use gpui::{FontWeight, IntoElement, SharedString, div, prelude::*, px};

use crate::shared::components::buttons::{btn_approve, btn_deny, btn_ghost_label};
use crate::tokens::icons;
use crate::tokens::{Tokens, activity_action_line_with_loading};
use gpui_component::Icon;

pub struct PendingActionBarProps {
    pub current_action: Option<CurrentActionProps>,
    pub approval: Option<ApprovalActionProps>,
    pub patch: Option<PatchActionProps>,
    pub plan_review: Option<PlanReviewActionProps>,
}

pub struct CurrentActionProps {
    pub label: String,
    pub detail: Option<String>,
    pub on_view_activity: Box<dyn Fn(&mut gpui::App) + 'static>,
    pub on_stop: Box<dyn Fn(&mut gpui::App) + 'static>,
}

#[allow(dead_code)]
pub struct ApprovalActionProps {
    pub title: String,
    pub risk: crate::features::shell::state::ApprovalRisk,
    pub risk_label: String,
    pub allow_always_label: Option<String>,
    pub can_act: bool,
    pub on_reject: Box<dyn Fn(&mut gpui::App) + 'static>,
    pub on_approve: Box<dyn Fn(&mut gpui::App) + 'static>,
    pub on_approve_always: Box<dyn Fn(&mut gpui::App) + 'static>,
}

pub struct PatchActionProps {
    pub summary: String,
    pub on_open: Box<dyn Fn(&mut gpui::App) + 'static>,
    pub on_cancel: Box<dyn Fn(&mut gpui::App) + 'static>,
}

pub struct PlanReviewActionProps {
    pub title: String,
    pub detail: String,
    pub primary_label: String,
    pub on_review: Box<dyn Fn(&mut gpui::App) + 'static>,
    pub on_implement: Box<dyn Fn(&mut gpui::App) + 'static>,
}

pub fn pending_action_bar(props: PendingActionBarProps) -> impl IntoElement {
    div()
        .id("pending-action-bar")
        .w_full()
        .max_w(px(Tokens::COMPOSER_MAX_WIDTH))
        .flex()
        .flex_col()
        .gap(Tokens::spacing_1())
        .pb(Tokens::spacing_2())
        .when_some(props.current_action, |el, current| {
            el.child(current_action_row(current))
        })
        .when_some(props.patch, |el, patch| el.child(patch_action_row(patch)))
        .when_some(props.plan_review, |el, plan_review| {
            el.child(plan_review_action_row(plan_review))
        })
        .when_some(props.approval, |el, approval| {
            el.child(approval_action_row(approval))
        })
}

fn current_action_row(props: CurrentActionProps) -> impl IntoElement {
    let on_view_activity = props.on_view_activity;
    let on_stop = props.on_stop;

    div()
        .id("current-action-row")
        .w_full()
        .min_h(px(Tokens::ROW_HEIGHT_MD))
        .px(Tokens::spacing_2())
        .py(Tokens::spacing_1p5())
        .rounded(Tokens::radius_xs())
        .border_1()
        .border_color(Tokens::border_subtle())
        .bg(Tokens::surface_hover().opacity(0.4))
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .child(activity_action_line_with_loading(
                    &props.label,
                    props.detail.as_deref(),
                    true,
                    false,
                    true,
                    "current-action",
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .flex_shrink_0()
                .gap(Tokens::spacing_1())
                .child(
                    btn_ghost_label("current-action-activity", "Activity").on_click(
                        move |_, _, app: &mut gpui::App| {
                            on_view_activity(app);
                        },
                    ),
                )
                .child(btn_deny("current-action-stop", "Stop").on_click(
                    move |_, _, app: &mut gpui::App| {
                        on_stop(app);
                    },
                )),
        )
}

fn approval_action_row(props: ApprovalActionProps) -> impl IntoElement {
    let on_reject = props.on_reject;
    let on_approve = props.on_approve;
    let on_approve_always = props.on_approve_always;
    let allow_label = props.allow_always_label.map(SharedString::from);

    div()
        .id("approval-action-row")
        .w_full()
        .min_h(px(Tokens::ROW_HEIGHT_LG))
        .px(Tokens::spacing_3())
        .py(Tokens::spacing_2())
        .rounded(Tokens::radius_sm())
        .bg(Tokens::surface_active())
        .border_1()
        .border_color(Tokens::border_subtle())
        .flex()
        .items_center()
        .gap(Tokens::spacing_3())
        .child(
            Icon::new(icons::TERMINAL)
                .size(px(14.0))
                .text_color(Tokens::warning()),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .min_w(px(0.0))
                .flex_col()
                .gap(Tokens::spacing_0p5())
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Tokens::text_primary())
                        .overflow_hidden()
                        .child(props.title),
                )
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Tokens::text_tertiary())
                        .child(format!(
                            "Blocked until you review this request · {}",
                            props.risk_label
                        )),
                ),
        )
        .when(props.can_act, |el| {
            el.child(
                div()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .gap(Tokens::spacing_1())
                    .child(btn_deny("thread-reject-approval", "Not now").on_click(
                        move |_, _, app: &mut gpui::App| {
                            on_reject(app);
                        },
                    ))
                    .when_some(allow_label, |el, label| {
                        el.child(btn_ghost_label("thread-approve-always", label).on_click(
                            move |_, _, app: &mut gpui::App| {
                                on_approve_always(app);
                            },
                        ))
                    })
                    .child(btn_approve("thread-approve-approval", "Continue").on_click(
                        move |_, _, app: &mut gpui::App| {
                            on_approve(app);
                        },
                    )),
            )
        })
}

fn patch_action_row(props: PatchActionProps) -> impl IntoElement {
    let on_open = props.on_open;
    let on_cancel = props.on_cancel;

    div()
        .id("patch-action-row")
        .w_full()
        .min_h(px(Tokens::ROW_HEIGHT_MD))
        .px(Tokens::spacing_2())
        .py(Tokens::spacing_1p5())
        .rounded(Tokens::radius_xs())
        .border_1()
        .border_color(Tokens::border_subtle())
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .child(
            Icon::new(icons::GIT_COMPARE)
                .size(px(14.0))
                .text_color(Tokens::text_tertiary()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .text_size(Tokens::text_sm())
                .text_color(Tokens::text_secondary())
                .child(format!("Changes ready · {}", props.summary)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .flex_shrink_0()
                .gap(Tokens::spacing_1())
                .child(btn_ghost_label("thread-open-diff", "Open diff").on_click(
                    move |_, _, app: &mut gpui::App| {
                        on_open(app);
                    },
                ))
                .child(btn_ghost_label("thread-cancel-patch", "Dismiss").on_click(
                    move |_, _, app: &mut gpui::App| {
                        on_cancel(app);
                    },
                )),
        )
}

fn plan_review_action_row(props: PlanReviewActionProps) -> impl IntoElement {
    let on_review = props.on_review;
    let on_implement = props.on_implement;

    div()
        .id("plan-review-action-row")
        .w_full()
        .min_h(px(Tokens::ROW_HEIGHT_LG))
        .px(Tokens::spacing_3())
        .py(Tokens::spacing_2())
        .rounded(Tokens::radius_sm())
        .bg(Tokens::surface())
        .border_1()
        .border_color(Tokens::border_subtle())
        .flex()
        .items_center()
        .gap(Tokens::spacing_3())
        .child(
            Icon::new(icons::CHECKLIST)
                .size(px(14.0))
                .text_color(Tokens::accent()),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .min_w(px(0.0))
                .flex_col()
                .gap(Tokens::spacing_0p5())
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Tokens::text_primary())
                        .child(props.title),
                )
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_tertiary())
                        .child(props.detail),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .flex_shrink_0()
                .gap(Tokens::spacing_1())
                .child(
                    btn_ghost_label("thread-review-plan", "Review plan").on_click(
                        move |_, _, app: &mut gpui::App| {
                            on_review(app);
                        },
                    ),
                )
                .child(
                    btn_approve("thread-implement-plan", props.primary_label).on_click(
                        move |_, _, app: &mut gpui::App| {
                            on_implement(app);
                        },
                    ),
                ),
        )
}
