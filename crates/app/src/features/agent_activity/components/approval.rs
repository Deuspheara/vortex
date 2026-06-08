//! Approval UI — diff panel bar and thread hint row.

use crate::features::shell::state::ApprovalRisk;
use crate::shared::components::buttons::{btn_approve, btn_deny};
use crate::tokens::{Tokens, activity_action_line_with_loading};
use gpui::{App, FontWeight, IntoElement, div, prelude::*, px};

/// Sticky elevated approval card — primary blocking action surface near composer.
pub struct ApprovalCardProps {
    pub title: String,
    pub risk: ApprovalRisk,
    pub can_act: bool,
    pub allow_always_label: Option<String>,
    pub on_deny: Box<dyn Fn(&mut App) + 'static>,
    pub on_approve: Box<dyn Fn(&mut App) + 'static>,
    pub on_approve_always: Option<Box<dyn Fn(&mut App) + 'static>>,
}

pub fn approval_card(props: ApprovalCardProps) -> impl IntoElement {
    let title = props.title;
    let risk_label = props.risk.label().to_string();
    let on_deny = props.on_deny;
    let on_approve = props.on_approve;
    let on_approve_always = props.on_approve_always;
    let allow_always = props.allow_always_label;

    div()
        .id("approval-card")
        .w_full()
        .max_w(px(Tokens::COMPOSER_MAX_WIDTH))
        .pb(Tokens::spacing_2())
        .child(
            div()
                .w_full()
                .px(Tokens::spacing_4())
                .py(Tokens::spacing_3())
                .rounded(Tokens::radius_lg())
                .bg(Tokens::surface())
                .border_1()
                .border_color(Tokens::approval_border())
                .shadow_sm()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_3())
                .child(
                    div()
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
                                .text_color(Tokens::text_tertiary())
                                .child(risk_label),
                        ),
                )
                .when(props.can_act, |el| {
                    el.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(Tokens::spacing_2())
                            .child(
                                btn_approve("approve-once", "Allow once")
                                    .on_click(move |_, _, app: &mut App| on_approve(app)),
                            )
                            .child(
                                btn_deny("deny-approval-card", "Deny")
                                    .on_click(move |_, _, app: &mut App| on_deny(app)),
                            )
                            .when_some(on_approve_always, |el, cb| {
                                el.when_some(allow_always, |el, label| {
                                    el.child(
                                        btn_approve("approve-always", label)
                                            .on_click(move |_, _, app: &mut App| cb(app)),
                                    )
                                })
                            }),
                    )
                }),
        )
}

/// Inline thread row when a tool awaits approval (actions live in the composer bar).
pub fn thread_approval_row(
    title: &str,
    risk: &ApprovalRisk,
    resolved: bool,
    approval_active: bool,
) -> impl IntoElement {
    let detail = if resolved {
        None
    } else if approval_active {
        Some(format!(
            "Review above the composer to continue · {}",
            risk.label()
        ))
    } else {
        None
    };
    div()
        .id("thread-approval-row")
        .w_full()
        .child(activity_action_line_with_loading(
            title,
            detail.as_deref(),
            !resolved && approval_active,
            false,
            approval_active,
            "approval-legacy",
        ))
}

/// Bottom bar inside the diff panel — title, risk, Reject / Approve.

/// Bottom bar inside the diff panel — title, risk, Reject / Approve.
pub fn diff_approval_bar(
    title: &str,
    risk: &ApprovalRisk,
    can_act: bool,
    on_reject: impl Fn(&mut App) + 'static,
    on_approve: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let title = title.to_string();
    let risk_label = risk.label().to_string();

    div()
        .id("diff-approval-bar")
        .flex_shrink_0()
        .px(Tokens::spacing_3())
        .py(Tokens::spacing_2())
        .border_t_1()
        .border_color(Tokens::border_subtle())
        .bg(Tokens::panel_bg())
        .child(
            div()
                .w_full()
                .px(Tokens::spacing_3())
                .py(Tokens::spacing_1p5())
                .rounded(Tokens::radius_full())
                .bg(Tokens::surface())
                .border_1()
                .border_color(Tokens::border_subtle())
                .flex()
                .items_center()
                .justify_between()
                .gap(Tokens::spacing_3())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(Tokens::spacing_0p5())
                        .child(
                            div()
                                .text_size(Tokens::text_sm())
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Tokens::text_primary())
                                .child(title),
                        )
                        .child(
                            div()
                                .text_size(Tokens::text_xs())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Tokens::text_tertiary())
                                .child(risk_label),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_shrink_0()
                        .items_center()
                        .gap(Tokens::spacing_1())
                        .when(can_act, |el| {
                            el.child(
                                btn_deny("deny-approval", "Not now")
                                    .on_click(move |_, _, app: &mut App| on_reject(app)),
                            )
                            .child(
                                btn_approve("approve-approval", "Continue")
                                    .on_click(move |_, _, app: &mut App| on_approve(app)),
                            )
                        }),
                ),
        )
}
