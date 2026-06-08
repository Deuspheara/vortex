//! Right-side diff panel — focused review surface.

use std::rc::Rc;

use gpui::{Entity, FontWeight, Hsla, IntoElement, div, prelude::*, px, uniform_list};
use gpui_component::Icon;
use gpui_component::button::{Button, ButtonVariants};

use crate::features::agent_activity::components::approval::diff_approval_bar;
use crate::features::shell::state::{
    DiffFile, DiffHunk, DiffPanelState, DiffRow, DiffRowKind, PlanArtifact, ReviewPanelTab,
};
use crate::features::todos::components::todo_list;
use crate::shared::components::buttons::{btn_approve, btn_ghost_label};
use crate::shared::components::tab_bar::{TabBarProps, TabItem, tab_bar};
use crate::tokens::icons;
use crate::tokens::{Tokens, element_key};
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

pub fn render_diff_panel_with_width(props: DiffPanelProps, _width: f32) -> impl IntoElement {
    if !props.state.open {
        return div()
            .id("diff-panel-closed")
            .w(px(0.0))
            .overflow_hidden()
            .into_any_element();
    }

    let files = props.state.files;
    let active_tab = props.state.active_tab;
    let selected = props.state.selected_file.min(files.len().saturating_sub(1));
    let selected_file = files.get(selected).cloned();
    let (total_add, total_del) = count_totals(&files);
    let approval = props.state.pending_approval.clone();
    let applied = props.state.applied;
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
            plan_available,
            total_add,
            total_del,
            applied,
            show_apply,
            props.show_close_button,
        ));

    if active_tab == ReviewPanelTab::Changes {
        let is_single = files.len() == 1;
        if files.len() > 1 {
            panel = panel.child(render_file_tabs(&files, selected, props.entity.clone()));
        }
        panel = panel.child(
            div()
                .id("diff-panel-body")
                .flex_1()
                .min_h(px(0.0))
                .overflow_hidden()
                .child(if let Some(file) = selected_file {
                    if is_single {
                        render_file_diff(file, DiffFileHeaderMode::SingleFileCompact)
                            .into_any_element()
                    } else {
                        render_file_diff(file, DiffFileHeaderMode::MultiFileTab).into_any_element()
                    }
                } else {
                    render_empty_changes_state().into_any_element()
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
                .child(render_plan_action_strip(
                    props.entity.clone(),
                    &action_artifact,
                    props.can_implement_plan,
                    props.show_implement_choice,
                    props.recommend_fresh_context,
                ))
                .child(
                    div()
                        .id("review-plan-body")
                        .flex_1()
                        .min_h(px(0.0))
                        .overflow_y_scroll()
                        .p(Tokens::spacing_3())
                        .child(todo_list::plan_artifact(body_artifact).into_any_element()),
                ),
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
) -> impl IntoElement {
    let can_start = can_implement && artifact.execution_state.can_start();
    let state_label = artifact.execution_state.label();
    let show_entity = entity.clone();
    let fresh_entity = entity.clone();
    let continue_entity = entity.clone();

    div()
        .id("plan-action-strip")
        .flex_shrink_0()
        .px(Tokens::spacing_3())
        .py(Tokens::spacing_2())
        .border_b_1()
        .border_color(Tokens::border_subtle())
        .bg(Tokens::panel_bg())
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_2())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(Tokens::spacing_3())
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
                                        .child("Plan review"),
                                )
                                .child(
                                    div()
                                        .text_size(Tokens::text_xs())
                                        .text_color(Tokens::text_secondary())
                                        .child(format!("Status · {state_label}")),
                                ),
                        )
                        .when(can_start && !show_choice, |el| {
                            el.child(
                                btn_approve("show-implement-plan-choice", "Implement Plan")
                                    .on_click(move |_, _, app: &mut gpui::App| {
                                        show_entity.update(app, |view, cx| {
                                            view.show_plan_implementation_choice(cx);
                                        });
                                    }),
                            )
                        })
                        .when(!can_start, |el| {
                            el.child(
                                div()
                                    .text_size(Tokens::text_xs())
                                    .text_color(Tokens::text_faint())
                                    .child(if can_implement {
                                        "No implementation action available"
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
                            .flex_col()
                            .gap(Tokens::spacing_2())
                            .child(
                                div()
                                    .text_size(Tokens::text_xs())
                                    .text_color(Tokens::text_faint())
                                    .child(if recommend_fresh_context {
                                        "Recommended: start fresh because the current context is getting large."
                                    } else {
                                        "Recommended: continue here because the current context is still useful."
                                    }),
                            )
                            .child(
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
                            ),
                    )
                }),
        )
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
        .child("Select a file to review changes")
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
    _plan_available: bool,
    total_add: usize,
    total_del: usize,
    applied: bool,
    show_apply: bool,
    show_close_button: bool,
) -> impl IntoElement {
    let apply_entity = entity.clone();
    let changes_selected = active_tab == ReviewPanelTab::Changes;

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
                    .when(applied, |el| {
                        el.child(
                            div()
                                .text_size(Tokens::text_xs())
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Tokens::success())
                                .child("Applied"),
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

#[derive(Clone, Copy)]
enum DiffFileHeaderMode {
    SingleFileCompact,
    MultiFileTab,
}

fn render_file_tabs(
    files: &[DiffFile],
    selected: usize,
    entity: Entity<AgentWindow>,
) -> impl IntoElement {
    let entity = entity.clone();
    let tabs: Vec<TabItem> = files
        .iter()
        .enumerate()
        .map(|(index, file)| {
            let file_name = file
                .path
                .rsplit('/')
                .next()
                .unwrap_or(&file.path)
                .to_string();
            TabItem {
                id: index as u64,
                label: file_name,
                icon: Some(icons::FILE_CODE),
            }
        })
        .collect();

    div()
        .flex()
        .flex_nowrap()
        .overflow_hidden()
        .gap(Tokens::spacing_1())
        .px(Tokens::spacing_2())
        .border_b_1()
        .border_color(Tokens::border_subtle())
        .child(tab_bar(TabBarProps {
            id: "diff-file",
            tabs,
            selected_id: selected as u64,
            on_select: Some(Rc::new(move |index, app| {
                entity.update(app, |view, cx| {
                    view.select_diff_file(index as usize, cx);
                });
            })),
            on_close: None,
            on_add: None,
            on_reorder: None,
            drag_kind: None,
        }))
}

fn render_file_diff(file: DiffFile, header_mode: DiffFileHeaderMode) -> impl IntoElement {
    let path = Rc::new(file.path);
    let rows = Rc::new(file.flat_rows);
    let row_count = rows.len();
    let list_rows = rows.clone();
    let added = file.added;
    let removed = file.removed;

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.0))
        .size_full()
        .when(
            matches!(header_mode, DiffFileHeaderMode::SingleFileCompact),
            |el| el.child(render_single_file_header(path.as_ref(), added, removed)),
        )
        .child(
            uniform_list(
                element_key("diff-rows", path.as_str()),
                row_count,
                move |range, _window, _cx| {
                    range
                        .map(|i| render_diff_row(&list_rows[i], i).into_any_element())
                        .collect()
                },
            )
            .size_full(),
        )
}

fn render_single_file_header(path: &str, added: usize, removed: usize) -> impl IntoElement {
    div()
        .bg(Tokens::panel_bg())
        .h(px(Tokens::DIFF_PATH_HEIGHT))
        .flex_shrink_0()
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
                .min_w(px(0.0))
                .overflow_hidden()
                .child(
                    div()
                        .text_size(Tokens::text_code())
                        .font_family("monospace")
                        .text_color(Tokens::text_secondary())
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(path.to_string()),
                )
                .child(
                    div()
                        .text_size(Tokens::text_code())
                        .text_color(Tokens::success())
                        .flex_shrink_0()
                        .child(format!("+{added}")),
                )
                .child(
                    div()
                        .text_size(Tokens::text_code())
                        .text_color(Tokens::danger())
                        .flex_shrink_0()
                        .child(format!("−{removed}")),
                ),
        )
        .child(
            div().flex().items_center().gap(Tokens::spacing_2()).child(
                btn_ghost_label("open-file", "Open")
                    .compact()
                    .h(px(Tokens::ROW_HEIGHT_XS)),
            ),
        )
}

fn render_diff_row(row: &DiffRow, index: usize) -> impl IntoElement {
    match row {
        DiffRow::Collapsed { count } => return render_collapsed_row(*count).into_any_element(),
        DiffRow::HunkHeader { label } => return render_hunk_row(label).into_any_element(),
        _ => {}
    }

    let kind = row.kind();
    let (bg, indicator, default_fg) = row_style(&kind);
    let (line_num, content) = match row {
        DiffRow::Context { new_line, text, .. } => (format!("{new_line}"), text.as_str()),
        DiffRow::Added { new_line, text } => (format!("{new_line}"), text.as_str()),
        DiffRow::Removed { old_line, text } => (format!("{old_line}"), text.as_str()),
        _ => (String::new(), ""),
    };

    div()
        .id(element_key("diff-row", &index.to_string()))
        .w_full()
        .h(px(Tokens::DIFF_LINE_HEIGHT))
        .bg(bg)
        .flex()
        .items_center()
        .font_family("monospace")
        .overflow_x_scroll()
        .child(
            div()
                .w(px(2.0))
                .h_full()
                .flex_shrink_0()
                .when(indicator.is_some(), |el| el.bg(indicator.unwrap())),
        )
        .child(
            div()
                .w(px(Tokens::DIFF_GUTTER_WIDTH))
                .h_full()
                .flex()
                .items_center()
                .justify_end()
                .pr(Tokens::spacing_2())
                .flex_shrink_0()
                .text_size(Tokens::text_code())
                .text_color(Tokens::diff_line_number())
                .child(line_num),
        )
        .child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .pl(Tokens::spacing_1())
                .pr(Tokens::spacing_4())
                .text_size(Tokens::text_code())
                .whitespace_nowrap()
                .text_color(default_fg)
                .child(content.to_string()),
        )
        .into_any_element()
}

