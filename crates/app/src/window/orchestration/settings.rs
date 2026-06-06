//! Settings orchestration — open/close_settings, set_appearance_mode, apply_color_theme, set_safety_mode.

use gpui::{Context, Window};

use super::super::AgentWindow;
use crate::features::shell::state::InspectorView;
use crate::tokens::theme::{apply_theme, set_appearance_mode};
use gpui_component::ThemeMode;

impl AgentWindow {
    pub fn open_chat(&mut self, cx: &mut Context<Self>) {
        self.screen = crate::window::AppScreen::Chat;
        cx.notify();
    }

    pub fn open_search(&mut self, cx: &mut Context<Self>) {
        self.screen = crate::window::AppScreen::Search;
        cx.notify();
    }

    pub fn open_extensions(&mut self, cx: &mut Context<Self>) {
        self.screen = crate::window::AppScreen::Extensions;
        cx.notify();
    }

    pub fn open_automations(&mut self, cx: &mut Context<Self>) {
        self.screen = crate::window::AppScreen::Automations;
        cx.notify();
    }

    pub fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.screen = crate::window::AppScreen::Settings;
        cx.notify();
    }

    pub fn open_context_workspace_panel(&mut self, cx: &mut Context<Self>) {
        self.screen = crate::window::AppScreen::Chat;
        self.select_inspector_view(InspectorView::Context, cx);
    }

    pub fn close_settings(&mut self, cx: &mut Context<Self>) {
        self.open_chat(cx);
    }

    pub fn set_appearance_mode(
        &mut self,
        mode: ThemeMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        set_appearance_mode(mode, Some(window), cx);
        cx.notify();
    }

    pub fn apply_color_theme(
        &mut self,
        name: &str,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        apply_theme(name, window, cx);
        cx.notify();
    }

    pub fn set_safety_mode(&mut self, mode: agent_protocol::AgentMode, cx: &mut Context<Self>) {
        self.safety_mode = mode;
        cx.notify();
    }

    pub fn rollback_checkpoint(
        &mut self,
        checkpoint_id: agent_protocol::CheckpointId,
        cx: &mut Context<Self>,
    ) {
        self.agent_bridge
            .send(agent_protocol::AgentCommand::RollbackCheckpoint { checkpoint_id })
            .ok();
        cx.notify();
    }
}
