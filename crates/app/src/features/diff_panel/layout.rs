//! Right-side diff panel — focused review surface.

use std::rc::Rc;

use gpui::{Entity, FontWeight, IntoElement, div, prelude::*, px};
use gpui_component::Selectable;
use gpui_component::button::{Button, ButtonVariants};

use crate::features::agent_activity::components::approval::diff_approval_bar;
use crate::features::shell::state::{
    DiffFile, DiffHunk, DiffPanelState, DiffRow, PlanArtifact, PlanExecutionState, ReviewPanelTab,
};
use crate::features::todos::components::todo_list;
use crate::shared::components::buttons::{btn_approve, btn_ghost_label};
use crate::tokens::Tokens;
use crate::tokens::icons;
use crate::ui::agent_window::AgentWindow;

/// Demo diff content for the active thread.
#[allow(dead_code)]
pub fn demo_diff_files() -> Vec<DiffFile> {
    vec![
        build_file(
            "crates/app/src/main.rs",
            7,
            1,
            vec![
                row_collapsed(4),
                row_hunk("@@ -1,8 +1,35 @@"),
                row_context(1, 1, "mod ui;"),
                row_added(2, ""),
                row_added(3, "use gpui::{App, Application};"),
                row_added(4, "use gpui_component::{Root, TitleBar};"),
                row_added(5, ""),
                row_added(6, "use ui::agent_window::AgentWindow;"),
                row_removed(7, "fn main() {"),
                row_added(7, "fn main() {"),
                row_added(8, "    Application::new().run(|cx: &mut App| {"),
            ],
        ),
        build_file(
            "crates/app/src/ui/agent_window.rs",
            2,
            0,
            vec![
                row_hunk("@@ -0,0 +1,12 @@"),
                row_added(1, "pub struct AgentWindow {"),
                row_added(2, "    pub projects: Vec<Project>,"),
            ],
        ),
        build_file(
            "crates/app/Cargo.toml",
            1,
            0,
            vec![
                row_hunk("@@ -8,6 +8,7 @@"),
                row_context(8, 8, "[dependencies]"),
                row_context(9, 9, "gpui = \"0.2\""),
                row_added(10, "gpui-component = \"0.5\""),
            ],
        ),
    ]
}

#[allow(dead_code)]
fn build_file(path: &str, added: usize, removed: usize, rows: Vec<DiffRow>) -> DiffFile {
    let flat_rows = rows.clone();
    DiffFile {
        path: path.into(),
        added,
        removed,
        hunks: vec![crate::features::shell::state::DiffHunk {
            old_start: 1,
            new_start: 1,
            rows,
        }],
        flat_rows,
    }
}

#[allow(dead_code)]
fn row_collapsed(count: usize) -> DiffRow {
    DiffRow::Collapsed { count }
}

#[allow(dead_code)]
fn row_hunk(label: &str) -> DiffRow {
    DiffRow::HunkHeader {
        label: label.into(),
    }
}

#[allow(dead_code)]
fn row_context(old: usize, new: usize, text: &str) -> DiffRow {
    DiffRow::Context {
        old_line: old,
        new_line: new,
        text: text.into(),
    }
}

#[allow(dead_code)]
fn row_added(new: usize, text: &str) -> DiffRow {
    DiffRow::Added {
        new_line: new,
        text: text.into(),
    }
}

#[allow(dead_code)]
fn row_removed(old: usize, text: &str) -> DiffRow {
    DiffRow::Removed {
        old_line: old,
        text: text.into(),
    }
}

pub struct DiffPanelProps {
    pub state: DiffPanelState,
    pub plan_artifact: Option<PlanArtifact>,
    pub plan_available: bool,
    pub can_implement_plan: bool,
    pub show_implement_choice: bool,
    pub recommend_fresh_context: bool,
    pub entity: Entity<AgentWindow>,
    /// False after Approve/Reject was clicked (prevents duplicate commands).
    pub approval_actions_enabled: bool,
    pub pending_patch_id: Option<String>,
    pub patch_apply_enabled: bool,
    pub show_close_button: bool,
}

