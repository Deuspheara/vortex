//! Lightweight markdown preview for assistant messages.

use std::sync::{Arc, Mutex, OnceLock};

use gpui::{
    FontStyle, FontWeight, HighlightStyle, IntoElement, StrikethroughStyle, StyledText, TextStyle,
    UnderlineStyle, div, prelude::*, px,
};
use gpui_component::scroll::ScrollableElement;
use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};

use crate::features::diff_panel::components::code_highlight::highlight_code;
use crate::shared::components::buttons::btn_copy_icon_arc;
use crate::tokens::{Tokens, element_key};

#[derive(Debug, Clone)]
pub enum MarkdownBlock {
    Paragraph(String),
    Heading(u8, String),
    Rule,
    Blockquote(String),
    List(Vec<ListItem>),
    OrderedList(Vec<ListItem>),
    Code {
        lang: Option<String>,
        body: String,
    },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
}

#[derive(Debug, Clone)]
pub struct ListItem {
    pub indent: usize,
    pub text: String,
    pub checked: Option<bool>,
}

const MAX_PARSE_CACHE: usize = 64;
const MAX_RENDERED_CODE_LINES: usize = 240;
const MAX_RENDERED_CODE_BYTES: usize = 24_000;

/// Line height for body text — must match `Tokens::text_md_leading()` used at render time.
pub const LINE_LEADING: f32 = 23.0;

static PARSE_CACHE: OnceLock<Mutex<std::collections::HashMap<String, Arc<[MarkdownBlock]>>>> =
    OnceLock::new();

/// Parse markdown into blocks, memoized by content hash.
#[allow(dead_code)]
pub fn parse_markdown_blocks(source: &str) -> Vec<MarkdownBlock> {
    parse_markdown_blocks_shared_streaming(source, false)
        .as_ref()
        .to_vec()
}

/// Parse markdown for a thread row; unclosed code fences are kept as-is while streaming.
#[allow(dead_code)]
pub fn parse_markdown_blocks_streaming(source: &str, streaming: bool) -> Vec<MarkdownBlock> {
    parse_markdown_blocks_shared_streaming(source, streaming)
        .as_ref()
        .to_vec()
}

/// Shared parse result for thread rows that re-render frequently.
pub fn parse_markdown_blocks_shared_streaming(
    source: &str,
    streaming: bool,
) -> Arc<[MarkdownBlock]> {
    let start = std::time::Instant::now();
    let source = normalize_markdown_source(source);
    let key = format!(
        "{}:{}",
        blake3::hash(source.as_bytes()).to_hex(),
        u8::from(streaming)
    );
    let cache = PARSE_CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    if let Some(hit) = cache.lock().unwrap().get(&key) {
        crate::shared::render_profile::record("markdown_parse_cache_hit", start.elapsed(), 0);
        return Arc::clone(hit);
    }
    let blocks: Arc<[MarkdownBlock]> = Arc::from(parse_blocks(&source, streaming));
    let mut guard = cache.lock().unwrap();
    if guard.len() >= MAX_PARSE_CACHE {
        guard.clear();
    }
    guard.insert(key, Arc::clone(&blocks));
    crate::shared::render_profile::record("markdown_parse", start.elapsed(), blocks.len() as u64);
    blocks
}

/// Rough height estimate for virtual-list sizing (matches render layout).
pub fn estimate_markdown_height(blocks: &[MarkdownBlock], include_header: bool) -> f32 {
    let mut height = if include_header { 22.0 } else { 0.0 };

    for (i, block) in blocks.iter().enumerate() {
        height += estimate_block_height(block);
        if i + 1 < blocks.len() {
            height += 8.0;
        }
    }

    height.max(24.0)
}

fn estimate_block_height(block: &MarkdownBlock) -> f32 {
    match block {
        MarkdownBlock::Paragraph(text) => {
            let lines = text.chars().count().div_ceil(88).max(1);
            LINE_LEADING * lines as f32
        }
        MarkdownBlock::Rule => 14.0,
        MarkdownBlock::Blockquote(text) => {
            let lines = text.chars().count().div_ceil(82).max(1);
            LINE_LEADING * lines as f32 + 8.0
        }
        MarkdownBlock::Heading(1, _) => 28.0,
        MarkdownBlock::Heading(2, _) => 24.0,
        MarkdownBlock::Heading(_, _) => 22.0,
        MarkdownBlock::List(items) | MarkdownBlock::OrderedList(items) => {
            let rows_h = items
                .iter()
                .map(|item| item.text.chars().count().div_ceil(82).max(1) as f32 * LINE_LEADING)
                .sum::<f32>();
            let gaps_h = items.len().saturating_sub(1) as f32 * f32::from(Tokens::spacing_1());
            rows_h + gaps_h + 4.0
        }
        MarkdownBlock::Code { body, .. } => code_block_height(body),
        MarkdownBlock::Table { headers, rows } => {
            let columns = headers
                .len()
                .max(rows.iter().map(Vec::len).max().unwrap_or(0))
                .max(1);
            let header_lines = headers
                .iter()
                .map(|cell| cell.chars().count().div_ceil(22).max(1))
                .max()
                .unwrap_or(1) as f32;
            let header_h = header_lines * LINE_LEADING + f32::from(Tokens::spacing_1()) * 2.0;
            let body_h = rows
                .iter()
                .map(|row| {
                    let row_lines = row
                        .iter()
                        .chain(
                            std::iter::repeat(&String::new())
                                .take(columns.saturating_sub(row.len())),
                        )
                        .map(|cell| cell.chars().count().div_ceil(22).max(1))
                        .max()
                        .unwrap_or(1) as f32;
                    row_lines * LINE_LEADING + f32::from(Tokens::spacing_1()) * 2.0 + 1.0
                })
                .sum::<f32>();
            header_h + body_h + 2.0
        }
    }
}

