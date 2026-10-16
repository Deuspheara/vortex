//! Stable text input with a single model, permissions, attachment, and send toolbar.

use super::mode_chips::{ModeChipsProps, mode_chips};
use agent_protocol::AgentMode;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    Entity, ExternalPaths, Focusable, IntoElement, ObjectFit, StyledImage, Window, div, img,
    prelude::*, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState, Paste};

use crate::features::composer::model_selection::CodexModelSelection;
use crate::features::composer::state::PendingImageAttachment;
use crate::shared::components::buttons::{btn_disabled_icon, btn_primary_icon};
use crate::shared::components::dropdown::{
    DropdownAnchor, DropdownItem, PickerDropdownProps, picker_dropdown,
};
use crate::tokens::Tokens;
use crate::tokens::icons;
use crate::ui::agent_window::AgentWindow;

pub struct ComposerPillProps {
    pub codex: bool,
    pub selected_mode: AgentMode,
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
pub fn composer_input_needs_stacked_layout(_text: &str) -> bool {
    // A stable text-above-toolbar layout avoids controls moving as the user types.
    true
}

/// The main composer surface with stable text and toolbar regions.
pub fn composer_pill(props: ComposerPillProps) -> impl IntoElement {
    let ComposerPillProps {
        codex,
        selected_mode,
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

    let pill_min_height = px(Tokens::COMPOSER_MIN_HEIGHT);
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
        .border_color(Tokens::border_subtle())
        .flex()
        .flex_col()
        .gap(Tokens::spacing_1())
        .px(Tokens::spacing_3())
        .py(Tokens::spacing_2());

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
        codex,
        selected_mode,
    ))
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
    codex: bool,
    selected_mode: AgentMode,
) -> impl IntoElement {
    div()
        .id("composer-pill-stacked")
        .w_full()
        .min_h(px(Tokens::COMPOSER_MIN_HEIGHT))
        .flex()
        .flex_col()
        .gap(Tokens::spacing_1())
        .child(
            div()
                .w_full()
                .min_h(px(Tokens::COMPOSER_INPUT_MIN_HEIGHT))
                .max_h(px(Tokens::composer_input_max_height()))
                .overflow_hidden()
                .pt(Tokens::spacing_0p5())
                .child(render_input(input_entity)),
        )
        .child(
            div()
                .id("composer-pill-toolbar")
                .px(Tokens::spacing_2())
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
                        .gap(Tokens::spacing_2())
                        .flex_wrap()
                        .min_w(px(0.0))
                        .flex_1()
                        .child(if codex {
                            render_codex_model_pickers(selected_model, model_items, entity.clone())
                                .into_any_element()
                        } else {
                            render_model_picker(
                                selected_model,
                                model_items,
                                model_search_keys,
                                entity.clone(),
                            )
                            .into_any_element()
                        })
                        .child(mode_chips(ModeChipsProps {
                            codex,
                            selected_mode,
                            on_select: Rc::new({
                                let entity = entity.clone();
                                move |mode, app| {
                                    entity.update(app, |view, cx| view.set_safety_mode(mode, cx));
                                }
                            }),
                        })),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(Tokens::spacing_2())
                        .child(render_attach_button(entity))
                        .child(render_send_button(has_text, is_running, on_send, on_cancel)),
                ),
        )
}

fn render_attach_button(entity: Entity<AgentWindow>) -> impl IntoElement {
    div().flex_shrink_0().child(
        Button::new("composer-attach")
            .icon(icons::paperclip())
            .tooltip("Attach images")
            .ghost()
            .compact()
            .h(px(Tokens::ROW_HEIGHT_SM))
            .w(px(Tokens::ROW_HEIGHT_SM))
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
    let focus_input = input_entity.clone();
    div()
        .id("composer-input")
        .min_h(px(Tokens::COMPOSER_INPUT_MIN_HEIGHT))
        .min_w(px(0.0))
        .w_full()
        .overflow_hidden()
        .on_mouse_down(gpui::MouseButton::Left, move |_, window, app| {
            focus_input.read(app).focus_handle(app).focus(window);
        })
        .child(Input::new(input_entity).appearance(false))
}

fn render_codex_model_pickers(
    selected: &str,
    catalog: &[String],
    entity: Entity<AgentWindow>,
) -> impl IntoElement {
    let choices = CodexModelSelection::new(selected, catalog);
    div()
        .flex()
        .items_center()
        .flex_wrap()
        .gap(Tokens::spacing_2())
        .child(render_catalog_picker(
            "composer-codex-model",
            choices.model,
            choices.models,
            Some(icons::BOT),
            entity.clone(),
            true,
        ))
        .when_some(choices.effort, |el, effort| {
            el.child(render_catalog_picker(
                "composer-codex-effort",
                effort,
                choices.efforts,
                None,
                entity,
                false,
            ))
        })
}

fn render_catalog_picker(
    id: &'static str,
    selected: String,
    values: Vec<(String, String)>,
    icon: Option<gpui_component::IconName>,
    entity: Entity<AgentWindow>,
    searchable: bool,
) -> impl IntoElement {
    picker_dropdown(PickerDropdownProps {
        id: id.into(),
        label: selected.clone(),
        items: values
            .iter()
            .map(|(label, _)| DropdownItem {
                label: label.clone(),
                icon: None,
            })
            .collect(),
        selected: Some(selected),
        anchor: DropdownAnchor::Above,
        menu_min_width: Tokens::COMPOSER_MODEL_MAX_WIDTH,
        trigger_icon: icon.map(Into::into),
        searchable,
        search_texts: None,
        search_placeholder: Some("Search models…".into()),
        on_select: Rc::new(move |index, _, app| {
            if let Some((_, value)) = values.get(index) {
                entity.update(app, |view, cx| view.on_model_selected(value.clone(), cx));
            }
        }),
    })
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
        .max_w(px(Tokens::COMPOSER_MODEL_MAX_WIDTH))
        .child(picker_dropdown(PickerDropdownProps {
            id: "composer-model".into(),
            label: selected.clone(),
            items,
            selected: Some(selected),
            anchor: DropdownAnchor::Above,
            menu_min_width: 220.0,
            trigger_icon: Some(icons::BOT.into()),
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
