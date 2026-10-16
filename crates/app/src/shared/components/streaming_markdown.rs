//! Streaming assistant markdown — sealed blocks + cheap live tail.
//!
//! During stream: finalized blocks render as cached markdown; the unstable tail
//! renders as plain text or a monospace code fence (no syntax highlight).

use std::sync::Arc;

use gpui::{IntoElement, div, prelude::*, px};

use crate::features::chat::components::message::static_streaming_cursor;
use crate::shared::components::markdown_preview::{
    MarkdownBlock, markdown_preview_blocks_shared_streaming,
};
use crate::tokens::{Tokens, element_key};

/// Split markdown at the last safe seal boundary (outside code fences).
///
/// Safe boundaries: blank line between paragraphs, closed code fence, end of complete table/list
/// followed by blank line.
pub fn split_at_seal_boundary(source: &str) -> (&str, &str) {
    if source.is_empty() {
        return ("", "");
    }

    let mut in_fence = false;
    let mut last_seal = 0usize;
    let bytes = source.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        if i + 2 < bytes.len() && bytes[i] == b'`' && bytes[i + 1] == b'`' && bytes[i + 2] == b'`' {
            in_fence = !in_fence;
            i += 3;
            if !in_fence {
                while i < bytes.len() && bytes[i] == b'\n' {
                    i += 1;
                }
                last_seal = i;
            }
            continue;
        }

        if !in_fence && i + 1 < bytes.len() && bytes[i] == b'\n' && bytes[i + 1] == b'\n' {
            i += 2;
            last_seal = i;
            continue;
        }

        i += 1;
    }

    if last_seal == 0 {
        return ("", source);
    }

    let (sealed, tail) = source.split_at(last_seal);
    (sealed, tail)
}

/// Cached sealed blocks for a streaming assistant message.
#[derive(Clone, Default)]
pub struct SealedBlockCache {
    pub sealed_end: usize,
    pub blocks: Arc<[MarkdownBlock]>,
    pub content_hash: u64,
    pub height: f32,
}

enum LiveRenderMode {
    Plain,
    OpenCodeFence { lang: Option<String> },
}

fn live_render_mode(tail: &str) -> LiveRenderMode {
    let mut fence_lang: Option<String> = None;
    for line in tail.lines() {
        if line.starts_with("```") {
            if fence_lang.is_none() {
                let lang = line.trim_start_matches('`').trim();
                fence_lang = if lang.is_empty() {
                    Some(String::new())
                } else {
                    Some(lang.to_string())
                };
            } else {
                return LiveRenderMode::Plain;
            }
        }
    }

    if let Some(lang) = fence_lang {
        LiveRenderMode::OpenCodeFence {
            lang: if lang.is_empty() { None } else { Some(lang) },
        }
    } else {
        LiveRenderMode::Plain
    }
}

fn open_fence_body(tail: &str) -> &str {
    let mut in_fence = false;
    let mut body_start = 0usize;
    let mut line_start = 0usize;
    while line_start < tail.len() {
        let line_end = tail[line_start..]
            .find('\n')
            .map(|offset| line_start + offset)
            .unwrap_or(tail.len());
        let line = &tail[line_start..line_end];
        if line.starts_with("```") {
            if !in_fence {
                in_fence = true;
                body_start = line_end.saturating_add(1).min(tail.len());
            } else {
                break;
            }
        }
        line_start = line_end.saturating_add(1);
    }
    if in_fence && body_start <= tail.len() {
        &tail[body_start..]
    } else {
        tail
    }
}

