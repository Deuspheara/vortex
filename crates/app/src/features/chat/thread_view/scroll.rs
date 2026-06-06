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
        if offset.y < px(-8.0) {
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
        Tokens::composer_stack_total(
            self.composer_overlay_bar_height,
            self.composer_input_expanded,
            self.composer_has_attachments,
            self.composer_has_error,
        )
    }

    pub(crate) fn composer_thread_inset_px(&self) -> f32 {
        Tokens::composer_thread_inset_px(
            self.composer_overlay_bar_height,
            self.composer_input_expanded,
            self.composer_has_attachments,
            self.composer_has_error,
        )
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