#[allow(dead_code)]
pub fn render_diff_panel(props: DiffPanelProps) -> impl IntoElement {
    render_diff_panel_with_width(props, Tokens::DIFF_PANEL_WIDTH)
}

pub fn render_diff_panel_with_width(props: DiffPanelProps, width: f32) -> impl IntoElement {
    if !props.state.open {
        return div()
            .id("diff-panel-closed")
            .w(px(0.0))
            .overflow_hidden()
            .into_any_element();
    }

    let files = props.state.files;
    let active_tab = props.state.active_tab;
    let presentation = props.state.presentation.clone();
    let (total_add, total_del) = count_totals(&files);
    let approval = props.state.pending_approval.clone();
    let applied = props.state.applied;
    let change_status = if files.is_empty() {
        None
    } else if applied {
        Some("Applied")
    } else if approval.is_some() {
        Some("Awaiting approval")
    } else if props.pending_patch_id.is_some() {
        Some("Preview")
    } else {
        Some("Not applied")
    };
    let show_apply = props.pending_patch_id.is_some() && props.patch_apply_enabled;
    let plan_artifact = props.plan_artifact;
    let plan_available = props.plan_available || plan_artifact.is_some();
    let active_tab = if active_tab == ReviewPanelTab::Plan && !plan_available {
        ReviewPanelTab::Changes
    } else {
        active_tab
    };

    let mut panel = div()
        .id("diff-panel")
        .w_full()
        .min_w(px(Tokens::DIFF_PANEL_MIN_WIDTH))
        .h_full()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .bg(Tokens::panel_bg())
        .overflow_hidden()
        .child(render_header(
            props.entity.clone(),
            active_tab,
            change_status,
            total_add,
            total_del,
            applied,
            show_apply,
            props.show_close_button,
            &presentation,
        ));

    if active_tab == ReviewPanelTab::Changes {
        let entity = props.entity.clone();
        let empty = files.is_empty();
        let review = super::review::DiffReview(super::review::ReviewConfig {
            selected_file: props.state.selected_file,
            files: Rc::new(files),
            presentation: presentation.clone(),
            width,
            on_toggle: Rc::new(move |path, app| {
                entity.update(app, |view, cx| view.toggle_diff_file(path, cx))
            }),
        });
        panel = panel.child(
            div()
                .id("diff-panel-body")
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.0))
                .min_w(px(0.0))
                .overflow_hidden()
                .child(if empty {
                    render_empty_changes_state().into_any_element()
                } else {
                    review.into_any_element()
                }),
        );
    } else if let Some(artifact) = plan_artifact {
        let action_artifact = artifact.clone();
        let body_artifact = artifact;
        panel = panel.child(
            div()
                .id("review-plan-pane")
                .flex_1()
                .min_h(px(0.0))
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(
                    div()
                        .id("review-plan-body")
                        .flex_1()
                        .min_h(px(0.0))
                        .overflow_y_scroll()
                        .p(Tokens::spacing_3())
                        .child(todo_list::plan_artifact(body_artifact).into_any_element()),
                )
                .child(render_plan_action_strip(
                    props.entity.clone(),
                    &action_artifact,
                    props.can_implement_plan,
                    props.show_implement_choice,
                    props.recommend_fresh_context,
                    width,
                )),
        );
    } else {
        panel = panel.child(render_empty_plan_state());
    }

    panel
        .when(active_tab == ReviewPanelTab::Changes, |el| {
            el.when_some(approval, |el, req| {
                let entity = props.entity.clone();
                let entity_approve = entity.clone();
                el.child(diff_approval_bar(
                    &req.title,
                    &req.risk,
                    props.approval_actions_enabled,
                    move |app: &mut gpui::App| {
                        entity.update(app, |view, cx| {
                            view.reject_pending(None, cx);
                        });
                    },
                    move |app: &mut gpui::App| {
                        entity_approve.update(app, |view, cx| {
                            view.approve_pending(cx);
                        });
                    },
                ))
            })
        })
        .into_any_element()
}

