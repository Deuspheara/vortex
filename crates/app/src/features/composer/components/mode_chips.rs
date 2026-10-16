//! Compact permission selector inside the composer toolbar.
use crate::shared::components::dropdown::{
    DropdownAnchor, DropdownItem, PickerDropdownProps, picker_dropdown,
};
use crate::tokens::icons;
use agent_protocol::AgentMode;
use gpui::IntoElement;
use std::rc::Rc;

pub struct ModeChipsProps {
    pub codex: bool,
    pub selected_mode: AgentMode,
    pub on_select: Rc<dyn Fn(AgentMode, &mut gpui::App) + 'static>,
}

pub fn mode_chips(props: ModeChipsProps) -> impl IntoElement {
    let modes = [
        (AgentMode::PlanOnly, "Plan · read only"),
        (AgentMode::ChatOnly, "Ask · no tools"),
        (AgentMode::SuggestPatch, "Suggest edits"),
        (AgentMode::ApplyWithApproval, "Approval required"),
        (AgentMode::FullAccessDangerous, "Full access"),
    ]
    .into_iter()
    .filter(|(mode, _)| {
        !props.codex || matches!(mode, AgentMode::PlanOnly | AgentMode::ApplyWithApproval)
    })
    .collect::<Vec<_>>();
    let selected = modes
        .iter()
        .find(|(mode, _)| *mode == props.selected_mode)
        .map(|(_, label)| *label)
        .unwrap_or("Approval required")
        .to_string();
    picker_dropdown(PickerDropdownProps {
        id: "composer-permissions".into(),
        label: selected.clone(),
        items: modes
            .iter()
            .map(|(_, label)| DropdownItem {
                label: label.to_string(),
                icon: None,
            })
            .collect(),
        selected: Some(selected),
        anchor: DropdownAnchor::Above,
        menu_min_width: 200.0,
        trigger_icon: Some(icons::shield()),
        searchable: false,
        search_texts: None,
        search_placeholder: None,
        on_select: Rc::new(move |index, _, app| {
            if let Some((mode, _)) = modes.get(index) {
                (props.on_select)(mode.clone(), app);
            }
        }),
    })
}
