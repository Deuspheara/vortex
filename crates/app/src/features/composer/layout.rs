//! Composer layout — structured footer with input pill, mode chips, and metadata.
//!
//! Three visual layers anchored at the bottom of the chat column:
//!   1. Metadata row (branch dropdown left + context usage ring right)
//!   2. Input pill (thin, rounded, focused on typing — with model dropdown inside)
//!   3. Mode chips (quiet row under the input)
//!
//! Blocking actions (approval, pending patch) overlay above the pill.
//! A soft gradient fade separates the bottom of the thread from the footer.
//! Provider is in the top bar, model is inside the input pill.

use std::rc::Rc;
use std::sync::Arc;

use agent_protocol::AgentMode;
use gpui::{Entity, IntoElement, Window, div, prelude::*, px};
use gpui_component::input::InputState;

use crate::features::agent_activity::components::approval::{ApprovalCardProps, approval_card};
use crate::features::agent_activity::components::pending_action_bar::{
    PendingActionBarProps, pending_action_bar,
};
use crate::features::composer::components::composer_pill::{ComposerPillProps, composer_pill};
use crate::features::composer::components::metadata_row::{MetadataRowProps, metadata_row};
use crate::features::composer::components::mode_chips::{ModeChipsProps, mode_chips};
use crate::features::composer::state::PendingImageAttachment;
use crate::shared::components::context_usage_ring::ContextUsageProps;
use crate::tokens::Tokens;
use crate::ui::agent_window::AgentWindow;

/// Gradient scrim at the bottom of the thread, behind the composer.
pub fn render_composer_fade(bottom_inset: f32) -> impl IntoElement {
    div()
        .id("composer-fade")
        .absolute()
        .bottom(px(bottom_inset))
        .left_0()
        .right_0()
        .h(px(Tokens::COMPOSER_FADE_HEIGHT))
        .bg(Tokens::composer_fade_gradient())
        .block_mouse_except_scroll()
}

/// Props for the composer layout.
pub struct ComposerProps {
    pub has_text: bool,
    pub input_expanded: bool,
    pub on_send: Option<Box<dyn Fn(&mut Window, &mut gpui::App) + 'static>>,
    pub is_running: bool,
    pub on_cancel: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
    pub input_entity: Entity<InputState>,
    pub selected_mode: AgentMode,
    pub context_usage: ContextUsageProps,
    pub pending_actions: PendingActionBarProps,
    pub sticky_approval: Option<ApprovalCardProps>,
    pub composer_dimmed: bool,
    pub composer_disabled: bool,
    pub entity: Entity<AgentWindow>,
    pub selected_branch: String,
    pub branch_items: Vec<String>,
    pub selected_model: String,
    pub model_items: Arc<[String]>,
    pub model_search_keys: Arc<[Arc<str>]>,
    pub pending_image_attachments: Vec<PendingImageAttachment>,
    pub composer_error: Option<String>,
}

/// Structured composer footer.
///
/// Layout (inside a centered column matching message width):
/// ┌─────────────────────────────────────┐
/// │  [approval / pending overlay]       │  ← blocking actions (when present)
/// │     ⌥ main    │              ◌      │  ← metadata (branch left, context right)
/// │  ┌─────────────────────────────┐   │
/// │  │ +  type your message...  ⌄ │   │  ← input pill (46–52px) with model dropdown
/// │  └─────────────────────────────┘   │
/// │     Plan  Ask  Edit  ●Apply Agent  │  ← mode chip row (8px below pill)
/// └─────────────────────────────────────┘
pub fn render_composer(props: ComposerProps) -> impl IntoElement {
    div()
        .id("composer-container")
        .w_full()
        .relative()
        .bg(Tokens::composer_footer_gradient())
        .block_mouse_except_scroll()
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .px(Tokens::thread_padding_x())
                .pb(Tokens::spacing_5())
                .pt(Tokens::spacing_2())
                // ══ Blocking action overlay ══
                .child(if let Some(approval) = props.sticky_approval {
                    approval_card(approval).into_any_element()
                } else {
                    pending_action_bar(props.pending_actions).into_any_element()
                })
                // ══ Layer 1: Metadata row (branch left, context right) ══
                .child(metadata_row(MetadataRowProps {
                    branch: props.selected_branch,
                    branch_items: props.branch_items,
                    context_usage: props.context_usage,
                    entity: props.entity.clone(),
                }))
                // ══ Layer 2: Input pill (with model dropdown inside) ══
                .child(composer_pill(ComposerPillProps {
                    has_text: props.has_text,
                    input_expanded: props.input_expanded,
                    input_entity: props.input_entity.clone(),
                    is_running: props.is_running,
                    on_send: props.on_send,
                    on_cancel: props.on_cancel,
                    selected_model: props.selected_model,
                    model_items: props.model_items,
                    model_search_keys: props.model_search_keys,
                    pending_image_attachments: props.pending_image_attachments,
                    composer_error: props.composer_error,
                    entity: props.entity.clone(),
                }))
                // ══ Layer 3: Mode chips (8px below pill) ══
                .child(render_mode_row(&props.selected_mode, props.entity.clone())),
        )
        .when(props.composer_dimmed, |el| el.opacity(0.72))
        .when(props.composer_disabled, |el| el.opacity(0.5))
}

// ── Mode chips row ──

fn render_mode_row(selected_mode: &AgentMode, entity: Entity<AgentWindow>) -> impl IntoElement {
    div()
        .id("composer-mode-row")
        .w_full()
        .max_w(px(Tokens::COMPOSER_MAX_WIDTH))
        .px(Tokens::composer_rail_inset_x())
        .flex()
        .items_center()
        .justify_between()
        .pt(Tokens::spacing_2())
        .child(mode_chips(ModeChipsProps {
            selected_mode: selected_mode.clone(),
            on_select: Rc::new(move |mode, app| {
                entity.update(app, |view, cx| {
                    view.set_safety_mode(mode, cx);
                });
            }),
        }))
}
