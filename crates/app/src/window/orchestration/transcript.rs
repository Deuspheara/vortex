//! Transcript orchestration — set_transcript_mode, cycle_transcript_mode.

use gpui::Context;

use super::super::AgentWindow;
use crate::shared::state::TranscriptMode;

impl AgentWindow {
    pub fn set_transcript_mode(&mut self, mode: TranscriptMode, cx: &mut Context<Self>) {
        self.transcript_mode = mode;
        if let Some(conv_id) = self.selected_conversation_id.clone() {
            self.sync_thread_view(conv_id, cx);
        }
        cx.notify();
    }

    pub fn cycle_transcript_mode(&mut self, cx: &mut Context<Self>) {
        self.set_transcript_mode(self.transcript_mode.cycle(), cx);
    }
}
