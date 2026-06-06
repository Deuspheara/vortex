//! Top-bar session context — project/branch pickers and a single status badge.

use std::rc::Rc;

use gpui::{Entity, FontWeight, IntoElement, div, prelude::*, px};

use crate::features::shell::state::SessionRunState;
use crate::shared::components::dropdown::{
    DropdownAnchor, DropdownItem, PickerDropdownProps, picker_dropdown,
};
use crate::tokens::Tokens;
use crate::tokens::icons;
use crate::ui::agent_window::AgentWindow;

pub struct SessionContextProps {
    pub selected_project: String,
    pub project_items: Vec<String>,
    pub selected_branch: String,
    pub branch_items: Vec<String>,
    pub conversation_title: String,
    pub show_pending_patch_badge: bool,
    pub session_run_state: SessionRunState,
    pub entity: Entity<AgentWindow>,
}

pub fn session_context(props: SessionContextProps) -> impl IntoElement {
    let entity = props.entity.clone();

    div()
        .id("session-context")
        .flex()
        .items_center()
        .gap(Tokens::spacing_1())
        .min_w(px(0.0))
        .flex_1()
        .overflow_hidden()
        .child(render_project_picker(
            props.selected_project,
            props.project_items,
            entity.clone(),
        ))
        .child(
            div()
                .text_size(Tokens::text_xs())
                .text_color(Tokens::text_tertiary())
                .flex_shrink_0()
                .child("·"),
        )
        .child(render_branch_picker(
            props.selected_branch,
            props.branch_items,
            entity,
        ))
        .child(
            div()
                .text_size(Tokens::text_sm())
                .text_color(Tokens::topbar_title_muted())
                .flex_shrink_0()
                .child("/"),
        )
        .child(
            div()
                .text_size(Tokens::text_sm())
                .text_color(Tokens::text_secondary())
                .overflow_hidden()
                .text_ellipsis()
                .flex_1()
                .min_w(px(0.0))
                .child(props.conversation_title),
        )
        .when_some(
            primary_status_badge(props.show_pending_patch_badge, props.session_run_state),
            |el, (label, color)| el.child(status_badge(label, color)),
        )
}

fn render_project_picker(
    selected_project: String,
    project_items: Vec<String>,
    entity: Entity<AgentWindow>,
) -> impl IntoElement {
    let items: Vec<DropdownItem> = project_items
        .into_iter()
        .map(|name| DropdownItem {
            label: name,
            icon: Some(icons::FOLDER),
        })
        .collect();

    picker_dropdown(PickerDropdownProps {
        id: "topbar-project-picker".into(),
        label: selected_project.clone(),
        items,
        selected: Some(selected_project),
        anchor: DropdownAnchor::Below,
        menu_min_width: 160.0,
        trigger_icon: None,
        searchable: false,
        search_texts: None,
        search_placeholder: None,
        on_select: Rc::new(move |_, project, app| {
            entity.update(app, |view, cx| {
                view.on_composer_project_selected(project, cx);
            });
        }),
    })
}

fn render_branch_picker(
    selected_branch: String,
    branch_items: Vec<String>,
    entity: Entity<AgentWindow>,
) -> impl IntoElement {
    let items: Vec<DropdownItem> = branch_items
        .into_iter()
        .map(|name| DropdownItem {
            label: name,
            icon: Some(icons::GIT_BRANCH),
        })
        .collect();

    picker_dropdown(PickerDropdownProps {
        id: "topbar-branch-picker".into(),
        label: selected_branch.clone(),
        items,
        selected: Some(selected_branch),
        anchor: DropdownAnchor::Below,
        menu_min_width: 140.0,
        trigger_icon: None,
        searchable: false,
        search_texts: None,
        search_placeholder: None,
        on_select: Rc::new(move |_, branch, app| {
            entity.update(app, |view, cx| {
                view.on_composer_branch_selected(branch, cx);
            });
        }),
    })
}

fn status_badge(label: &'static str, color: gpui::Hsla) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .ml(Tokens::spacing_1())
        .px(Tokens::spacing_1p5())
        .py(Tokens::spacing_0p5())
        .rounded(Tokens::radius_xs())
        .bg(Tokens::surface_hover())
        .text_size(Tokens::text_xs())
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(label.to_string())
}

fn primary_status_badge(
    pending_patch: bool,
    state: SessionRunState,
) -> Option<(&'static str, gpui::Hsla)> {
    if state == SessionRunState::WaitingApproval {
        return Some(("Awaiting approval", Tokens::warning()));
    }
    if pending_patch {
        return Some(("Unapplied changes", Tokens::accent()));
    }
    match state {
        SessionRunState::Running => Some(("Running", Tokens::text_tertiary())),
        SessionRunState::Planning => Some(("Planning", Tokens::text_tertiary())),
        _ => None,
    }
}
