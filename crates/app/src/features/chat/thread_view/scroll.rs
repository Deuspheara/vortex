//! Thread scroll behavior

use super::*;
use crate::features::composer::components::composer_pill::composer_input_needs_stacked_layout;

impl ThreadView {
    pub(crate) fn handle_scroll_if_changed(&mut self) {
        let offset = self.scroll_handle.base_handle().offset();
        if self.last_scroll_offset == Some(offset) {
            return;
        }
        self.last_scroll_offset = Some(offset);
        self.motion_paused = true;
        self.last_scroll_activity = Some(Instant::now());
        let distance_to_bottom = self.scroll_handle.base_handle().max_offset().height + offset.y;
        if distance_to_bottom > Tokens::spacing_2() {
            self.user_scrolled_up = true;
            self.stick_to_bottom = false;
            self.pending_scroll_bottom = false;
        } else {
            self.user_scrolled_up = false;
            self.stick_to_bottom = true;
        }
    }

    pub(crate) fn clear_motion_pause_if_idle(&mut self) {
        if !self.motion_paused {
            return;
        }
        if let Some(t) = self.last_scroll_activity {
            if t.elapsed() >= Duration::from_millis(MOTION_PAUSE_MS) {
                self.motion_paused = false;
                self.last_scroll_activity = None;
            }
        }
    }

    pub(crate) fn animate(&self) -> bool {
        !self.motion_paused
    }

    pub(crate) fn end_spacer_height(&self) -> f32 {
        f32::from(Tokens::spacing_6())
    }

    pub(crate) fn composer_thread_inset_px(&self) -> f32 {
        0.0
    }

    pub(crate) fn refresh_end_spacer_size(&mut self) {
        let Some(ix) = self
            .manifest
            .iter()
            .rposition(|row| matches!(row, RowRef::EndSpacer))
        else {
            return;
        };
        let height = px(self.end_spacer_height());
        self.mutate_row_sizes(|sizes| {
            if ix < sizes.len() {
                sizes[ix].height = height;
            }
        });
    }

    /// Keep end-spacer height in sync with the live composer footer (pill + mode chips + overlays).
    pub(crate) fn sync_composer_footer(&mut self, cx: &Context<Self>) {
        let agent = self.agent.read(cx);
        let overlay = agent.composer_overlay_bar_height();
        let has_attachments = !agent.pending_image_attachments.is_empty();
        let has_error = agent.composer_error.is_some();
        let expanded = composer_input_needs_stacked_layout(&agent.input_state.read(cx).value())
            || has_attachments
            || has_error;
        if (overlay - self.composer_overlay_bar_height).abs() <= 0.5
            && expanded == self.composer_input_expanded
            && has_attachments == self.composer_has_attachments
            && has_error == self.composer_has_error
        {
            return;
        }
        self.composer_overlay_bar_height = overlay;
        self.composer_input_expanded = expanded;
        self.composer_has_attachments = has_attachments;
        self.composer_has_error = has_error;
        self.refresh_end_spacer_size();
        if self.stick_to_bottom && !self.user_scrolled_up {
            self.pending_scroll_bottom = true;
        }
    }
}

/// Preserve the viewport's local position inside a stable row across regrouping.
pub(super) fn scroll_anchor(
    rows: &[RowRef],
    sizes: &[gpui::Size<gpui::Pixels>],
    offset: gpui::Pixels,
) -> Option<(RowRef, gpui::Pixels)> {
    let mut top = px(0.0);
    for (&row, size) in rows.iter().zip(sizes) {
        if top + size.height > -offset {
            return Some((row, -offset - top));
        }
        top += size.height;
    }
    None
}
pub(super) fn anchored_offset(
    rows: &[RowRef],
    sizes: &[gpui::Size<gpui::Pixels>],
    (anchor, local): (RowRef, gpui::Pixels),
) -> Option<gpui::Pixels> {
    let index = rows.iter().position(|row| *row == anchor)?;
    Some(-sizes[..index].iter().fold(px(0.0), |sum, s| sum + s.height) - local)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folding_completed_commands_keeps_the_same_response_position_visible() {
        let old = [
            RowRef::ToolHeader { item_ix: 0 },
            RowRef::ToolHeader { item_ix: 1 },
            RowRef::AssistantMessage { item_ix: 2 },
        ];
        let sizes = [
            gpui::size(px(400.0), px(24.0)),
            gpui::size(px(400.0), px(24.0)),
            gpui::size(px(400.0), px(200.0)),
        ];
        let anchor = scroll_anchor(&old, &sizes, px(-70.0)).unwrap();
        let new = [
            RowRef::CommandGroup {
                item_ix: 0,
                count: 2,
            },
            RowRef::AssistantMessage { item_ix: 2 },
        ];
        assert_eq!(anchored_offset(&new, &sizes[..2], anchor), Some(px(-46.0)));
    }
}
