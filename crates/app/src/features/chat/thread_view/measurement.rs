//! Width-aware measurement for visible variable-height transcript rows.
//! The manifest remains virtualized; unchanged rows reuse their layout result.
use super::*;
use crate::features::shell::state::AgentStatus;
use std::hash::{Hash, Hasher};

pub(super) struct MeasuredRow {
    width: gpui::Pixels,
    signature: u64,
    height: gpui::Pixels,
}

impl ThreadView {
    fn measurement_key(&self, row: RowRef) -> Option<(String, u64)> {
        let item_ix = row.item_ix()? as usize;
        let item = self.items.get(item_ix)?;
        let mut signature = std::collections::hash_map::DefaultHasher::new();
        match (row, item) {
            (
                RowRef::UserMessage { .. },
                ThreadItem::UserMessage {
                    text,
                    expanded,
                    attachments,
                    ..
                },
            ) => {
                text.hash(&mut signature);
                expanded.hash(&mut signature);
                attachments.len().hash(&mut signature);
            }
            (
                RowRef::AssistantMessage { .. },
                ThreadItem::AssistantMessage {
                    id,
                    markdown,
                    streaming,
                    ..
                },
            ) => {
                markdown.hash(&mut signature);
                streaming.hash(&mut signature);
                self.sealed_blocks
                    .get(id)
                    .map(|cache| cache.sealed_end)
                    .hash(&mut signature);
                self.user_scrolled_up.hash(&mut signature);
                crate::features::chat::state::is_final_assistant(item_ix, &self.items)
                    .hash(&mut signature);
            }
            (
                RowRef::ReasoningBody { .. },
                ThreadItem::ReasoningStep {
                    summary, status, ..
                },
            ) => {
                summary.hash(&mut signature);
                matches!(status, AgentStatus::Thinking).hash(&mut signature);
            }
            _ => return None,
        }
        Some((item.id().to_string(), signature.finish()))
    }

    pub(super) fn cached_measured_height(&self, row: RowRef) -> Option<gpui::Pixels> {
        let (key, signature) = self.measurement_key(row)?;
        self.measured_rows
            .get(&key)
            .filter(|cached| cached.width == self.layout_width && cached.signature == signature)
            .map(|cached| cached.height)
    }

    pub(super) fn measure_visible_row(
        &mut self,
        row: RowRef,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Pixels> {
        if self.layout_width <= px(0.0) {
            return None;
        }
        if let Some(height) = self.cached_measured_height(row) {
            return Some(height);
        }
        let (key, signature) = self.measurement_key(row)?;
        let height = if let RowRef::UserMessage { item_ix } = row {
            let ThreadItem::UserMessage {
                text,
                expanded,
                attachments,
                ..
            } = &self.items[item_ix as usize]
            else {
                return None;
            };
            let bubble_width = crate::features::chat::components::message::user_message_width(
                text,
                !attachments.is_empty(),
                cx,
            )
            .min(self.layout_width);
            let body_width = (bubble_width - Tokens::spacing_3() * 2.0).max(px(1.0));
            let text_width = (body_width - Tokens::spacing_6()).max(px(1.0));
            let run = gpui::TextRun {
                len: text.len(),
                font: gpui::font(Tokens::ui_font_family()),
                color: Tokens::text_primary(),
                background_color: None,
                underline: Default::default(),
                strikethrough: Default::default(),
            };
            let lines = window
                .text_system()
                .shape_text(
                    text.clone().into(),
                    Tokens::text_md(),
                    &[run],
                    Some(text_width),
                    None,
                )
                .ok()?;
            let count = lines
                .iter()
                .map(|line| line.wrap_boundaries.len() + 1)
                .sum::<usize>()
                .max(1);
            let truncatable = crate::features::shell::state::first_user_message_ix(&self.items)
                == Some(item_ix as usize)
                && crate::features::shell::state::user_message_truncatable(text);
            let count = if truncatable && !expanded {
                count.min(crate::features::shell::state::USER_MESSAGE_PREVIEW_LINES)
            } else {
                count
            };
            let mut height = Tokens::spacing_3() * 2.0 + px(count as f32 * LINE_LEADING);
            if truncatable {
                height += Tokens::spacing_1() + Tokens::text_sm_leading_compact();
            }
            if !attachments.is_empty() {
                let gap = Tokens::spacing_1();
                let tile = Tokens::attachment_preview_size();
                let columns = (((body_width + gap) / (tile + gap)).floor() as usize).max(1);
                let rows = attachments.len().div_ceil(columns);
                height += (tile + gap) * rows as f32;
            }
            height.ceil()
        } else {
            let agent = self.agent.clone();
            let mut content = self.render_row(
                row,
                px(0.0),
                0.0,
                self.approval_active,
                false,
                &agent,
                true,
                cx,
            );
            content
                .layout_as_root(
                    gpui::size(
                        gpui::AvailableSpace::Definite(self.layout_width),
                        gpui::AvailableSpace::MinContent,
                    ),
                    window,
                    cx,
                )
                .height
                .ceil()
        };
        self.measured_rows.insert(
            key,
            MeasuredRow {
                width: self.layout_width,
                signature,
                height,
            },
        );
        Some(height)
    }

    pub(super) fn observe_layout_width(&mut self, width: gpui::Pixels, cx: &mut Context<Self>) {
        if width <= px(0.0) || (width - self.layout_width).abs() < px(1.0) {
            return;
        }
        self.layout_width = width;
        self.measured_rows.clear();
        cx.notify();
    }
}