fn render_collapsed_row(count: usize) -> impl IntoElement {
    div()
        .w_full()
        .h(px(Tokens::DIFF_LINE_HEIGHT))
        .flex()
        .items_center()
        .gap(Tokens::spacing_2())
        .px(Tokens::spacing_3())
        .text_size(Tokens::text_code())
        .text_color(Tokens::text_tertiary())
        .child(
            Icon::new(icons::CHEVRON_RIGHT)
                .size(px(10.0))
                .text_color(Tokens::text_faint()),
        )
        .child(format!("{count} unchanged lines"))
}

fn render_hunk_row(label: &str) -> impl IntoElement {
    div()
        .w_full()
        .h(px(Tokens::DIFF_LINE_HEIGHT))
        .border_t_1()
        .border_color(Tokens::border_subtle())
        .flex()
        .items_center()
        .px(Tokens::spacing_3())
        .font_family("monospace")
        .text_size(Tokens::text_xs())
        .text_color(Tokens::text_faint())
        .child(label.to_string())
}

fn row_style(kind: &DiffRowKind) -> (Hsla, Option<Hsla>, Hsla) {
    let code_fg = Tokens::diff_code_normal();
    match kind {
        DiffRowKind::Add => (
            Tokens::diff_add_bg(),
            Some(Tokens::diff_add_indicator()),
            code_fg,
        ),
        DiffRowKind::Remove => (
            Tokens::diff_del_bg(),
            Some(Tokens::diff_del_indicator()),
            code_fg,
        ),
        DiffRowKind::Context => (Tokens::diff_bg(), None, code_fg),
        _ => (Tokens::diff_bg(), None, code_fg),
    }
}

/// Parse a unified diff string into diff panel files.
pub fn parse_unified_diff(unified_diff: &str) -> Vec<DiffFile> {
    let mut files = Vec::new();
    let mut current_path: Option<String> = None;
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
            if !path.is_empty() && path != "/dev/null" {
                current_path = Some(path);
            }
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
