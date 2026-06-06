//! Composer pill — the primary thin input surface.
//!
//! A compact, pill-shaped text input with attach and send controls.
//! Height is ~48 px when single-line; auto-grows on multi-line input.
//! The model dropdown sits between the text input and the send button.
//! When text wraps past the inline width, controls move to a row below the input.
//! No mode, provider, or approval controls — those live in sibling rows.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    Entity, ExternalPaths, IntoElement, ObjectFit, StyledImage, Window, div, img, prelude::*, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState, Paste};

use crate::features::composer::state::PendingImageAttachment;
use crate::shared::components::buttons::{btn_disabled_icon, btn_primary_icon};
use crate::shared::components::dropdown::{
    DropdownAnchor, DropdownItem, PickerDropdownProps, picker_dropdown,
};
use crate::tokens::Tokens;
use crate::tokens::icons;
use crate::ui::agent_window::AgentWindow;

pub struct ComposerPillProps {
    pub has_text: bool,
    pub input_expanded: bool,
    pub input_entity: Entity<InputState>,
    pub is_running: bool,
    pub on_send: Option<Box<dyn Fn(&mut Window, &mut gpui::App) + 'static>>,
    pub on_cancel: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    pub selected_model: String,
    pub model_items: Arc<[String]>,
    pub model_search_keys: Arc<[Arc<str>]>,
    pub pending_image_attachments: Vec<PendingImageAttachment>,
    pub composer_error: Option<String>,
    pub entity: Entity<AgentWindow>,
}

/// True when the input should use the stacked layout (text above, toolbar below).
pub fn composer_input_needs_stacked_layout(text: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }
    if text.contains('\n') {
        return true;
    }
    let limit = Tokens::COMPOSER_INLINE_CHARS_PER_LINE;
    text.lines().any(|line| line.chars().count() > limit)
}

/// The main input pill — thin, rounded, focused on typing.
pub fn composer_pill(props: ComposerPillProps) -> impl IntoElement {
    let ComposerPillProps {
        has_text,
        input_expanded,
        input_entity,
        is_running,
        on_send,
        on_cancel,
        selected_model,
        model_items,
        model_search_keys,
        pending_image_attachments,
        composer_error,
        entity,
    } = props;
    let has_attachments = !pending_image_attachments.is_empty();
    let has_error = composer_error.is_some();

    let pill_min_height = px(Tokens::COMPOSER_INPUT_MIN_HEIGHT);
    let pill_max_height = px(if input_expanded {
        Tokens::composer_pill_stacked_max_height(has_attachments, has_error)
    } else {
        Tokens::composer_pill_inline_max_height(has_attachments, has_error)
    });
    let shell = div()
        .id("composer-pill")
        .w_full()
        .max_w(px(Tokens::COMPOSER_MAX_WIDTH))
        .min_h(pill_min_height)
        .max_h(pill_max_height)
        .overflow_hidden()
        .rounded(Tokens::radius_composer())
        .bg(Tokens::composer_bg())
        .border_1()
        .border_color(Tokens::composer_border())
        .flex()
        .flex_col()
        .gap(Tokens::spacing_1())
        .px(Tokens::spacing_2())
        .py(Tokens::spacing_1());

    let drop_entity = entity.clone();
    let paste_entity = entity.clone();
    let shell = shell
        .on_drop(move |paths: &ExternalPaths, _, app: &mut gpui::App| {
            let paths = paths.paths().to_vec();
            drop_entity.update(app, |view, cx| {
                view.add_image_attachment_paths(paths, cx);
            });
        })
        .on_action(move |_: &Paste, _, app: &mut gpui::App| {
            let handled =
                paste_entity.update(app, |view, cx| view.add_clipboard_image_attachment(cx));
            if handled {
                app.stop_propagation();
            }
        })
        .when(!pending_image_attachments.is_empty(), |el| {
            el.child(render_attachment_strip(
                &pending_image_attachments,
                entity.clone(),
            ))
        })
        .when_some(composer_error, |el, error| {
            el.child(
                div()
                    .px(Tokens::spacing_2())
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::danger())
                    .child(error),
            )
        });

    if input_expanded {
        shell.child(stacked_pill_body(
            &input_entity,
            &selected_model,
            &model_items,
            &model_search_keys,
            entity,
            has_text,
            is_running,
            on_send,
            on_cancel,
        ))
    } else {
        shell.child(inline_pill_body(
            &input_entity,
            &selected_model,
            &model_items,
            &model_search_keys,
            entity,
            has_text,
            is_running,
            on_send,
            on_cancel,
        ))
    }
}