fn code_block_height(body: &str) -> f32 {
    let header = 18.0;
    let body_pad = 8.0;
    let preview = code_preview(body);
    let footer = if preview.truncated { 22.0 } else { 0.0 };
    header + body_pad + preview.rendered_lines.max(1) as f32 * Tokens::DIFF_LINE_HEIGHT + footer
}

/// Renders markdown with code and tables.
pub fn markdown_preview(source: &str, syntax_highlight: bool) -> impl IntoElement {
    markdown_preview_streaming(source, syntax_highlight, false)
}

pub fn markdown_preview_streaming(
    source: &str,
    syntax_highlight: bool,
    streaming: bool,
) -> impl IntoElement {
    let blocks = parse_markdown_blocks_shared_streaming(source, streaming);
    markdown_preview_blocks_shared_streaming(blocks, syntax_highlight, streaming)
}

/// Render pre-parsed markdown blocks.
#[allow(dead_code)]
pub fn markdown_preview_blocks(
    blocks: Vec<MarkdownBlock>,
    syntax_highlight: bool,
) -> impl IntoElement {
    markdown_preview_blocks_streaming(blocks, syntax_highlight, false)
}

#[allow(dead_code)]
pub fn markdown_preview_blocks_streaming(
    blocks: Vec<MarkdownBlock>,
    syntax_highlight: bool,
    streaming: bool,
) -> impl IntoElement {
    markdown_preview_blocks_shared_streaming(Arc::from(blocks), syntax_highlight, streaming)
}

pub fn markdown_preview_blocks_shared_streaming(
    blocks: Arc<[MarkdownBlock]>,
    syntax_highlight: bool,
    streaming: bool,
) -> impl IntoElement {
    let _profile = crate::shared::render_profile::span("markdown_render_blocks");
    let block_count = blocks.len();
    div()
        .id("markdown-preview")
        .w_full()
        .min_w(gpui::px(0.0))
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .children(blocks.iter().enumerate().map(|(index, block)| {
            let is_tail = streaming && index + 1 == block_count;
            let highlight = syntax_highlight && !is_tail && !is_incomplete_code_block(block);
            let inline = syntax_highlight && !is_tail;
            render_block(block, highlight, inline, index)
        }))
}

pub fn markdown_preview_blocks_thread_shared(
    blocks: Arc<[MarkdownBlock]>,
    streaming: bool,
) -> impl IntoElement {
    let _profile = crate::shared::render_profile::span("markdown_render_blocks");
    let block_count = blocks.len();
    div()
        .id("markdown-preview-thread")
        .w_full()
        .min_w(gpui::px(0.0))
        .flex()
        .flex_col()
        .gap(Tokens::spacing_2())
        .children(blocks.iter().enumerate().map(|(index, block)| {
            let _is_tail = streaming && index + 1 == block_count;
            render_block_thread(block, index)
        }))
}

pub fn markdown_preview_thread_streaming(source: &str, streaming: bool) -> impl IntoElement {
    let blocks = parse_markdown_blocks_shared_streaming(source, streaming);
    markdown_preview_blocks_thread_shared(blocks, streaming)
}

fn is_incomplete_code_block(block: &MarkdownBlock) -> bool {
    matches!(block, MarkdownBlock::Code { body, .. } if body.is_empty())
}

fn parse_blocks(source: &str, streaming: bool) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if line.starts_with("```") {
            let lang = line.trim_start_matches('`').trim().to_string();
            let lang = if lang.is_empty() { None } else { Some(lang) };
            i += 1;
            let mut body = String::new();
            while i < lines.len() && !lines[i].starts_with("```") {
                if !body.is_empty() {
                    body.push('\n');
                }
                body.push_str(lines[i]);
                i += 1;
            }
            let closed = i < lines.len();
            blocks.push(MarkdownBlock::Code { lang, body });
            if closed {
                i += 1;
            } else if streaming {
                break;
            }
            continue;
        }

        if is_table_header_line(line)
            && i + 1 < lines.len()
            && is_table_separator_line(lines[i + 1], line)
        {
            let headers = parse_table_row(line);
            i += 2;
            let mut rows = Vec::new();
            while i < lines.len() && is_table_row_line(lines[i], headers.len()) {
                rows.push(parse_table_row(lines[i]));
                i += 1;
            }
            blocks.push(MarkdownBlock::Table { headers, rows });
            continue;
        }

        if let Some(level) = parse_heading_level(line) {
            let text = line[level as usize..].trim().to_string();
            blocks.push(MarkdownBlock::Heading(level, text));
            i += 1;
            continue;
        }

        if is_rule_line(line) {
            blocks.push(MarkdownBlock::Rule);
            i += 1;
            continue;
        }

        if is_blockquote_line(line) {
            let mut quote = String::new();
            while i < lines.len() && is_blockquote_line(lines[i]) {
                if !quote.is_empty() {
                    quote.push('\n');
                }
                quote.push_str(blockquote_text(lines[i]).trim());
                i += 1;
            }
            blocks.push(MarkdownBlock::Blockquote(quote));
            continue;
        }

        if is_list_line(line) {
            let mut items = Vec::new();
            while i < lines.len() && is_list_line(lines[i]) {
                items.push(list_item(lines[i]));
                i += 1;
            }
            blocks.push(MarkdownBlock::List(items));
            continue;
        }

        if is_ordered_list_line(line) {
            let mut items = Vec::new();
            while i < lines.len() && is_ordered_list_line(lines[i]) {
                items.push(ordered_list_item(lines[i]));
                i += 1;
            }
            blocks.push(MarkdownBlock::OrderedList(items));
            continue;
        }

        if line.trim().is_empty() {
            i += 1;
            continue;
        }

        let mut para = line.to_string();
        i += 1;
        while i < lines.len()
            && !lines[i].trim().is_empty()
            && parse_heading_level(lines[i]).is_none()
            && !(is_table_header_line(lines[i])
                && i + 1 < lines.len()
                && is_table_separator_line(lines[i + 1], lines[i]))
            && !lines[i].starts_with("```")
            && !is_blockquote_line(lines[i])
            && !is_list_line(lines[i])
            && !is_ordered_list_line(lines[i])
        {
            para.push(' ');
            para.push_str(lines[i].trim());
            i += 1;
        }
        blocks.push(MarkdownBlock::Paragraph(para));
    }

    blocks
}

