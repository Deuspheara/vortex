//! Metadata row — secondary context info above the composer pill.
//!
//! Shows the current branch as a compact dropdown on the left
//! and the context usage ring on the right.
//! Provider is in the top bar; model is inside the input pill.

use std::rc::Rc;

use gpui::{Entity, IntoElement, div, prelude::*, px};

use crate::shared::components::context_usage_ring::{ContextUsageProps, context_usage_ring};
use crate::shared::components::dropdown::{
    DropdownAnchor, DropdownItem, PickerDropdownProps, picker_dropdown,
};
use crate::tokens::Tokens;
use crate::tokens::icons;
use crate::ui::agent_window::AgentWindow;

pub struct MetadataRowProps {
    pub branch: String,
    pub branch_items: Vec<String>,
    pub context_usage: ContextUsageProps,
    pub entity: Entity<AgentWindow>,
}

/// Secondary metadata row under the composer.
/// Left side: branch dropdown. Right side: context usage ring.
pub fn metadata_row(props: MetadataRowProps) -> impl IntoElement {
    let has_branch = !props.branch.is_empty() && props.branch != "default";

    div()
        .id("composer-metadata")
        .w_full()
        .max_w(px(Tokens::COMPOSER_MAX_WIDTH))
        .px(Tokens::composer_rail_inset_x())
        .h(px(Tokens::ROW_HEIGHT_SM))
        .flex()
        .items_center()
        .justify_between()
        .pb(px(6.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .min_w(px(0.0))
                .overflow_hidden()
                .when(has_branch, |el| {
                    el.child(render_branch_dropdown(
                        &props.branch,
                        &props.branch_items,
                        props.entity.clone(),
                    ))
                }),
        )
        .child(context_usage_ring(props.context_usage))
}

// ── Branch dropdown ──

fn render_branch_dropdown(
    selected: &str,
    branch_items: &[String],
    entity: Entity<AgentWindow>,
) -> impl IntoElement {
    // Ensure the current branch always appears, even when git returns nothing.
    let mut deduped = branch_items.to_vec();
    if !selected.is_empty() && !deduped.iter().any(|b| b == selected) {
        deduped.insert(0, selected.to_string());
    }
    let items: Vec<DropdownItem> = deduped
        .iter()
        .map(|b| DropdownItem {
            label: b.clone(),
            icon: Some(icons::GIT_BRANCH),
        })
        .collect();

    picker_dropdown(PickerDropdownProps {
        id: "metadata-branch".into(),
        label: selected.to_string(),
        items,
        selected: Some(selected.to_string()),
        anchor: DropdownAnchor::Below,
        menu_min_width: 140.0,
        trigger_icon: Some(icons::GIT_BRANCH),
        searchable: false,
        search_texts: None,
        search_placeholder: None,
        on_select: Rc::new(move |_index, selected, app| {
            entity.update(app, |view, cx| {
                view.on_composer_branch_selected(selected, cx);
            });
        }),
    })
}
