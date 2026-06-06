//! Todos orchestration — toggle_todo_strip.

use gpui::Context;

use super::super::AgentWindow;

impl AgentWindow {
    pub fn toggle_todo_strip(&mut self, cx: &mut Context<Self>) {
        self.todo_strip_expanded = !self.todo_strip_expanded;
        cx.notify();
    }
}