fn inline_pill_body(
    input_entity: &Entity<InputState>,
    selected_model: &str,
    model_items: &[String],
    model_search_keys: &[Arc<str>],
    entity: Entity<AgentWindow>,
    has_text: bool,
    is_running: bool,
    on_send: Option<Box<dyn Fn(&mut Window, &mut gpui::App) + 'static>>,
    on_cancel: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
) -> impl IntoElement {
    div()
        .id("composer-pill-inline")
        .w_full()
        .flex()
        .items_center()
        .gap(Tokens::spacing_1())
        .child(render_attach_button(entity.clone()))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .min_h(px(0.0))
                .max_h(px(Tokens::composer_input_max_height()))
                .overflow_hidden()
                .child(render_input(input_entity)),
        )
        .child(render_model_picker(
            selected_model,
            model_items,
            model_search_keys,
            entity,
        ))
        .child(render_send_button(has_text, is_running, on_send, on_cancel))
}

fn stacked_pill_body(
    input_entity: &Entity<InputState>,
    selected_model: &str,
    model_items: &[String],
    model_search_keys: &[Arc<str>],
    entity: Entity<AgentWindow>,
    has_text: bool,
    is_running: bool,
    on_send: Option<Box<dyn Fn(&mut Window, &mut gpui::App) + 'static>>,
    on_cancel: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
) -> impl IntoElement {
    div()
        .id("composer-pill-stacked")
        .w_full()
        .h_full()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .gap(Tokens::spacing_1())
        .child(
            div()
                .w_full()
                .flex_1()
                .min_h(px(0.0))
                .max_h(px(Tokens::composer_input_max_height()))
                .overflow_hidden()
                .pt(Tokens::spacing_0p5())
                .child(render_input(input_entity)),
        )
        .child(
            div()
                .id("composer-pill-toolbar")
                .w_full()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_between()
                .gap(Tokens::spacing_2())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(Tokens::spacing_1())
                        .min_w(px(0.0))
                        .flex_1()
                        .child(render_attach_button(entity.clone()))
                        .child(render_model_picker(
                            selected_model,
                            model_items,
                            model_search_keys,
                            entity,
                        )),
                )
                .child(render_send_button(has_text, is_running, on_send, on_cancel)),
        )
}

fn render_attach_button(entity: Entity<AgentWindow>) -> impl IntoElement {
    div().flex_shrink_0().child(
        Button::new("composer-attach")
            .icon(icons::PLUS)
            .ghost()
            .compact()
            .h(px(28.0))
            .w(px(28.0))
            .rounded(Tokens::radius_full())
            .on_click(move |_, window, app: &mut gpui::App| {
                entity.update(app, |view, cx| {
                    view.open_image_attachment_picker(window, cx);
                });
            }),
    )
}

fn render_attachment_strip(
    attachments: &[PendingImageAttachment],
    entity: Entity<AgentWindow>,
) -> impl IntoElement {
    div()
        .id("composer-attachments")
        .flex()
        .flex_wrap()
        .gap(Tokens::spacing_1())
        .children(attachments.iter().map(|attachment| {
            let id = attachment.id.clone();
            let remove_entity = entity.clone();
            div()
                .id(crate::tokens::element_key(
                    "composer-attachment",
                    &attachment.id,
                ))
                .relative()
                .w(Tokens::attachment_preview_size())
                .h(Tokens::attachment_preview_size())
                .overflow_hidden()
                .rounded(Tokens::radius_sm())
                .border_1()
                .border_color(Tokens::border_subtle())
                .bg(Tokens::surface_active())
                .child(render_pending_attachment_preview(attachment))
                .child(
                    div()
                        .absolute()
                        .top(Tokens::spacing_0p5())
                        .right(Tokens::spacing_0p5())
                        .rounded(Tokens::radius_full())
                        .bg(Tokens::surface_overlay().opacity(0.92))
                        .child(
                            Button::new(crate::tokens::element_key(
                                "remove-composer-attachment",
                                &attachment.id,
                            ))
                            .icon(icons::CLOSE)
                            .ghost()
                            .compact()
                            .h(Tokens::spacing_5())
                            .w(Tokens::spacing_5())
                            .on_click(
                                move |_, _, app: &mut gpui::App| {
                                    remove_entity.update(app, |view, cx| {
                                        view.remove_image_attachment(&id, cx);
                                    });
                                },
                            ),
                        ),
                )
        }))
}