fn parse_table_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

fn is_table_header_line(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty() && trimmed.contains('|')
}

fn is_table_separator_line(line: &str, header_line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || !trimmed.contains('-') {
        return false;
    }

    let header_columns = parse_table_row(header_line).len();
    let separator_columns = parse_table_row(trimmed);
    separator_columns.len() == header_columns
        && !separator_columns.is_empty()
        && separator_columns.iter().all(|segment| {
            let segment = segment.trim();
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|ch| ch == '-' || ch == ':' || ch.is_whitespace())
                && segment.chars().filter(|ch| *ch == '-').count() >= 3
        })
}

fn is_table_row_line(line: &str, expected_columns: usize) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && trimmed.contains('|')
        && parse_table_row(trimmed).len() == expected_columns
}

fn parse_heading_level(line: &str) -> Option<u8> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) && line.len() > hashes && line.as_bytes()[hashes] == b' ' {
        Some(hashes as u8)
    } else {
        None
    }
}

fn is_list_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ")
}

fn is_rule_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= 3
        && trimmed
            .chars()
            .all(|ch| ch == '-' || ch == '_' || ch == '*' || ch.is_whitespace())
        && trimmed.chars().filter(|ch| !ch.is_whitespace()).count() >= 3
}

fn is_blockquote_line(line: &str) -> bool {
    line.trim_start().starts_with("> ")
}

fn blockquote_text(line: &str) -> String {
    line.trim_start()
        .strip_prefix("> ")
        .unwrap_or(line.trim_start())
        .to_string()
}

fn is_ordered_list_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(dot_ix) = trimmed.find('.') else {
        return false;
    };
    dot_ix > 0
        && trimmed[..dot_ix].chars().all(|c| c.is_ascii_digit())
        && trimmed
            .as_bytes()
            .get(dot_ix + 1)
            .is_some_and(u8::is_ascii_whitespace)
}

fn list_item(line: &str) -> ListItem {
    let indent = line.chars().take_while(|c| c.is_whitespace()).count() / 2;
    let trimmed = line.trim_start();
    let text = if let Some(rest) = trimmed.strip_prefix("- ") {
        rest.to_string()
    } else if let Some(rest) = trimmed.strip_prefix("* ") {
        rest.to_string()
    } else if let Some(rest) = trimmed.strip_prefix("+ ") {
        rest.to_string()
    } else {
        trimmed.to_string()
    };
    let (checked, text) = parse_task_marker(&text);
    ListItem {
        indent,
        text,
        checked,
    }
}

fn ordered_list_item(line: &str) -> ListItem {
    let indent = line.chars().take_while(|c| c.is_whitespace()).count() / 2;
    let trimmed = line.trim_start();
    let dot_ix = trimmed.find('.').unwrap_or(0);
    let (checked, text) = parse_task_marker(trimmed[dot_ix + 1..].trim_start());
    ListItem {
        indent,
        text,
        checked,
    }
}

fn parse_task_marker(text: &str) -> (Option<bool>, String) {
    if let Some(rest) = text.strip_prefix("[ ] ") {
        return (Some(false), rest.to_string());
    }
    if let Some(rest) = text.strip_prefix("[x] ") {
        return (Some(true), rest.to_string());
    }
    if let Some(rest) = text.strip_prefix("[X] ") {
        return (Some(true), rest.to_string());
    }
    (None, text.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineStyle {
    Bold,
    Italic,
    Strikethrough,
    Code,
    Link,
}

fn parse_inline_spans(source: &str) -> (String, Vec<(std::ops::Range<usize>, InlineStyle)>) {
    let mut plain = String::new();
    let mut spans = Vec::new();
    let parser = Parser::new_ext(source, markdown_options());
    let mut stack: Vec<(InlineStyle, usize)> = Vec::new();

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Emphasis => stack.push((InlineStyle::Italic, plain.len())),
                Tag::Strong => stack.push((InlineStyle::Bold, plain.len())),
                Tag::Strikethrough => stack.push((InlineStyle::Strikethrough, plain.len())),
                Tag::Link { link_type, .. }
                    if matches!(
                        link_type,
                        LinkType::Inline
                            | LinkType::Reference
                            | LinkType::Autolink
                            | LinkType::Email
                    ) =>
                {
                    stack.push((InlineStyle::Link, plain.len()));
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Emphasis => {
                    close_inline_style(InlineStyle::Italic, plain.len(), &mut stack, &mut spans)
                }
                TagEnd::Strong => {
                    close_inline_style(InlineStyle::Bold, plain.len(), &mut stack, &mut spans)
                }
                TagEnd::Strikethrough => close_inline_style(
                    InlineStyle::Strikethrough,
                    plain.len(),
                    &mut stack,
                    &mut spans,
                ),
                TagEnd::Link => {
                    close_inline_style(InlineStyle::Link, plain.len(), &mut stack, &mut spans)
                }
                _ => {}
            },
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                plain.push_str(&text)
            }
            Event::Code(text) => {
                let start = plain.len();
                plain.push_str(&text);
                spans.push((start..plain.len(), InlineStyle::Code));
            }
            Event::SoftBreak => plain.push(' '),
            Event::HardBreak => plain.push('\n'),
            _ => {}
        }
    }

    (plain, spans)
}

