//! Stateless message components — full-width, minimal chrome.

use std::sync::Arc;

use crate::features::shell::state::{MessageAttachment, USER_MESSAGE_PREVIEW_LINES};
use crate::shared::components::buttons::btn_copy_icon_arc;
use crate::shared::components::markdown_preview::LINE_LEADING;
use crate::tokens::{Tokens, braille_spinner, element_key};
use gpui::{
    App, Image, ImageFormat, IntoElement, ObjectFit, StyledImage, div, img, prelude::*, px,
};

/// Preferred bubble width; the parent limits it to the actual transcript width.
pub fn user_message_width(text: &str, has_attachments: bool, app: &App) -> gpui::Pixels {
    let max_width = px(Tokens::THREAD_MAX_WIDTH * 0.88);
    let text_width = if text.len() > 128 {
        max_width
    } else {
        let text_system = app.text_system();
        let font = text_system.resolve_font(&gpui::font(Tokens::ui_font_family()));
        text.lines()
            .map(|line| {
                line.chars().fold(px(0.0), |width, character| {
                    width
                        + text_system
                            .advance(font, Tokens::text_md(), character)
                            .map(|advance| advance.width)
                            .unwrap_or(Tokens::text_md())
                })
            })
            .max()
            .unwrap_or_default()
    };
    let content_width = if !has_attachments {
        text_width
    } else {
        text_width.max(Tokens::attachment_preview_size())
    };
    (content_width + Tokens::spacing_3() * 2.0 + Tokens::spacing_6()).min(max_width)
}

/// User turn — flatter Zed-like row with just a subtle accent wash.
pub fn user_message(
    text: &str,
    attachments: &[MessageAttachment],
    id: &str,
    body_height: gpui::Pixels,
    collapsed: bool,
    show_see_more: bool,
    app: &App,
    on_toggle: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let bubble_width = user_message_width(text, !attachments.is_empty(), app);
    let text = text.to_string();
    let copy_text = std::sync::Arc::<str>::from(text.as_str());
    let preview_max_h = px(USER_MESSAGE_PREVIEW_LINES as f32 * LINE_LEADING);
    let hover_group = format!("user-message-hover-{id}");
    div()
        .id(element_key("user-message", id))
        .w(bubble_width)
        .h(body_height)
        .flex_shrink_0()
        .max_w(gpui::relative(1.0))
        .relative()
        .flex()
        .flex_col()
        .items_start()
        .gap(Tokens::spacing_1())
        .p(Tokens::spacing_3())
        .rounded(Tokens::radius_md())
        .bg(Tokens::user_message_bg())
        .min_w(px(0.0))
        .group(hover_group.clone())
        .child(
            div()
                .pr(Tokens::spacing_6())
                .text_size(Tokens::text_md())
                .line_height(Tokens::text_md_leading())
                .text_color(Tokens::text_primary())
                .when(collapsed, |el| el.max_h(preview_max_h).overflow_hidden())
                .child(text.clone()),
        )
        .when(!attachments.is_empty(), |el| {
            el.child(
                div()
                    .w_full()
                    .flex()
                    .flex_wrap()
                    .gap(Tokens::spacing_1())
                    .children(attachments.iter().map(|attachment| {
                        div()
                            .w(Tokens::attachment_preview_size())
                            .h(Tokens::attachment_preview_size())
                            .overflow_hidden()
                            .rounded(Tokens::radius_sm())
                            .border_1()
                            .border_color(Tokens::border_subtle())
                            .bg(Tokens::surface_active())
                            .child(render_message_attachment_preview(attachment))
                    })),
            )
        })
        .when(show_see_more, |el| {
            el.child(
                div()
                    .id(element_key("user-message-see-more", id))
                    .text_size(Tokens::text_xs())
                    .line_height(Tokens::text_sm_leading_compact())
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
                .top(px(0.0))
                .right(px(0.0))
                .opacity(0.0)
                .group_hover(hover_group, |s| s.opacity(1.0))
                .child(btn_copy_icon_arc(
                    element_key("copy-user", id),
                    copy_text,
                    "Copy message",
                )),
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