fn render_plan_action_strip(
    entity: Entity<AgentWindow>,
    artifact: &PlanArtifact,
    can_implement: bool,
    show_choice: bool,
    recommend_fresh_context: bool,
    width: f32,
) -> impl IntoElement {
    let can_restart = matches!(artifact.execution_state, PlanExecutionState::Completed);
    let can_start = can_implement && (artifact.execution_state.can_start() || can_restart);
    let stacked = width < 520.0 || show_choice;
    let state_label = artifact.execution_state.label();
    let primary_label = plan_primary_action_label(artifact.execution_state);
    let show_entity = entity.clone();
    let fresh_entity = entity.clone();
    let continue_entity = entity.clone();

    div()
        .id("plan-action-strip")
        .flex_shrink_0()
        .px(Tokens::spacing_3())
        .py(Tokens::spacing_2())
        .border_t_1()
        .border_color(Tokens::border_subtle())
        .bg(Tokens::panel_bg())
        .child(
            div()
                .w_full()
                .px(Tokens::spacing_3())
                .py(Tokens::spacing_2())
                .rounded(Tokens::radius_md())
                .bg(Tokens::surface())
                .border_1()
                .border_color(Tokens::border_subtle())
                .flex()
                .flex_col()
                .gap(Tokens::spacing_2())
                .child(
                    div()
                        .flex()
                        .gap(Tokens::spacing_3())
                        .when(stacked, |el| el.flex_col().items_start())
                        .when(!stacked, |el| el.items_center().justify_between())
                        .child(
                            div()
                                .min_w(px(0.0))
                                .flex()
                                .flex_col()
                                .gap(Tokens::spacing_0p5())
                                .child(
                                    div()
                                        .text_size(Tokens::text_sm())
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(Tokens::text_primary())
                                        .child("Review plan"),
                                )
                                .child(
                                    div()
                                        .text_size(Tokens::text_xs())
                                        .text_color(Tokens::text_secondary())
                                        .child(if show_choice {
                                            if recommend_fresh_context {
                                                "Choose where to implement this plan · Fresh context recommended"
                                                    .to_string()
                                            } else {
                                                "Choose where to implement this plan · Continuing here is recommended"
                                                    .to_string()
                                            }
                                        } else {
                                            format!("Status · {state_label}")
                                        }),
                                ),
                        )
                        .when(can_start && !show_choice, |el| {
                            el.child(
                                btn_approve("show-implement-plan-choice", primary_label)
                                    .on_click(move |_, _, app: &mut gpui::App| {
                                        show_entity.update(app, |view, cx| {
                                            view.show_plan_implementation_choice(cx);
                                        });
                                    }),
                            )
                        })
                        .when(!can_start && !show_choice, |el| {
                            el.child(
                                div()
                                    .text_size(Tokens::text_xs())
                                    .text_color(Tokens::text_faint())
                                    .child(if can_implement {
                                        "Implementation is already in progress or waiting on approval"
                                    } else {
                                        "Agent is running"
                                    }),
                            )
                        }),
                )
                .when(can_start && show_choice, |el| {
                    el.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(Tokens::spacing_2())
                            .child(btn_approve("implement-plan-fresh", "Start fresh").on_click(
                                move |_, _, app: &mut gpui::App| {
                                    fresh_entity.update(app, |view, cx| {
                                        view.implement_plan_fresh(cx);
                                    });
                                },
                            ))
                            .child(btn_ghost_label("implement-plan-here", "Continue here").on_click(
                                move |_, _, app: &mut gpui::App| {
                                    continue_entity.update(app, |view, cx| {
                                        view.implement_plan_here(cx);
                                    });
                                },
                            )),
                    )
                }),
        )
}

fn plan_primary_action_label(state: PlanExecutionState) -> &'static str {
    match state {
        PlanExecutionState::Completed => "Implement again",
        PlanExecutionState::Failed | PlanExecutionState::Cancelled | PlanExecutionState::Stale => {
            "Retry plan"
        }
        _ => "Implement plan",
    }
}

fn count_totals(files: &[DiffFile]) -> (usize, usize) {
    files
        .iter()
        .map(|f| (f.added, f.removed))
        .fold((0, 0), |(a, d), (fa, fd)| (a + fa, d + fd))
}

