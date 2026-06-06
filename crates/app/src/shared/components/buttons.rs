//! Themed button primitives — soft, rounded variants across the app.

use gpui::{ElementId, SharedString, Styled, px};
use gpui_component::Disableable;
use gpui_component::IconName;
use gpui_component::Sizable;
use gpui_component::Size;
use gpui_component::button::{Button, ButtonRounded, ButtonVariants};

use crate::tokens::Tokens;
use crate::tokens::icons;

fn radius_pill() -> ButtonRounded {
    ButtonRounded::Size(Tokens::radius_full())
}

#[allow(dead_code)]
fn radius_lg() -> ButtonRounded {
    ButtonRounded::Size(Tokens::radius_lg())
}

fn radius_md() -> ButtonRounded {
    ButtonRounded::Size(Tokens::radius_md())
}

fn radius_sm() -> ButtonRounded {
    ButtonRounded::Size(Tokens::radius_sm())
}

/// Compact ghost button for tertiary actions (sidebar toggles, icon-only controls).
pub fn btn_ghost(id: impl Into<ElementId>) -> Button {
    Button::new(id).ghost().compact().rounded(radius_md())
}

pub fn btn_ghost_label(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    btn_ghost(id).label(label)
}

pub fn btn_ghost_icon(id: impl Into<ElementId>, icon: IconName) -> Button {
    btn_ghost(id).icon(icon)
}

/// Top bar / secondary action — pill with subtle fill.
pub fn btn_outline(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id)
        .label(label)
        .outline()
        .compact()
        .with_size(Size::Small)
        .h(px(Tokens::ROW_HEIGHT_SM))
        .rounded(radius_pill())
}

/// Compact icon-only ghost control (e.g. project-row actions).
pub fn btn_icon_sm(id: impl Into<ElementId>, icon: IconName) -> Button {
    Button::new(id)
        .icon(icon)
        .ghost()
        .compact()
        .rounded(radius_sm())
}

/// Copy-to-clipboard icon button.
#[allow(dead_code)]
pub fn btn_copy_icon(
    id: impl Into<ElementId>,
    text: impl Into<String>,
    tooltip: impl Into<SharedString>,
) -> Button {
    btn_copy_icon_arc(id, std::sync::Arc::from(text.into().as_str()), tooltip)
}

/// Copy-to-clipboard icon button — clones text only when clicked.
pub fn btn_copy_icon_arc(
    id: impl Into<ElementId>,
    text: std::sync::Arc<str>,
    tooltip: impl Into<SharedString>,
) -> Button {
    let tooltip = tooltip.into();
    btn_icon_sm(id, icons::COPY)
        .tooltip(tooltip)
        .on_click(move |_, _, app: &mut gpui::App| {
            app.write_to_clipboard(text.to_string().into());
        })
}

/// Primary filled button — pill-shaped for CTAs.
#[allow(dead_code)]
pub fn btn_primary(id: impl Into<ElementId>) -> Button {
    Button::new(id).primary().compact().rounded(radius_lg())
}

#[allow(dead_code)]
pub fn btn_primary_label(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    btn_primary(id).label(label)
}

/// Approval deny — transparent with border.
pub fn btn_deny(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id)
        .label(label)
        .outline()
        .compact()
        .with_size(Size::Small)
        .h(px(Tokens::ROW_HEIGHT_SM))
        .px(Tokens::spacing_2p5())
        .rounded(radius_pill())
}

/// Approval approve — soft white fill.
pub fn btn_approve(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id)
        .label(label)
        .primary()
        .compact()
        .with_size(Size::Small)
        .h(px(Tokens::ROW_HEIGHT_SM))
        .px(Tokens::spacing_2p5())
        .rounded(radius_pill())
}

/// Circular send / icon CTA.
pub fn btn_primary_icon(id: impl Into<ElementId>, icon: IconName) -> Button {
    Button::new(id)
        .icon(icon)
        .primary()
        .compact()
        .h(px(Tokens::ROW_HEIGHT_MD))
        .w(px(Tokens::ROW_HEIGHT_MD))
        .rounded(radius_pill())
}

pub fn btn_disabled_icon(id: impl Into<ElementId>, icon: IconName) -> Button {
    Button::new(id)
        .icon(icon)
        .compact()
        .h(px(Tokens::ROW_HEIGHT_MD))
        .w(px(Tokens::ROW_HEIGHT_MD))
        .rounded(radius_pill())
        .disabled(true)
}

#[allow(dead_code)]
pub fn btn_sidebar_action(
    id: impl Into<ElementId>,
    icon: IconName,
    label: impl Into<SharedString>,
) -> Button {
    Button::new(id)
        .icon(icon)
        .label(label)
        .ghost()
        .compact()
        .rounded(radius_md())
}
