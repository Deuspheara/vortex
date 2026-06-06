//! Thread view caches.

use super::*;

#[derive(Clone)]
pub(crate) struct ToolOutputPreview {
    pub(crate) source: String,
    pub(crate) lines: Arc<[Arc<str>]>,
    pub(crate) total_lines: usize,
    pub(crate) full: Arc<str>,
}

pub(crate) struct MarkdownCacheEntry {
    content_hash: u64,
    streaming: bool,
    _has_text: bool,
    _char_count: usize,
    blocks: Arc<[MarkdownBlock]>,
}

pub(crate) fn live_tail_height(tail: &str) -> f32 {
    if tail.trim().is_empty() {
        return 0.0;
    }
    if tail
        .lines()
        .next()
        .is_some_and(|line| line.starts_with("```"))
    {
        let lines = tail.lines().count().saturating_sub(1).max(1) as f32;
        return 18.0 + 8.0 + lines * Tokens::DIFF_LINE_HEIGHT;
    }
    let visual_lines = tail
        .lines()
        .map(|line| line.chars().count().div_ceil(58).max(1))
        .sum::<usize>()
        .max(1) as f32;
    visual_lines * LINE_LEADING
}

pub(crate) fn markdown_content_hash(source: &str, streaming: bool) -> u64 {
    let hash = blake3::hash(source.as_bytes());
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hash.as_bytes()[..8]);
    u64::from_le_bytes(bytes) ^ ((u64::from(streaming)) << 63)
}

impl ThreadView {
    pub(crate) fn streaming_assistant_height_estimate_cached(
        &self,
        item_id: &str,
        sanitized: &str,
        gap: f32,
    ) -> f32 {
        let (_, tail) =
            crate::shared::components::streaming_markdown::split_at_seal_boundary(sanitized);
        let sealed_height = self
            .sealed_blocks
            .get(item_id)
            .map(|sealed| {
                crate::shared::components::markdown_preview::estimate_markdown_height(
                    sealed.blocks.as_ref(),
                    false,
                )
            })
            .unwrap_or(0.0);
        let mut height = crate::features::chat::layout::assistant_body_pt()
            + sealed_height
            + live_tail_height(tail);
        if !sanitized.is_empty() {
            height += crate::features::chat::layout::assistant_streaming_extra();
        }
        let provenance_height = self
            .item_index
            .get(item_id)
            .copied()
            .map(|item_ix| assistant_provenance_height(item_ix, &self.items))
            .unwrap_or(0.0);
        height + provenance_height + gap
    }

    pub(crate) fn record_stream_patch_cost(&mut self, elapsed: Duration) {
        self.last_stream_patch = Some(Instant::now());
        self.stream_batch_ms = if elapsed > Duration::from_millis(12) {
            STREAM_BATCH_BACKPRESSURE_MS
        } else if elapsed > Duration::from_millis(6) {
            STREAM_BATCH_SLOW_MS
        } else {
            STREAM_FRAME_SYNC_MS
        };
    }

    pub(crate) fn flush_sealed_blocks(&mut self, item_id: &str, markdown: &str) {
        let display = crate::agent::text::sanitize_assistant_text(markdown);
        self.flush_sealed_blocks_sanitized(item_id, &display);
    }

    pub(crate) fn flush_sealed_blocks_sanitized(&mut self, item_id: &str, display: &str) {
        self.sealed_blocks
            .entry(item_id.to_string())
            .or_default()
            .update(display);
    }

    pub(crate) fn cached_markdown_blocks(
        &mut self,
        id: &str,
        markdown: &str,
        streaming: bool,
    ) -> Arc<[MarkdownBlock]> {
        let display = crate::agent::text::sanitize_assistant_text(markdown);
        let hash = markdown_content_hash(&display, streaming);
        if let Some(entry) = self.markdown_cache.get(id) {
            if entry.content_hash == hash && entry.streaming == streaming {
                return Arc::clone(&entry.blocks);
            }
        }
        let blocks = parse_markdown_blocks_shared_streaming(&display, streaming);
        let char_count = display.chars().count();
        self.markdown_cache.insert(
            id.to_string(),
            MarkdownCacheEntry {
                content_hash: hash,
                streaming,
                _has_text: !display.is_empty(),
                _char_count: char_count,
                blocks: Arc::clone(&blocks),
            },
        );
        blocks
    }

    pub(crate) fn cached_tool_output_preview(
        &mut self,
        id: &str,
        output: &str,
    ) -> ToolOutputPreview {
        if let Some(cached) = self.tool_output_cache.get(id) {
            if cached.source == output {
                return cached.clone();
            }
        }

        let (lines, total_lines, _truncated, full) = build_tool_output_preview(output);
        let preview = ToolOutputPreview {
            source: output.to_string(),
            lines,
            total_lines,
            full,
        };
        self.tool_output_cache
            .insert(id.to_string(), preview.clone());
        preview
    }
}