fn render_empty_changes_state() -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .p(Tokens::spacing_6())
        .text_size(Tokens::text_sm())
        .text_color(Tokens::text_tertiary())
        .child("No changes yet")
}

fn render_empty_plan_state() -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .p(Tokens::spacing_6())
        .text_size(Tokens::text_sm())
        .text_color(Tokens::text_tertiary())
        .child("No plan yet — run Plan mode to create a reviewed implementation plan")
}

fn render_header(
    entity: Entity<AgentWindow>,
    active_tab: ReviewPanelTab,
    change_status: Option<&'static str>,
    total_add: usize,
    total_del: usize,
    applied: bool,
    show_apply: bool,
    show_close_button: bool,
    presentation: &super::state::DiffPresentation,
) -> impl IntoElement {
    let apply_entity = entity.clone();
    let changes_selected = active_tab == ReviewPanelTab::Changes;
    let split = presentation.split;
    let wrap = presentation.wrap;
    let tree = presentation.show_tree;

    div()
        .h(px(Tokens::DIFF_HEADER_HEIGHT))
        .px(Tokens::spacing_3())
        .flex()
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(Tokens::border_subtle())
        .child(
            div()
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .when(changes_selected, |el| {
                    el.child(
                        div()
                            .text_size(Tokens::text_code())
                            .text_color(Tokens::success())
                            .child(format!("+{total_add}")),
                    )
                    .child(
                        div()
                            .text_size(Tokens::text_code())
                            .text_color(Tokens::danger())
                            .child(format!("−{total_del}")),
                    )
                    .when_some(change_status, |el, status| {
                        el.child(
                            div()
                                .text_size(Tokens::text_xs())
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if applied {
                                    Tokens::success()
                                } else {
                                    Tokens::text_secondary()
                                })
                                .child(status),
                        )
                    })
                })
                .when(!changes_selected, |el| {
                    el.child(
                        div()
                            .text_size(Tokens::text_sm())
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Tokens::text_primary())
                            .child("Plan review"),
                    )
                }),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(Tokens::spacing_1())
                .when(changes_selected, |el| {
                    let fold = entity.clone();
                    let unified = entity.clone();
                    let side = entity.clone();
                    let wrapping = entity.clone();
                    let files = entity.clone();
                    el.child(
                        Button::new("diff-collapse-all")
                            .icon(gpui_component::IconName::ChevronsUpDown)
                            .ghost()
                            .compact()
                            .tooltip("Collapse or expand all files")
                            .on_click(move |_, _, app| {
                                fold.update(app, |view, cx| view.toggle_diff_collapse_all(cx))
                            }),
                    )
                    .child(
                        Button::new("diff-stacked")
                            .icon(icons::PANEL_BOTTOM)
                            .ghost()
                            .compact()
                            .selected(!split)
                            .tooltip("Stacked diff view")
                            .on_click(move |_, _, app| {
                                unified.update(app, |view, cx| view.set_diff_split(false, cx))
                            }),
                    )
                    .child(
                        Button::new("diff-split")
                            .icon(icons::PANEL_RIGHT)
                            .ghost()
                            .compact()
                            .selected(split)
                            .tooltip("Split diff view")
                            .on_click(move |_, _, app| {
                                side.update(app, |view, cx| view.set_diff_split(true, cx))
                            }),
                    )
                    .child(
                        Button::new("diff-wrap")
                            .icon(crate::tokens::icons::wrap_lines())
                            .ghost()
                            .compact()
                            .selected(wrap)
                            .tooltip("Toggle line wrapping")
                            .on_click(move |_, _, app| {
                                wrapping.update(app, |view, cx| view.toggle_diff_wrap(cx))
                            }),
                    )
                    .child(
                        Button::new("diff-tree")
                            .icon(icons::PANEL_LEFT)
                            .ghost()
                            .compact()
                            .selected(tree)
                            .tooltip("Toggle file tree")
                            .on_click(move |_, _, app| {
                                files.update(app, |view, cx| view.toggle_diff_tree(cx))
                            }),
                    )
                })
                .when(show_apply && !applied && changes_selected, |el| {
                    el.child(
                        btn_approve("apply-patch", "Apply").on_click(move |_, _, cx| {
                            apply_entity.update(cx, |view, cx| {
                                view.apply_pending_patch(cx);
                            });
                        }),
                    )
                })
                .when(show_close_button, |el| {
                    el.child(
                        Button::new("close-diff-panel")
                            .icon(icons::CLOSE)
                            .ghost()
                            .compact()
                            .on_click(move |_, _, app: &mut gpui::App| {
                                entity.update(app, |view, cx| view.close_diff_panel(cx));
                            }),
                    )
                }),
        )
}