fn markdown_options() -> Options {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options
}

fn close_inline_style(
    style: InlineStyle,
    end: usize,
    stack: &mut Vec<(InlineStyle, usize)>,
    spans: &mut Vec<(std::ops::Range<usize>, InlineStyle)>,
) {
    if let Some(ix) = stack.iter().rposition(|(active, _)| *active == style) {
        let (_, start) = stack.remove(ix);
        if start < end {
            spans.push((start..end, style));
        }
    }
}

fn inline_highlight_style(style: InlineStyle) -> HighlightStyle {
    match style {
        InlineStyle::Bold => HighlightStyle {
            font_weight: Some(FontWeight::SEMIBOLD),
            ..Default::default()
        },
        InlineStyle::Italic => HighlightStyle {
            font_style: Some(FontStyle::Italic),
            ..Default::default()
        },
        InlineStyle::Strikethrough => HighlightStyle {
            strikethrough: Some(StrikethroughStyle {
                thickness: px(1.0),
                color: Some(Tokens::text_secondary()),
            }),
            ..Default::default()
        },
        InlineStyle::Code => HighlightStyle {
            color: Some(Tokens::code_fg()),
            background_color: Some(Tokens::code_bg()),
            ..Default::default()
        },
        InlineStyle::Link => HighlightStyle {
            color: Some(Tokens::accent()),
            underline: Some(UnderlineStyle {
                color: Some(Tokens::accent()),
                ..Default::default()
            }),
            ..Default::default()
        },
    }
}

fn next_char_boundary(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while index < text.len() && !text.is_char_boundary(index) {
        index += 1;
    }
    index
}

fn prev_char_boundary(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn sanitize_highlights(
    text: &str,
    highlights: Vec<(std::ops::Range<usize>, HighlightStyle)>,
) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    let mut bounded = Vec::with_capacity(highlights.len());
    for (range, style) in highlights {
        let start = next_char_boundary(text, range.start);
        let end = prev_char_boundary(text, range.end);
        if start < end {
            bounded.push((start..end, style));
        }
    }

    bounded.sort_by_key(|(range, _)| (range.start, range.end));

    let mut sanitized = Vec::with_capacity(bounded.len());
    let mut cursor = 0usize;
    for (range, style) in bounded {
        let start = next_char_boundary(text, range.start.max(cursor));
        let end = prev_char_boundary(text, range.end);
        if start < end {
            cursor = end;
            sanitized.push((start..end, style));
        }
    }

    sanitized
}

fn render_inline(text: &str, enabled: bool, base_size: gpui::AbsoluteLength) -> impl IntoElement {
    if !enabled || text.is_empty() {
        return div()
            .min_w(gpui::px(0.0))
            .whitespace_normal()
            .child(text.to_string())
            .into_any_element();
    }

    let (plain, spans) = parse_inline_spans(text);
    if spans.is_empty() {
        return div()
            .min_w(gpui::px(0.0))
            .whitespace_normal()
            .child(plain)
            .into_any_element();
    }

    let default_style = TextStyle {
        color: Tokens::text_primary().into(),
        font_size: base_size,
        font_family: Tokens::ui_font_family().into(),
        ..Default::default()
    };

    let highlights = sanitize_highlights(
        &plain,
        spans
            .into_iter()
            .map(|(range, style)| (range, inline_highlight_style(style)))
            .collect(),
    );

    div()
        .min_w(gpui::px(0.0))
        .whitespace_normal()
        .child(StyledText::new(plain).with_default_highlights(&default_style, highlights))
        .into_any_element()
}

