//! Streaming assistant markdown — sealed blocks + cheap live tail.
//!
//! During stream: finalized blocks render as cached markdown; the unstable tail
//! renders as plain text or a monospace code fence (no syntax highlight).

use std::sync::Arc;

use gpui::{IntoElement, div, prelude::*, px};

use crate::features::chat::components::message::static_streaming_cursor;
use crate::shared::components::markdown_preview::{
    MarkdownBlock, markdown_preview_blocks_shared_streaming, parse_markdown_blocks_shared_streaming,
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
}

impl SealedBlockCache {
    pub fn update(&mut self, source: &str) -> Arc<[MarkdownBlock]> {
        let (sealed, _) = split_at_seal_boundary(source);
        let sealed_len = sealed.len();
        if sealed_len <= self.sealed_end {
            return Arc::clone(&self.blocks);
        }

        let hash = blake3_seal_hash(sealed);
        if hash == self.content_hash && sealed_len == self.sealed_end {
            return Arc::clone(&self.blocks);
        }

        self.sealed_end = sealed_len;
        self.content_hash = hash;
        self.blocks = parse_markdown_blocks_shared_streaming(sealed, false);
        Arc::clone(&self.blocks)
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

fn blake3_seal_hash(sealed: &str) -> u64 {
    let hash = blake3::hash(sealed.as_bytes());
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hash.as_bytes()[..8]);
    u64::from_le_bytes(bytes)
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
    for (i, line) in tail.lines().enumerate() {
        if line.starts_with("```") {
            if !in_fence {
                in_fence = true;
                body_start = tail.lines().take(i + 1).map(|l| l.len() + 1).sum();
            } else {
                break;
            }
        }
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
            let label = lang.as_deref().unwrap_or("code");
            div()
                .w_full()
                .rounded(Tokens::radius_sm())
                .border_1()
                .border_color(Tokens::border_subtle())
                .overflow_hidden()
                .child(
                    div()
                        .px(Tokens::spacing_2())
                        .py(Tokens::spacing_0p5())
                        .bg(Tokens::surface_hover())
                        .border_b_1()
                        .border_color(Tokens::border_subtle())
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_tertiary())
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .w_full()
                        .px(Tokens::spacing_2())
                        .py(Tokens::spacing_1())
                        .bg(Tokens::code_bg())
                        .font_family(Tokens::terminal_font_family())
                        .text_size(Tokens::text_code())
                        .text_color(Tokens::code_fg())
                        .line_height(px(Tokens::DIFF_LINE_HEIGHT))
                        .children(body.lines().map(|line| div().child(line.to_string()))),
                )
                .into_any_element()
        }
    }
}

/// Streaming assistant body: sealed markdown blocks + live tail + cursor.
pub fn streaming_assistant_body(
    id: &str,
    source: &str,
    sealed: &SealedBlockCache,
    show_cursor: bool,
) -> impl IntoElement {
    let (_, tail) = split_at_seal_boundary(source);
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
        .when(!tail.is_empty(), |el| el.child(render_live_tail(tail)))
        .when(show_cursor, |el| el.child(static_streaming_cursor()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