pub fn parse_unified_diff(unified_diff: &str) -> Vec<DiffFile> {
    let mut files = Vec::new();
    let mut current_path: Option<String> = None;
    let mut old_path = String::new();
    let mut rows: Vec<DiffRow> = Vec::new();
    let mut added = 0usize;
    let mut removed = 0usize;
    let mut old_line = 0usize;
    let mut new_line = 0usize;

    let flush = |path: Option<String>,
                 rows: &mut Vec<DiffRow>,
                 added: &mut usize,
                 removed: &mut usize,
                 files: &mut Vec<DiffFile>| {
        if let Some(path) = path {
            if !rows.is_empty() {
                let taken = std::mem::take(rows);
                files.push(DiffFile {
                    path,
                    added: *added,
                    removed: *removed,
                    hunks: vec![DiffHunk {
                        old_start: 1,
                        new_start: 1,
                        rows: taken.clone(),
                    }],
                    flat_rows: taken,
                });
            }
        }
        *added = 0;
        *removed = 0;
    };

    for line in unified_diff.lines() {
        if let Some(path) = line.strip_prefix("--- ") {
            old_path = path.strip_prefix("a/").unwrap_or(path).trim().to_string();
            continue;
        }
        if line.starts_with("+++ ") {
            flush(
                current_path.take(),
                &mut rows,
                &mut added,
                &mut removed,
                &mut files,
            );
            let path = line
                .strip_prefix("+++ b/")
                .or_else(|| line.strip_prefix("+++ a/"))
                .or_else(|| line.strip_prefix("+++ "))
                .unwrap_or("")
                .trim()
                .to_string();
            let path = if path == "/dev/null" {
                old_path.clone()
            } else {
                path
            };
            if !path.is_empty() && path != "/dev/null" {
                current_path = Some(path);
            }
            old_line = 0;
            new_line = 0;
            continue;
        }
        if line.starts_with("@@") {
            rows.push(DiffRow::HunkHeader {
                label: line.to_string(),
            });
            if let Some(rest) = line.split('+').nth(1) {
                if let Some(n) = rest
                    .split(',')
                    .next()
                    .and_then(|s| s.trim().parse::<usize>().ok())
                {
                    new_line = n.saturating_sub(1);
                }
            }
            if let Some(rest) = line.split('-').nth(1) {
                if let Some(n) = rest
                    .split(',')
                    .next()
                    .and_then(|s| s.trim().parse::<usize>().ok())
                {
                    old_line = n.saturating_sub(1);
                }
            }
            continue;
        }
        if line.starts_with('+') && !line.starts_with("+++") {
            new_line += 1;
            added += 1;
            rows.push(DiffRow::Added {
                new_line,
                text: line[1..].to_string(),
            });
        } else if line.starts_with('-') && !line.starts_with("---") {
            old_line += 1;
            removed += 1;
            rows.push(DiffRow::Removed {
                old_line,
                text: line[1..].to_string(),
            });
        } else if line.starts_with(' ') {
            old_line += 1;
            new_line += 1;
            rows.push(DiffRow::Context {
                old_line,
                new_line,
                text: line[1..].to_string(),
            });
        }
    }
    flush(
        current_path,
        &mut rows,
        &mut added,
        &mut removed,
        &mut files,
    );
    files
}

#[cfg(test)]
mod codex_diff_tests {
    use super::*;
    #[test]
    fn deleted_files_keep_their_path_and_line_counts() {
        let files = parse_unified_diff("--- a/proof.txt\n+++ /dev/null\n@@ -1,1 +0,0 @@\n-proof\n");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "proof.txt");
        assert_eq!((files[0].added, files[0].removed), (0, 1));
    }
}