fn render_block(
    block: &MarkdownBlock,
    syntax_highlight: bool,
    inline_enabled: bool,
    index: usize,
) -> gpui::AnyElement {
    match block {
        MarkdownBlock::Paragraph(text) => div()
            .w_full()
            .min_w(gpui::px(0.0))
            .whitespace_normal()
            .font_family(Tokens::ui_font_family())
            .text_size(Tokens::text_md())
            .line_height(Tokens::text_md_leading())
            .text_color(Tokens::text_primary())
            .child(render_inline(
                &text,
                inline_enabled,
                Tokens::text_md().into(),
            ))
            .into_any_element(),
        MarkdownBlock::Heading(level, text) => {
            let (size, weight) = match level {
                1 => (Tokens::text_lg(), FontWeight::SEMIBOLD),
                2 => (Tokens::text_base(), FontWeight::SEMIBOLD),
                _ => (Tokens::text_sm(), FontWeight::SEMIBOLD),
            };
            div()
                .w_full()
                .min_w(gpui::px(0.0))
                .whitespace_normal()
                .font_family(Tokens::ui_font_family())
                .text_size(size)
                .font_weight(weight)
                .text_color(Tokens::text_primary())
                .child(render_inline(&text, inline_enabled, size.into()))
                .into_any_element()
        }
        MarkdownBlock::Rule => div()
            .w_full()
            .h(px(1.0))
            .bg(Tokens::border_subtle())
            .opacity(0.9)
            .into_any_element(),
        MarkdownBlock::Blockquote(text) => div()
            .w_full()
            .min_w(gpui::px(0.0))
            .whitespace_normal()
            .pl(Tokens::spacing_4())
            .border_l_1()
            .border_color(Tokens::border_subtle())
            .font_family(Tokens::ui_font_family())
            .text_size(Tokens::text_md())
            .line_height(Tokens::text_md_leading())
            .text_color(Tokens::text_secondary())
            .child(render_inline(
                text,
                inline_enabled,
                Tokens::text_md().into(),
            ))
            .into_any_element(),
        MarkdownBlock::List(items) => render_list(&items, inline_enabled).into_any_element(),
        MarkdownBlock::OrderedList(items) => {
            render_ordered_list(&items, inline_enabled).into_any_element()
        }
        MarkdownBlock::Code { lang, body } => {
            render_code_block(lang.as_deref(), &body, syntax_highlight, index).into_any_element()
        }
        MarkdownBlock::Table { headers, rows } => render_table(&headers, &rows).into_any_element(),
    }
}

fn render_block_thread(block: &MarkdownBlock, index: usize) -> gpui::AnyElement {
    match block {
        MarkdownBlock::Paragraph(text) => div()
            .w_full()
            .min_w(gpui::px(0.0))
            .whitespace_normal()
            .font_family(Tokens::ui_font_family())
            .text_size(Tokens::text_md())
            .line_height(Tokens::text_md_leading())
            .text_color(Tokens::text_primary())
            .child(render_inline(text, true, Tokens::text_md().into()))
            .into_any_element(),
        MarkdownBlock::Heading(level, text) => {
            let (size, weight) = match level {
                1 => (Tokens::text_lg(), FontWeight::SEMIBOLD),
                2 => (Tokens::text_base(), FontWeight::SEMIBOLD),
                _ => (Tokens::text_sm(), FontWeight::SEMIBOLD),
            };
            div()
                .w_full()
                .min_w(gpui::px(0.0))
                .whitespace_normal()
                .font_family(Tokens::ui_font_family())
                .text_size(size)
                .font_weight(weight)
                .text_color(Tokens::text_primary())
                .child(render_inline(text, true, size.into()))
                .into_any_element()
        }
        MarkdownBlock::Rule => div()
            .w_full()
            .h(px(1.0))
            .bg(Tokens::border_subtle())
            .opacity(0.9)
            .into_any_element(),
        MarkdownBlock::Blockquote(text) => div()
            .w_full()
            .min_w(gpui::px(0.0))
            .whitespace_normal()
            .pl(Tokens::spacing_3())
            .border_l_1()
            .border_color(Tokens::border_subtle())
            .font_family(Tokens::ui_font_family())
            .text_size(Tokens::text_md())
            .line_height(Tokens::text_md_leading())
            .text_color(Tokens::text_secondary())
            .child(render_inline(text, true, Tokens::text_md().into()))
            .into_any_element(),
        MarkdownBlock::List(items) => render_list(items, true).into_any_element(),
        MarkdownBlock::OrderedList(items) => render_ordered_list(items, true).into_any_element(),
        MarkdownBlock::Code { lang, body } => {
            render_code_block_thread(lang.as_deref(), body, index).into_any_element()
        }
        MarkdownBlock::Table { headers, rows } => {
            render_table_thread(headers, rows).into_any_element()
        }
    }
}

fn render_list(items: &[ListItem], inline_enabled: bool) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_1())
        .children(items.iter().map(|item| {
            let indent = Tokens::tree_indent(item.indent.min(4) as u32);
            div()
                .w_full()
                .min_w(gpui::px(0.0))
                .flex()
                .items_start()
                .gap(Tokens::spacing_2())
                .pl(indent)
                .child(render_list_marker(item.checked))
                .child(
                    div()
                        .flex_1()
                        .min_w(gpui::px(0.0))
                        .whitespace_normal()
                        .font_family(Tokens::ui_font_family())
                        .text_size(Tokens::text_md())
                        .line_height(Tokens::text_md_leading())
                        .text_color(Tokens::text_primary())
                        .child(render_inline(
                            &item.text,
                            inline_enabled,
                            Tokens::text_md().into(),
                        )),
                )
        }))
}

fn render_ordered_list(items: &[ListItem], inline_enabled: bool) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(Tokens::spacing_1())
        .children(items.iter().enumerate().map(|(ix, item)| {
            let indent = Tokens::tree_indent(item.indent.min(4) as u32);
            div()
                .w_full()
                .min_w(gpui::px(0.0))
                .flex()
                .items_start()
                .gap(Tokens::spacing_2())
                .pl(indent)
                .child(render_ordered_list_marker(ix, item.checked))
                .child(
                    div()
                        .flex_1()
                        .min_w(gpui::px(0.0))
                        .whitespace_normal()
                        .font_family(Tokens::ui_font_family())
                        .text_size(Tokens::text_md())
                        .line_height(Tokens::text_md_leading())
                        .text_color(Tokens::text_primary())
                        .child(render_inline(
                            &item.text,
                            inline_enabled,
                            Tokens::text_md().into(),
                        )),
                )
        }))
}

