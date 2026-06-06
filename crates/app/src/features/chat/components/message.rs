//! Stateless message components — full-width, minimal chrome.

use std::sync::Arc;

use crate::features::shell::state::{MessageAttachment, USER_MESSAGE_PREVIEW_LINES};
use crate::shared::components::buttons::btn_copy_icon_arc;
use crate::shared::components::markdown_preview::LINE_LEADING;
use crate::tokens::{Tokens, braille_spinner, element_key};
use gpui::{
    App, Image, ImageFormat, IntoElement, ObjectFit, StyledImage, div, img, prelude::*, px,
};

/// User turn — accent bubble on flat thread surface.
pub fn user_message(
    text: &str,
    attachments: &[MessageAttachment],
    id: &str,
    collapsed: bool,
    show_see_more: bool,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let text = text.to_string();
    let copy_text = std::sync::Arc::<str>::from(text.as_str());
    let preview_max_h = px(USER_MESSAGE_PREVIEW_LINES as f32 * LINE_LEADING);
    div()
        .id(element_key("user-message", id))
        .w_full()
        .flex()
        .items_end()
        .child(
            div()
                .max_w(px(Tokens::THREAD_MAX_WIDTH * 0.82))
                .relative()
                .flex()
                .flex_col()
                .items_start()
                .gap(Tokens::spacing_1())
                .px(Tokens::spacing_3())
                .py(Tokens::spacing_2())
                .rounded(Tokens::radius_md())
                .bg(Tokens::accent().opacity(0.10))
                .text_size(Tokens::text_md())
                .line_height(Tokens::text_md_leading())
                .text_color(Tokens::text_primary())
                .child(
                    div()
                        .w_full()
                        .when(collapsed, |el| el.max_h(preview_max_h).overflow_hidden())
                        .child(text.clone()),
                )
                .when(!attachments.is_empty(), |el| {
                    el.child(div().flex().flex_wrap().gap(Tokens::spacing_1()).children(
                        attachments.iter().map(|attachment| {
                            div()
                                .w(Tokens::attachment_preview_size())
                                .h(Tokens::attachment_preview_size())
                                .overflow_hidden()
                                .rounded(Tokens::radius_sm())
                                .border_1()
                                .border_color(Tokens::border_subtle())
                                .bg(Tokens::surface_active())
                                .child(render_message_attachment_preview(attachment))
                        }),
                    ))
                })
                .when(show_see_more, |el| {
                    el.child(
                        div()
                            .id(element_key("user-message-see-more", id))
                            .text_size(Tokens::text_xs())
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(Tokens::accent())
                            .cursor_pointer()
                            .hover(|s| s.text_color(Tokens::accent().opacity(0.8)))
                            .child(if collapsed { "See more" } else { "See less" })
                            .on_click(move |_, _, app: &mut gpui::App| on_toggle(app)),
                    )
                })
                .child(
                    div()
                        .absolute()
                        .top(Tokens::spacing_1())
                        .right(Tokens::spacing_1())
                        .opacity(0.0)
                        .hover(|s| s.opacity(1.0))
                        .child(btn_copy_icon_arc(
                            element_key("copy-user", id),
                            copy_text,
                            "Copy message",
                        )),
                ),
        )
}

/// Subtle stream indicator at the end of an in-progress assistant reply.
#[allow(dead_code)]
pub fn streaming_cursor(animate: bool) -> impl IntoElement {
    div()
        .mt(Tokens::spacing_0p5())
        .text_size(Tokens::text_sm())
        .text_color(Tokens::text_faint())
        .opacity(0.6)
        .child(braille_spinner("streaming-cursor", animate))
}

/// Plain-text body for a streaming assistant reply — no markdown parse.
pub fn streaming_assistant_plain(text: &str) -> impl IntoElement {
    div()
        .w_full()
        .font_family(Tokens::ui_font_family())
        .text_size(Tokens::text_md())
        .line_height(Tokens::text_md_leading())
        .text_color(Tokens::text_primary())
        .child(text.to_string())
}

/// Static cursor for streaming — avoids braille animation repainting at display rate.
pub fn static_streaming_cursor() -> impl IntoElement {
    div()
        .mt(Tokens::spacing_0p5())
        .text_size(Tokens::text_sm())
        .text_color(Tokens::text_faint())
        .opacity(0.6)
        .child("▍")
}

fn render_message_attachment_preview(attachment: &MessageAttachment) -> gpui::AnyElement {
    match &attachment.preview {
        crate::features::shell::state::MessageAttachmentPreview::File(path) => img(path.clone())
            .w_full()
            .h_full()
            .object_fit(ObjectFit::Cover)
            .with_fallback(render_message_attachment_fallback)
            .into_any_element(),
        crate::features::shell::state::MessageAttachmentPreview::Bytes { mime_type, bytes } => {
            ImageFormat::from_mime_type(mime_type)
                .map(|format| Arc::new(Image::from_bytes(format, bytes.clone())))
                .map(|image| {
                    img(image)
                        .w_full()
                        .h_full()
                        .object_fit(ObjectFit::Cover)
                        .with_fallback(render_message_attachment_fallback)
                        .into_any_element()
                })
                .unwrap_or_else(render_message_attachment_fallback)
        }
    }
}

fn render_message_attachment_fallback() -> gpui::AnyElement {
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