fn render_pending_attachment_preview(attachment: &PendingImageAttachment) -> gpui::AnyElement {
    match &attachment.source {
        crate::features::composer::state::PendingImageSource::File(path) => img(path.clone())
            .w_full()
            .h_full()
            .object_fit(ObjectFit::Cover)
            .with_fallback(render_attachment_preview_fallback)
            .into_any_element(),
        crate::features::composer::state::PendingImageSource::Clipboard(_) => attachment
            .preview_image()
            .map(|image| {
                img(image)
                    .w_full()
                    .h_full()
                    .object_fit(ObjectFit::Cover)
                    .with_fallback(render_attachment_preview_fallback)
                    .into_any_element()
            })
            .unwrap_or_else(render_attachment_preview_fallback),
    }
}

fn render_attachment_preview_fallback() -> gpui::AnyElement {
    div()
        .w_full()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(Tokens::surface_active())
        .text_color(Tokens::text_tertiary())
        .child("IMG")
        .into_any_element()
}

fn render_input(input_entity: &Entity<InputState>) -> impl IntoElement {
    div()
        .id("composer-input")
        .h_full()
        .min_h(px(0.0))
        .min_w(px(0.0))
        .w_full()
        .overflow_hidden()
        .child(Input::new(input_entity).appearance(false))
}

fn render_model_picker(
    selected_model: &str,
    model_items: &[String],
    model_search_keys: &[Arc<str>],
    entity: Entity<AgentWindow>,
) -> impl IntoElement {
    let selected = selected_model.to_string();
    let mut deduped = model_items.to_vec();
    let mut search_keys = model_search_keys.to_vec();
    if !selected.is_empty() && !deduped.iter().any(|item| item == &selected) {
        deduped.insert(0, selected.clone());
        search_keys.insert(0, Arc::from(selected.to_lowercase()));
    }
    let items: Vec<DropdownItem> = deduped
        .iter()
        .map(|name| DropdownItem {
            label: name.clone(),
            icon: Some(icons::BOT),
        })
        .collect();

    div()
        .flex_shrink_0()
        .max_w(px(180.0))
        .child(picker_dropdown(PickerDropdownProps {
            id: "composer-model".into(),
            label: selected.clone(),
            items,
            selected: Some(selected),
            anchor: DropdownAnchor::Above,
            menu_min_width: 220.0,
            trigger_icon: Some(icons::BOT),
            searchable: true,
            search_texts: Some(search_keys),
            search_placeholder: Some("Search models…".into()),
            on_select: Rc::new(move |_, model, app| {
                entity.update(app, |view, cx| {
                    view.on_model_selected(model, cx);
                });
            }),
        }))
}

// ── Send / stop button ──

fn render_send_button(
    has_text: bool,
    is_running: bool,
    on_send: Option<Box<dyn Fn(&mut Window, &mut gpui::App) + 'static>>,
    on_cancel: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
) -> impl IntoElement {
    if is_running {
        if let Some(cb) = on_cancel {
            return btn_primary_icon("cancel-run", icons::X_MARK)
                .on_click(move |_, _, app: &mut gpui::App| cb(app))
                .into_any_element();
        }
        return btn_primary_icon("cancel-run-disabled", icons::X_MARK).into_any_element();
    }
    if has_text {
        if let Some(cb) = on_send {
            btn_primary_icon("send-message", icons::ARROW_UP)
                .on_click(move |_, window, app: &mut gpui::App| cb(window, app))
                .into_any_element()
        } else {
            btn_primary_icon("send-message", icons::ARROW_UP).into_any_element()
        }
    } else {
        btn_disabled_icon("send-message-disabled", icons::ARROW_UP).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stacked_when_text_wraps_inline_width() {
        let long = "a".repeat(Tokens::COMPOSER_INLINE_CHARS_PER_LINE + 1);
        assert!(composer_input_needs_stacked_layout(&long));
    }

    #[test]
    fn inline_when_short_text() {
        assert!(!composer_input_needs_stacked_layout("hello"));
        assert!(!composer_input_needs_stacked_layout(""));
    }

    #[test]
    fn stacked_on_explicit_newline() {
        assert!(composer_input_needs_stacked_layout("line one\nline two"));
    }
}
