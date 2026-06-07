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
        let sealed = self.sealed_blocks.get(item_id);
        let sealed_height = sealed.map(|sealed| sealed.height).unwrap_or(0.0);
        let tail = if let Some(sealed) = sealed {
            sanitized.get(sealed.sealed_end..).unwrap_or_default()
        } else {
            crate::shared::components::streaming_markdown::split_at_seal_boundary(sanitized).1
        };
        let mut height = crate::features::chat::layout::assistant_body_pt()
            + sealed_height
            + live_tail_height(tail);
        if !sanitized.is_empty() {
            height += crate::features::chat::layout::assistant_streaming_extra();
        }
        let accessory_height = self
            .item_index
            .get(item_id)
            .copied()
            .map(|item_ix| assistant_accessory_height(item_ix, &self.items))
            .unwrap_or_else(assistant_actions_height);
        height + accessory_height + gap
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

    pub(crate) fn flush_sealed_blocks_sanitized(&mut self, item_id: &str, display: &str) {
        let _profile = crate::shared::render_profile::span("flush_sealed_blocks_sanitized");
        let cached_sealed_end = self
            .sealed_blocks
            .get(item_id)
            .map(|cached| cached.sealed_end)
            .unwrap_or(0);
        let (sealed, sealed_len) =
            if cached_sealed_end > 0 && display.is_char_boundary(cached_sealed_end) {
                let tail = display.get(cached_sealed_end..).unwrap_or_default();
                let (newly_sealed, _) =
                    crate::shared::components::streaming_markdown::split_at_seal_boundary(tail);
                let sealed_len = cached_sealed_end + newly_sealed.len();
                (&display[..sealed_len], sealed_len)
            } else {
                let (sealed, _) =
                    crate::shared::components::streaming_markdown::split_at_seal_boundary(display);
                (sealed, sealed.len())
            };
        if sealed_len == 0 {
            return;
        }
        if self
            .sealed_blocks
            .get(item_id)
            .is_some_and(|cached| cached.sealed_end >= sealed_len)
        {
            return;
        }

        let content_hash = markdown_content_hash(sealed, false);
        if self
            .pending_sealed_parses
            .get(item_id)
            .is_some_and(|pending_hash| *pending_hash == content_hash)
        {
            return;
        }

        self.pending_sealed_parses
            .insert(item_id.to_string(), content_hash);
        let tx = self.sealed_parse_tx.clone();
        let item_id = item_id.to_string();
        let sealed = sealed.to_string();
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            let blocks = parse_markdown_blocks_shared_streaming(&sealed, false);
            let height = crate::shared::components::markdown_preview::estimate_markdown_height(
                &blocks, false,
            );
            crate::shared::render_profile::record(
                "sealed_markdown_parse_worker",
                start.elapsed(),
                blocks.len() as u64,
            );
            let _ = tx.send(SealedMarkdownParseResult {
                item_id,
                sealed_end: sealed_len,
                content_hash,
                blocks,
                height,
            });
        });
    }

    pub(crate) fn handle_sealed_parse_result(
        &mut self,
        result: SealedMarkdownParseResult,
        cx: &mut Context<Self>,
    ) {
        if !self
            .pending_sealed_parses
            .get(&result.item_id)
            .is_some_and(|pending_hash| *pending_hash == result.content_hash)
        {
            return;
        }
        self.pending_sealed_parses.remove(&result.item_id);
        if self
            .sealed_blocks
            .get(&result.item_id)
            .is_some_and(|cached| {
                cached.sealed_end >= result.sealed_end && cached.content_hash == result.content_hash
            })
        {
            return;
        }
        self.sealed_blocks.insert(
            result.item_id.clone(),
            SealedBlockCache {
                sealed_end: result.sealed_end,
                blocks: result.blocks,
                content_hash: result.content_hash,
                height: result.height,
            },
        );

        let Some(&item_ix) = self.item_index.get(&result.item_id) else {
            cx.notify();
            return;
        };
        let Some(header_ix) = self.header_ix_for_item(item_ix as u32) else {
            cx.notify();
            return;
        };
        let (start, end) = manifest_span(&self.manifest, header_ix);
        if end - start == 1 {
            let effects = self.patch_streaming_assistant_row(item_ix, start);
            self.apply_thread_effects(effects, cx);
        } else {
            cx.notify();
        }
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
                crate::shared::render_profile::record(
                    "cached_markdown_blocks_hit",
                    Duration::ZERO,
                    1,
                );
                return Arc::clone(&entry.blocks);
            }
        }
        let _profile = crate::shared::render_profile::span("cached_markdown_blocks_miss");
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