fn render_list_marker(checked: Option<bool>) -> gpui::AnyElement {
    match checked {
        Some(done) => div()
            .mt(px(3.0))
            .w(px(14.0))
            .h(px(14.0))
            .flex_shrink_0()
            .rounded(Tokens::radius_xs())
            .border_1()
            .border_color(if done {
                Tokens::success()
            } else {
                Tokens::border_subtle()
            })
            .bg(if done {
                Tokens::success().opacity(0.18)
            } else {
                Tokens::main_bg().opacity(0.0)
            })
            .flex()
            .items_center()
            .justify_center()
            .text_size(Tokens::text_xs())
            .text_color(if done {
                Tokens::success()
            } else {
                Tokens::text_tertiary()
            })
            .child(if done { "✓" } else { "" })
            .into_any_element(),
        None => div()
            .flex_shrink_0()
            .text_size(Tokens::text_md())
            .text_color(Tokens::text_tertiary())
            .child("•")
            .into_any_element(),
    }
}

fn render_ordered_list_marker(index: usize, checked: Option<bool>) -> gpui::AnyElement {
    if checked.is_some() {
        return render_list_marker(checked);
    }

    div()
        .w(Tokens::ordered_list_marker_width())
        .flex_shrink_0()
        .text_size(Tokens::text_md())
        .text_color(Tokens::text_tertiary())
        .child(format!("{}.", index + 1))
        .into_any_element()
}

fn split_highlights_by_line(
    body: &str,
    highlights: &[(std::ops::Range<usize>, HighlightStyle)],
) -> Vec<Vec<(std::ops::Range<usize>, HighlightStyle)>> {
    let mut lines = if body.is_empty() {
        vec![(0usize, 0usize)]
    } else {
        let mut ranges = Vec::new();
        let mut start = 0usize;
        for line in body.split('\n') {
            let end = start + line.len();
            ranges.push((start, end));
            start = end + 1;
        }
        ranges
    };

    if lines.is_empty() {
        lines.push((0, 0));
    }

    let mut per_line = vec![Vec::new(); lines.len()];
    for (range, style) in highlights {
        for (line_ix, (line_start, line_end)) in lines.iter().copied().enumerate() {
            if range.end <= line_start || range.start >= line_end {
                continue;
            }
            let local_start = range.start.max(line_start) - line_start;
            let local_end = range.end.min(line_end) - line_start;
            if local_start < local_end {
                per_line[line_ix].push((local_start..local_end, style.clone()));
            }
        }
    }

    per_line
}

fn render_code_text_line(
    line: String,
    spans: Vec<(std::ops::Range<usize>, HighlightStyle)>,
) -> gpui::AnyElement {
    let line = if line.is_empty() {
        " ".to_string()
    } else {
        line
    };
    if spans.is_empty() {
        return div()
            .w_full()
            .overflow_hidden()
            .whitespace_nowrap()
            .font_family(Tokens::terminal_font_family())
            .text_size(Tokens::text_code())
            .text_color(Tokens::code_fg())
            .child(line)
            .into_any_element();
    }

    let default_style = TextStyle {
        color: Tokens::code_fg().into(),
        font_size: Tokens::text_code().into(),
        ..Default::default()
    };

    let spans = sanitize_highlights(&line, spans);

    div()
        .w_full()
        .overflow_hidden()
        .whitespace_nowrap()
        .font_family(Tokens::terminal_font_family())
        .text_size(Tokens::text_code())
        .text_color(Tokens::code_fg())
        .child(StyledText::new(line).with_default_highlights(&default_style, spans))
        .into_any_element()
}

fn render_code_lines(
    body: &str,
    lang: Option<&str>,
    syntax_highlight: bool,
) -> Vec<gpui::AnyElement> {
    let lines: Vec<String> = if body.is_empty() {
        vec![String::new()]
    } else {
        body.split('\n').map(str::to_string).collect()
    };
    let per_line = split_highlights_by_line(body, &highlight_code(lang, body, syntax_highlight));

    lines
        .into_iter()
        .enumerate()
        .map(|(ix, line)| {
            render_code_text_line(line, per_line.get(ix).cloned().unwrap_or_default())
        })
        .collect()
}

fn normalize_markdown_source(source: &str) -> String {
    let mut output = Vec::new();
    let mut in_html_comment = false;

    for line in source.lines() {
        let mut current = line.to_string();
        let trimmed = current.trim();

        if in_html_comment {
            if trimmed.contains("-->") {
                in_html_comment = false;
            }
            continue;
        }
        if trimmed.starts_with("<!--") {
            if !trimmed.contains("-->") {
                in_html_comment = true;
            }
            continue;
        }

        if let Some(markdown_line) = html_block_line_to_markdown(trimmed) {
            if !markdown_line.is_empty() {
                output.push(markdown_line);
            }
            continue;
        }

        current = current.replace("<br>", "\n");
        current = current.replace("<br/>", "\n");
        current = current.replace("<br />", "\n");
        output.push(decode_basic_html_entities(&current));
    }

    output.join("\n")
}

fn html_block_line_to_markdown(line: &str) -> Option<String> {
    if !line.starts_with('<') {
        return None;
    }

    let lower = line.to_ascii_lowercase();
    if lower.starts_with("<img")
        || lower.starts_with("<svg")
        || lower.starts_with("</svg")
        || lower.starts_with("<picture")
        || lower.starts_with("</picture")
        || lower.starts_with("<source")
        || lower.starts_with("<br")
        || lower.starts_with("</")
    {
        return Some(String::new());
    }

    for (tag, prefix) in [
        ("h1", "# "),
        ("h2", "## "),
        ("h3", "### "),
        ("h4", "#### "),
        ("li", "- "),
    ] {
        if lower.starts_with(&format!("<{tag}")) {
            let text = strip_html_tags(line);
            return Some(if text.is_empty() {
                String::new()
            } else {
                format!("{prefix}{text}")
            });
        }
    }

    if lower.starts_with("<p")
        || lower.starts_with("<div")
        || lower.starts_with("<span")
        || lower.starts_with("<summary")
        || lower.starts_with("<details")
        || lower.starts_with("<strong")
        || lower.starts_with("<em")
        || lower.starts_with("<a ")
    {
        return Some(strip_html_tags(line));
    }

    if line.ends_with('>') {
        return Some(strip_html_tags(line));
    }

    None
}