/// Cheap live tail — plain text or open monospace code block, no markdown AST.
pub fn render_live_tail(tail: &str) -> impl IntoElement {
    if tail.is_empty() {
        return div().into_any_element();
    }

    match live_render_mode(tail) {
        LiveRenderMode::Plain => div()
            .w_full()
            .font_family(Tokens::ui_font_family())
            .text_size(Tokens::text_md())
            .line_height(Tokens::text_md_leading())
            .text_color(Tokens::text_primary())
            .child(tail.to_string())
            .into_any_element(),
        LiveRenderMode::OpenCodeFence { lang } => {
            let body = open_fence_body(tail);
            let label = lang.as_deref().unwrap_or("text");
            div()
                .w_full()
                .min_w(px(0.0))
                .rounded(Tokens::radius_md())
                .border_1()
                .border_color(Tokens::border_strong())
                .bg(Tokens::code_bg())
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(
                    div()
                        .h(px(Tokens::CODE_HEADER_HEIGHT))
                        .flex_shrink_0()
                        .px(Tokens::spacing_3())
                        .flex()
                        .items_center()
                        .font_family(Tokens::ui_font_family())
                        .text_size(Tokens::text_label())
                        .text_color(Tokens::text_tertiary())
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .id("streaming-code-body")
                        .w_full()
                        .min_w(px(0.0))
                        .overflow_x_scroll()
                        .px(Tokens::spacing_3())
                        .pb(Tokens::spacing_3())
                        .font_family(Tokens::code_font_family())
                        .text_size(Tokens::text_code())
                        .text_color(Tokens::code_fg())
                        .line_height(px(Tokens::DIFF_LINE_HEIGHT))
                        .children(body.lines().map(|line| {
                            div()
                                .h(px(Tokens::DIFF_LINE_HEIGHT))
                                .flex_shrink_0()
                                .whitespace_nowrap()
                                .child(line.to_string())
                        })),
                )
                .into_any_element()
        }
    }
}

/// Retain the prefix while its first background parse is pending. Rendering only
/// the open-fence tail would otherwise drop prose from a single-chunk update.
pub(crate) fn streaming_text_parts(source: &str, sealed_end: usize) -> (&str, &str) {
    if sealed_end > 0 {
        if let Some(tail) = source.get(sealed_end..) {
            return ("", tail);
        }
    }
    split_at_seal_boundary(source)
}

/// Streaming assistant body: sealed markdown blocks + live tail + cursor.
pub fn streaming_assistant_body(
    id: &str,
    source: &str,
    sealed: &SealedBlockCache,
    show_cursor: bool,
) -> impl IntoElement {
    let (pending_prefix, tail) = streaming_text_parts(source, sealed.sealed_end);
    let has_sealed = sealed.sealed_end > 0;

    div()
        .id(element_key("assistant-stream", id))
        .w_full()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .when(has_sealed, |el| {
            el.child(markdown_preview_blocks_shared_streaming(
                Arc::clone(&sealed.blocks),
                true,
                false,
            ))
        })
        .when(!pending_prefix.is_empty(), |el| {
            el.child(render_live_tail(pending_prefix))
        })
        .when(!tail.is_empty(), |el| el.child(render_live_tail(tail)))
        .when(show_cursor, |el| el.child(static_streaming_cursor()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_update_keeps_prose_before_open_code_until_parse_finishes() {
        let source = "Introduction.\n\n```rust\nfn main() {}";
        let (prefix, tail) = streaming_text_parts(source, 0);
        assert_eq!(prefix, "Introduction.\n\n");
        assert_eq!(tail, "```rust\nfn main() {}");
        assert_eq!(streaming_text_parts(source, prefix.len()), ("", tail));
    }

    #[test]
    fn seal_at_paragraph_boundary() {
        let (sealed, tail) = split_at_seal_boundary("Hello world.\n\nStill typing");
        assert_eq!(sealed, "Hello world.\n\n");
        assert_eq!(tail, "Still typing");
    }

    #[test]
    fn seal_after_closed_fence() {
        let src = "Intro\n\n```rust\nfn main() {}\n```\n\nNext";
        let (sealed, tail) = split_at_seal_boundary(src);
        assert!(sealed.contains("```"));
        assert_eq!(tail, "Next");
    }

    #[test]
    fn open_fence_stays_in_tail() {
        let src = "Text\n\n```python\nprint('hi')";
        let (sealed, tail) = split_at_seal_boundary(src);
        assert_eq!(sealed, "Text\n\n");
        assert!(tail.starts_with("```python"));
    }
}
