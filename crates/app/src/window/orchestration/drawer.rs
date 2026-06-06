//! Drawer orchestration — open/close_drawer.

use gpui::Context;

use super::super::AgentWindow;
use crate::features::shell::state::DrawerMode;

impl AgentWindow {
    pub fn open_drawer(&mut self, mode: DrawerMode, cx: &mut Context<Self>) {
        self.drawer.mode = mode;
        cx.notify();
    }

    pub fn close_drawer(&mut self, cx: &mut Context<Self>) {
        self.drawer.mode = DrawerMode::Hidden;
        cx.notify();
    }
}