fn strip_html_tags(input: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    decode_basic_html_entities(out.trim())
}

fn decode_basic_html_entities(input: &str) -> String {
    input
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn render_code_block(
    lang: Option<&str>,
    body: &str,
    syntax_highlight: bool,
    index: usize,
) -> impl IntoElement {
    let label = lang.unwrap_or("code").to_string();
    let full_body = body.to_string();
    let preview = code_preview(body);
    let preview_body = preview.body;
    let show_copy = syntax_highlight;
    let code_lines = render_code_lines(&preview_body, lang.as_deref(), syntax_highlight);

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
                .bg(Tokens::panel_bg())
                .border_b_1()
                .border_color(Tokens::border_subtle())
                .flex()
                .items_center()
                .justify_between()
                .gap(Tokens::spacing_2())
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_tertiary())
                        .child(label),
                )
                .when(show_copy, |row| {
                    row.child(btn_copy_icon_arc(
                        element_key("copy-code", &index.to_string()),
                        std::sync::Arc::from(full_body.as_str()),
                        "Copy code",
                    ))
                }),
        )
        .child(
            div()
                .w_full()
                .min_w(px(0.0))
                .overflow_x_hidden()
                .px(Tokens::spacing_2())
                .py(Tokens::spacing_1())
                .bg(Tokens::code_bg())
                .line_height(px(Tokens::DIFF_LINE_HEIGHT))
                .whitespace_nowrap()
                .children(code_lines),
        )
        .when(preview.truncated, |el| {
            el.child(
                div()
                    .w_full()
                    .px(Tokens::spacing_2())
                    .py(Tokens::spacing_1())
                    .bg(Tokens::panel_bg())
                    .border_t_1()
                    .border_color(Tokens::border_subtle())
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_tertiary())
                    .child(format!(
                        "Preview truncated: first {} of {} lines",
                        preview.rendered_lines, preview.total_lines
                    )),
            )
        })
}

fn render_code_block_thread(lang: Option<&str>, body: &str, index: usize) -> impl IntoElement {
    let preview = code_preview(body);
    let label = lang.unwrap_or("code").to_string();
    let code_lines = render_code_lines(&preview.body, lang, true);
    let full_body = std::sync::Arc::<str>::from(body.to_string().as_str());

    div()
        .w_full()
        .rounded(Tokens::radius_sm())
        .border_1()
        .border_color(Tokens::border_subtle())
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .px(Tokens::spacing_2())
                .py(Tokens::spacing_0p5())
                .bg(Tokens::surface_hover())
                .border_b_1()
                .border_color(Tokens::border_subtle())
                .flex()
                .items_center()
                .justify_between()
                .gap(Tokens::spacing_2())
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_tertiary())
                        .child(label),
                )
                .child(btn_copy_icon_arc(
                    element_key("copy-code-thread", &index.to_string()),
                    full_body,
                    "Copy code",
                )),
        )
        .child(
            div()
                .w_full()
                .min_w(px(0.0))
                .overflow_x_hidden()
                .px(Tokens::spacing_2())
                .py(Tokens::spacing_1())
                .bg(Tokens::code_bg())
                .line_height(px(Tokens::DIFF_LINE_HEIGHT))
                .children(code_lines),
        )
        .when(preview.truncated, |el| {
            el.child(
                div()
                    .w_full()
                    .px(Tokens::spacing_2())
                    .py(Tokens::spacing_0p5())
                    .bg(Tokens::surface_hover())
                    .border_t_1()
                    .border_color(Tokens::border_subtle())
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_tertiary())
                    .child(format!(
                        "Showing first {} of {} lines",
                        preview.rendered_lines, preview.total_lines
                    )),
            )
        })
}

struct CodePreview {
    body: String,
    rendered_lines: usize,
    total_lines: usize,
    truncated: bool,
}

fn code_preview(body: &str) -> CodePreview {
    let total_lines = body.lines().count().max(1);
    let mut rendered_lines = 0usize;
    let mut rendered_bytes = 0usize;
    let mut truncated = false;
    let mut out = String::new();

    for line in body.lines() {
        let line_bytes = line.len() + 1;
        if rendered_lines == 0 && line_bytes > MAX_RENDERED_CODE_BYTES {
            out = line.chars().take(MAX_RENDERED_CODE_BYTES).collect();
            rendered_lines = 1;
            truncated = true;
            break;
        }
        if rendered_lines >= MAX_RENDERED_CODE_LINES
            || (rendered_bytes + line_bytes > MAX_RENDERED_CODE_BYTES && rendered_lines > 0)
        {
            truncated = true;
            break;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
        rendered_lines += 1;
        rendered_bytes += line_bytes;
    }

    if rendered_lines < total_lines {
        truncated = true;
    }

    if out.is_empty() && !body.is_empty() {
        out = body.chars().take(MAX_RENDERED_CODE_BYTES).collect();
        rendered_lines = 1;
        truncated = body.len() > out.len();
    }

    CodePreview {
        body: out,
        rendered_lines: rendered_lines.max(1),
        total_lines,
        truncated,
    }
}

fn render_table(headers: &[String], rows: &[Vec<String>]) -> impl IntoElement {
    let column_count = headers
        .len()
        .max(rows.iter().map(Vec::len).max().unwrap_or(0))
        .max(1);
    let header_cells: Vec<_> = headers
        .iter()
        .map(|h| {
            div()
                .min_w(px(160.0))
                .flex_1()
                .px(Tokens::spacing_2())
                .py(Tokens::spacing_1())
                .text_size(Tokens::text_label())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(Tokens::text_secondary())
                .child(render_inline(h, true, Tokens::text_label().into()))
                .into_any_element()
        })
        .collect();

    let row_elements: Vec<_> = rows
        .iter()
        .map(|row| {
            let cells: Vec<String> = row
                .iter()
                .cloned()
                .chain(std::iter::repeat_n(
                    String::new(),
                    column_count.saturating_sub(row.len()),
                ))
                .collect();
            div()
                .flex()
                .border_t_1()
                .border_color(Tokens::border_subtle())
                .children(cells.into_iter().map(|cell| {
                    div()
                        .min_w(px(160.0))
                        .flex_1()
                        .px(Tokens::spacing_2())
                        .py(Tokens::spacing_1())
                        .text_size(Tokens::text_label())
                        .line_height(Tokens::text_sm_leading_compact())
                        .text_color(Tokens::text_primary())
                        .child(render_inline(&cell, true, Tokens::text_label().into()))
                        .into_any_element()
                }))
                .into_any_element()
        })
        .collect();

    div()
        .w_full()
        .rounded(Tokens::radius_sm())
        .border_1()
        .border_color(Tokens::border_subtle())
        .overflow_hidden()
        .overflow_x_scrollbar()
        .child(
            div()
                .min_w(px((column_count as f32 * 160.0).max(320.0)))
                .child(div().flex().bg(Tokens::panel_bg()).children(header_cells))
                .children(row_elements),
        )
}

fn render_table_thread(headers: &[String], rows: &[Vec<String>]) -> impl IntoElement {
    render_table(headers, rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blockquote_block() {
        let blocks = parse_markdown_blocks("> quoted\n> second");
        assert!(matches!(
            blocks.as_slice(),
            [MarkdownBlock::Blockquote(text)] if text == "quoted\nsecond"
        ));
    }

    #[test]
    fn preserves_list_indentation() {
        let blocks = parse_markdown_blocks("- top\n  - nested\n    - deep");
        let [MarkdownBlock::List(items)] = blocks.as_slice() else {
            panic!("expected list block");
        };
        assert_eq!(items[0].indent, 0);
        assert_eq!(items[1].indent, 1);
        assert_eq!(items[2].indent, 2);
    }

    #[test]
    fn keeps_incomplete_streaming_fence_as_code() {
        let blocks = parse_markdown_blocks_streaming("before\n\n```rust\nfn main() {}", true);
        assert!(matches!(
            blocks.last(),
            Some(MarkdownBlock::Code {
                lang: Some(lang),
                body
            }) if lang == "rust" && body.contains("fn main")
        ));
    }

    #[test]
    fn parses_gfm_table_without_outer_pipes() {
        let blocks = parse_markdown_blocks(
            "Task | Status | Priority\n--- | --- | ---\nWrite docs | Done | High\nFix bug | Todo | Medium",
        );
        let [MarkdownBlock::Table { headers, rows }] = blocks.as_slice() else {
            panic!("expected table block");
        };
        assert_eq!(headers, &["Task", "Status", "Priority"]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec!["Write docs", "Done", "High"]);
    }

    #[test]
    fn parses_task_list_markers() {
        let blocks = parse_markdown_blocks("- [x] shipped\n- [ ] pending");
        let [MarkdownBlock::List(items)] = blocks.as_slice() else {
            panic!("expected list block");
        };
        assert_eq!(items[0].checked, Some(true));
        assert_eq!(items[0].text, "shipped");
        assert_eq!(items[1].checked, Some(false));
        assert_eq!(items[1].text, "pending");
    }

    #[test]
    fn code_preview_truncates_large_blocks() {
        let body = (0..MAX_RENDERED_CODE_LINES + 20)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let preview = code_preview(&body);
        assert!(preview.truncated);
        assert_eq!(preview.rendered_lines, MAX_RENDERED_CODE_LINES);
    }

    #[test]
    fn sanitize_highlights_clips_overlapping_spans() {
        let style = HighlightStyle::default();
        let highlights = sanitize_highlights(
            "abcdef",
            vec![(0..6, style.clone()), (2..4, style.clone()), (4..6, style)],
        );
        let total_len = highlights
            .iter()
            .map(|(range, _)| range.end - range.start)
            .sum::<usize>();
        assert!(total_len <= "abcdef".len());
        assert_eq!(highlights.len(), 1);
        assert_eq!(highlights[0].0, 0..6);
    }

    #[test]
    fn inline_spans_can_be_nested_without_invalid_runs() {
        let (plain, spans) = parse_inline_spans("**[Android](https://example.com)**");
        assert_eq!(plain, "Android");
        let highlights = sanitize_highlights(
            &plain,
            spans
                .into_iter()
                .map(|(range, style)| (range, inline_highlight_style(style)))
                .collect(),
        );
        let total_len = highlights
            .iter()
            .map(|(range, _)| range.end - range.start)
            .sum::<usize>();
        assert!(total_len <= plain.len());
    }
}
