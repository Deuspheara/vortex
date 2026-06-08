//! Thread row rendering

use super::*;
use crate::features::shell::state::ActivityGroupPos;
use crate::shared::components::collapsible_row::timeline_body;

impl ThreadView {
    pub(crate) fn render_visible(
        &mut self,
        range: std::ops::Range<usize>,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let start = std::time::Instant::now();
        let row_count = range.len() as u64;
        let approval_active = self.approval_active;
        let animate = self.animate();
        let agent = self.agent.clone();
        let sizes = Rc::clone(&self.row_sizes);

        let rows = range
            .map(|row_ix| {
                let row_h = sizes
                    .get(row_ix)
                    .map(|s| s.height)
                    .unwrap_or(px(Tokens::TOOL_ROW_HEIGHT));
                match self.manifest.get(row_ix).copied() {
                    Some(row_ref) => {
                        let prev = self.manifest.get(row_ix.wrapping_sub(1)).copied();
                        let gap = row_top_gap(row_ref, prev, &self.items);
                        self.render_row(row_ref, row_h, gap, approval_active, animate, &agent, cx)
                    }
                    None => div().w_full().h(row_h).into_any_element(),
                }
            })
            .collect();
        crate::shared::render_profile::record(
            "ThreadView::render_visible",
            start.elapsed(),
            row_count,
        );
        rows
    }

    pub(crate) fn render_row(
        &mut self,
        row_ref: RowRef,
        row_h: gpui::Pixels,
        top_gap: f32,
        approval_active: bool,
        animate: bool,
        agent: &Entity<AgentWindow>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if matches!(row_ref, RowRef::EndSpacer) {
            return div()
                .id("thread-end-spacer")
                .w_full()
                .h(row_h)
                .into_any_element();
        }

        if let RowRef::TimelineSection { phase } = row_ref {
            return div()
                .id(element_key("timeline-section-row", &format!("{phase}")))
                .w_full()
                .when(top_gap > 0.0, |el| el.pt(px(top_gap)))
                .child(timeline_section_header(phase_from_u8(phase)))
                .into_any_element();
        }

        let item_ix = row_ref.item_ix().expect("content row") as usize;
        let item = self.items.get(item_ix);
        let depth = item.and_then(item_depth).unwrap_or(0);
        let row_id = stable_row_id(row_ref, item);
        let group_pos = activity_group_pos(item_ix, &self.items);

        let content = match row_ref {
            RowRef::TimelineSection { .. } => div().into_any_element(),
            RowRef::UserMessage { .. } => {
                let Some(ThreadItem::UserMessage {
                    text,
                    attachments,
                    id,
                    expanded,
                }) = item
                else {
                    return div().into_any_element();
                };
                let is_initial = crate::features::shell::state::first_user_message_ix(&self.items)
                    == Some(item_ix);
                let truncatable =
                    is_initial && crate::features::shell::state::user_message_truncatable(text);
                let collapsed = truncatable && !expanded;
                let toggle_id = id.clone();
                let agent_toggle = agent.clone();
                user_message(
                    text,
                    attachments,
                    id,
                    collapsed,
                    truncatable,
                    move |app: &mut gpui::App| {
                        agent_toggle.update(app, |view, cx| {
                            view.toggle_thread_item(&toggle_id, cx);
                        });
                    },
                )
                .into_any_element()
            }
            RowRef::SubagentHeader { .. } => {
                let Some(ThreadItem::SubagentRun {
                    id,
                    task,
                    model: _,
                    summary: _,
                    status,
                    ..
                }) = item
                else {
                    return div().into_any_element();
                };
                let toggle_id = id.clone();
                let agent_select = agent.clone();
                let _profile =
                    crate::shared::render_profile::span("ThreadView::subagent_header_vm");
                let (activity_summary, is_selected) = agent.read_with(cx, |window, _| {
                    let projection = window.subagent_transcripts.get(id);
                    let summary = projection
                        .map(|projection| projection.activity_summary())
                        .unwrap_or_else(|| "No child activity yet".to_string());
                    let selected = window
                        .inspector_tabs
                        .active()
                        .is_some_and(|tab| matches!(&tab.kind, crate::features::shell::state::InspectorTabKind::Subagent(active_id) if active_id == id));
                    (summary, selected)
                });
                render_subagent_summary_row(
                    id,
                    task.clone(),
                    status,
                    &activity_summary,
                    is_selected,
                    group_pos,
                    animate,
                    move |app: &mut gpui::App| {
                        agent_select.update(app, |view, cx| {
                            view.select_subagent_tab(&toggle_id, cx);
                        });
                    },
                )
                .into_any_element()
            }
            RowRef::SubagentBody { .. } => {
                let Some(ThreadItem::SubagentRun { id, summary, .. }) = item else {
                    return div().into_any_element();
                };
                let _profile = crate::shared::render_profile::span("ThreadView::subagent_body_vm");
                let (activity_summary, last_event) = agent.read_with(cx, |window, _| {
                    let projection = window.subagent_transcripts.get(id);
                    (
                        projection
                            .map(|projection| projection.activity_summary())
                            .unwrap_or_else(|| "No child activity yet".to_string()),
                        projection.and_then(|projection| projection.last_event_label.clone()),
                    )
                });
                render_subagent_detail_row(
                    id,
                    summary,
                    &activity_summary,
                    last_event.as_deref(),
                    group_pos,
                )
                .into_any_element()
            }
            RowRef::ReasoningHeader { .. } => {
                let Some(ThreadItem::ReasoningStep {
                    id,
                    title,
                    summary,
                    expanded,
                    status,
                    ..
                }) = item
                else {
                    return div().into_any_element();
                };
                let toggle_id = id.clone();
                let agent_toggle = agent.clone();
                render_reasoning_header_row(
                    id,
                    title,
                    summary,
                    *expanded,
                    status,
                    animate,
                    group_pos,
                    move |app: &mut gpui::App| {
                        agent_toggle.update(app, |view, cx| {
                            view.toggle_thread_item(&toggle_id, cx);
                        });
                    },
                )
                .into_any_element()
            }
            RowRef::ReasoningBody { .. } => {
                let Some(ThreadItem::ReasoningStep {
                    id,
                    summary,
                    status,
                    ..
                }) = item
                else {
                    return div().into_any_element();
                };
                let streaming =
                    matches!(status, crate::features::shell::state::AgentStatus::Thinking);
                activity_group_wrap(
                    timeline_body(
                        element_key("reasoning-body-wrap", id),
                        div()
                            .id(element_key("reasoning-body", id))
                            .w_full()
                            .pr(Tokens::spacing_1())
                            .max_h(px(520.0))
                            .overflow_y_scroll()
                            .child(markdown_preview_thread_streaming(summary, streaming)),
                    ),
                    group_pos,
                )
                .into_any_element()
            }
            RowRef::ReasoningPreviewLine { line_ix, .. } => {
                let Some(item) = item else {
                    return div().into_any_element();
                };
                let text = reasoning_preview_line_text(item, line_ix as usize)
                    .unwrap_or_else(|| Arc::from(""));
                activity_group_wrap(
                    render_reasoning_preview_line_row(
                        &format!("reason-{item_ix}-preview-{line_ix}"),
                        &text,
                    ),
                    group_pos,
                )
                .into_any_element()
            }
            RowRef::ToolHeader { .. } => {
                let Some(ThreadItem::ToolCall {
                    id,
                    tool_name,
                    command,
                    output,
                    expanded,
                    status,
                    ..
                }) = item
                else {
                    return div().into_any_element();
                };
                let preview = if matches!(
                    status,
                    crate::features::shell::state::AgentStatus::WaitingApproval
                ) {
                    output.as_deref().or(command.as_deref())
                } else {
                    command.as_deref()
                };
                let running = matches!(
                    status,
                    crate::features::shell::state::AgentStatus::RunningTool
                );
                let _profile = crate::shared::render_profile::span("ThreadView::tool_header_vm");
                let (change_counts, display_label) = agent.read_with(cx, |window, _| {
                    (
                        window.live_edit_change_counts(tool_name, running),
                        window.tool_row_label(tool_name, preview, running),
                    )
                });
                let toggle_id = id.clone();
                let agent_toggle = agent.clone();
                let agent_select = agent.clone();
                render_tool_header_row(
                    id,
                    &display_label,
                    preview,
                    *expanded,
                    status,
                    animate,
                    group_pos,
                    change_counts,
                    move |app: &mut gpui::App| {
                        agent_select.update(app, |view, cx| {
                            view.select_tool_artifact(&toggle_id, cx);
                        });
                        agent_toggle.update(app, |view, cx| {
                            view.toggle_thread_item(&toggle_id, cx);
                        });
                    },
                )
                .into_any_element()
            }
            RowRef::ToolOutputLine { line_ix, .. } => {
                let (id, out) = {
                    let Some(ThreadItem::ToolCall {
                        id,
                        output: Some(out),
                        expanded: true,
                        ..
                    }) = item
                    else {
                        return div().into_any_element();
                    };
                    (id.clone(), out.clone())
                };
                let preview = self.cached_tool_output_preview(&id, &out);
                let text = preview
                    .lines
                    .get(line_ix as usize)
                    .cloned()
                    .unwrap_or_else(|| Arc::from(""));
                activity_group_wrap(
                    render_tool_output_line_row(&format!("tool-{item_ix}-line-{line_ix}"), &text),
                    group_pos,
                )
                .into_any_element()
            }
            RowRef::ToolOutputTruncated { .. } => {
                let (id, out) = {
                    let Some(ThreadItem::ToolCall {
                        id,
                        output: Some(out),
                        ..
                    }) = item
                    else {
                        return div().into_any_element();
                    };
                    (id.clone(), out.clone())
                };
                let preview = self.cached_tool_output_preview(&id, &out);
                activity_group_wrap(
                    render_tool_output_truncated_row(&id, preview.total_lines, preview.full),
                    group_pos,
                )
                .into_any_element()
            }
            RowRef::DiffHeader { .. } => {
                let Some(ThreadItem::DiffSummary {
                    id,
                    files_changed,
                    additions,
                    deletions,
                    ..
                }) = item
                else {
                    return div().into_any_element();
                };
                let toggle_id = id.clone();
                let agent_toggle = agent.clone();
                let agent_review = agent.clone();
                render_diff_header_row(
                    id,
                    *files_changed,
                    *additions,
                    *deletions,
                    group_pos,
                    move |app: &mut gpui::App| {
                        agent_toggle.update(app, |view, cx| {
                            view.toggle_thread_item(&toggle_id, cx);
                        });
                    },
                    move |app: &mut gpui::App| {
                        agent_review.update(app, |view, cx| {
                            view.open_diff_panel(cx);
                        });
                    },
                )
                .into_any_element()
            }
            RowRef::DiffFileLine { file_ix, .. } => {
                let Some(ThreadItem::DiffSummary { id, files, .. }) = item else {
                    return div().into_any_element();
                };
                let Some(file) = files.get(file_ix as usize) else {
                    return div().into_any_element();
                };
                activity_group_wrap(
                    render_diff_file_line_row(
                        &format!("{id}-file-{file_ix}"),
                        &file.path,
                        file.added,
                        file.removed,
                    ),
                    group_pos,
                )
                .into_any_element()
            }
            RowRef::AssistantMessage { .. } => self.render_assistant(item_ix, cx),
            RowRef::Approval { .. } => {
                let Some(ThreadItem::ApprovalRequest {
                    title,
                    risk,
                    resolved,
                    ..
                }) = item
                else {
                    return div().into_any_element();
                };
                thread_approval_row(title, risk, *resolved, approval_active).into_any_element()
            }
            RowRef::RunError { .. } => {
                let Some(ThreadItem::RunError {
                    message, retryable, ..
                }) = item
                else {
                    return div().into_any_element();
                };
                let title = message.lines().next().unwrap_or(message).to_string();
                let msg = message.clone();
                let retry = *retryable;
                let settings_entity = agent.clone();
                error_card(ErrorCardProps {
                    title,
                    message: msg,
                    retryable: retry,
                    on_open_settings: Some(Box::new(move |app| {
                        settings_entity.update(app, |view, cx| view.open_settings(cx));
                    })),
                    on_retry: None,
                })
                .into_any_element()
            }
            RowRef::ChoiceRequest { .. } => {
                let Some(ThreadItem::ChoiceRequest {
                    id,
                    prompt,
                    options,
                    meta,
                    selected,
                    resolved,
                }) = item
                else {
                    return div().into_any_element();
                };
                let choice_id = id.clone();
                let entity = agent.clone();
                let cancel_entity = agent.clone();
                choice_card(
                    id,
                    prompt,
                    options,
                    meta,
                    selected.as_deref(),
                    *resolved,
                    move |option_id, app| {
                        entity.update(app, |view, cx| {
                            view.submit_choice(&choice_id, &option_id, cx);
                        });
                    },
                    move |app| {
                        cancel_entity.update(app, |view, cx| {
                            view.cancel_active_run(cx);
                        });
                    },
                )
                .into_any_element()
            }
            RowRef::PlanStatus { .. } => {
                let Some(ThreadItem::PlanStatus {
                    id,
                    state,
                    summary,
                    counts,
                    source_conversation_id,
                }) = item
                else {
                    return div().into_any_element();
                };
                let agent_open = agent.clone();
                render_plan_status_row(
                    id,
                    state,
                    summary,
                    counts.summary(),
                    source_conversation_id.as_ref().map(|id| id.0.as_str()),
                    group_pos,
                    move |app: &mut gpui::App| {
                        agent_open.update(app, |view, cx| {
                            view.select_inspector_view(
                                crate::features::shell::state::InspectorView::Plan,
                                cx,
                            );
                        });
                    },
                )
                .into_any_element()
            }
            RowRef::ContextTraceHeader { .. } => {
                let Some(ThreadItem::ContextTrace {
                    id,
                    entries,
                    expanded,
                }) = item
                else {
                    return div().into_any_element();
                };
                let summary = context_trace_counts_summary(entries);
                let toggle_id = id.clone();
                let agent_toggle = agent.clone();
                activity_header_row(
                    "context-trace-row",
                    "context-trace-header",
                    id,
                    "Context".to_string(),
                    Some(summary),
                    false,
                    animate && *expanded,
                    group_pos,
                    div().into_any_element(),
                    move |app: &mut gpui::App| {
                        agent_toggle.update(app, |view, cx| {
                            view.toggle_thread_item(&toggle_id, cx);
                        });
                    },
                )
                .into_any_element()
            }
            RowRef::ContextTraceEntryLine { entry_ix, .. } => {
                let Some(ThreadItem::ContextTrace { id, entries, .. }) = item else {
                    return div().into_any_element();
                };
                let Some(entry) = entries.get(entry_ix as usize) else {
                    return div().into_any_element();
                };
                activity_group_wrap(
                    activity_output_line_row(
                        &format!("{id}-ctx-{entry_ix}"),
                        &context_trace_entry_line(entry),
                        false,
                    ),
                    group_pos,
                )
                .into_any_element()
            }
            RowRef::EndSpacer => unreachable!("handled above"),
        };

        div()
            .id(element_key("thread-row-shell", &row_id))
            .w_full()
            .h(row_h)
            .when(depth > 0, |el| el.pl(Tokens::tree_indent(depth as u32)))
            .when(top_gap > 0.0, |el| el.pt(px(top_gap)))
            .overflow_hidden()
            .flex()
            .flex_col()
            .justify_start()
            .child(content)
            .into_any_element()
    }

    pub(crate) fn render_assistant(
        &mut self,
        item_ix: usize,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let _profile = crate::shared::render_profile::span("ThreadView::render_assistant");
        let Some(ThreadItem::AssistantMessage {
            id,
            markdown,
            streaming,
            ..
        }) = self.items.get(item_ix)
        else {
            return div().into_any_element();
        };
        if *streaming {
            let id = id.clone();
            let jump_to_latest = self.user_scrolled_up.then(|| {
                let entity = cx.entity();
                let button_id = id.clone();
                div()
                    .id(element_key("stream-scroll-pill", &button_id))
                    .w_full()
                    .pt(Tokens::spacing_1())
                    .flex()
                    .justify_start()
                    .child(
                        div()
                            .id(element_key("stream-scroll-pill-btn", &button_id))
                            .px(Tokens::spacing_2())
                            .py(Tokens::spacing_0p5())
                            .rounded(Tokens::radius_sm())
                            .bg(Tokens::surface_hover())
                            .text_size(Tokens::text_xs())
                            .text_color(Tokens::text_tertiary())
                            .cursor_pointer()
                            .hover(|s| s.bg(Tokens::surface_active()))
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |view, cx| {
                                    view.stick_to_bottom = true;
                                    view.user_scrolled_up = false;
                                    view.pending_scroll_bottom = true;
                                    cx.notify();
                                });
                            })
                            .child("Jump to latest ↓"),
                    )
                    .into_any_element()
            });

            let sealed = self.sealed_blocks.get(&id).cloned().unwrap_or_default();
            let show_cursor = markdown.len() > 2;
            let actions_projection = self.assistant_actions;
            let actions = assistant_action_row(
                &id,
                true,
                cx.entity().clone(),
                self.agent.clone(),
                actions_projection.can_retry,
                actions_projection.can_open_diff,
                actions_projection.can_approve,
            );
            return div()
                .id(element_key("assistant-segment", &id))
                .w_full()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_1())
                .child(assistant_result_label(&id))
                .child(streaming_assistant_body(
                    &id,
                    markdown,
                    &sealed,
                    show_cursor,
                ))
                .when_some(jump_to_latest, |el, button| el.child(button))
                .child(actions)
                .into_any_element();
        }

        let id = id.clone();
        let markdown = markdown.clone();
        self.sealed_blocks.remove(&id);

        let blocks = self.cached_markdown_blocks(&id, &markdown, false);
        let actions_projection = self.assistant_actions;
        let actions = assistant_action_row(
            &id,
            false,
            cx.entity().clone(),
            self.agent.clone(),
            actions_projection.can_retry,
            actions_projection.can_open_diff,
            actions_projection.can_approve,
        );
        div()
            .id(element_key("assistant-segment", &id))
            .w_full()
            .flex()
            .flex_col()
            .gap(Tokens::spacing_1())
            .text_size(Tokens::text_md())
            .line_height(Tokens::text_md_leading())
            .child(assistant_result_label(&id))
            .child(markdown_preview_blocks_thread_shared(blocks, false))
            .child(actions)
            .into_any_element()
    }
}

fn assistant_result_label(item_id: &str) -> impl IntoElement {
    div()
        .id(element_key("assistant-result-label", item_id))
        .h(Tokens::text_sm_leading_compact())
        .flex()
        .items_center()
        .text_size(Tokens::text_xs())
        .line_height(Tokens::text_sm_leading_compact())
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(Tokens::text_tertiary())
        .child("Result")
}

fn assistant_action_row(
    item_id: &str,
    streaming: bool,
    thread: Entity<ThreadView>,
    agent: Entity<AgentWindow>,
    can_retry: bool,
    can_open_diff: bool,
    can_approve: bool,
) -> impl IntoElement {
    use crate::shared::components::buttons::btn_icon_sm;
    use crate::tokens::icons;

    div()
        .id(element_key("assistant-actions", item_id))
        .w_full()
        .pt(Tokens::spacing_0p5())
        .flex()
        .items_center()
        .gap(Tokens::spacing_1())
        .opacity(if streaming { 0.44 } else { 0.24 })
        .hover(|s| s.opacity(0.64))
        .child({
            let copy_thread = thread.clone();
            let copy_item_id = item_id.to_string();
            btn_icon_sm(element_key("assistant-copy", item_id), icons::COPY)
                .tooltip("Copy reply")
                .on_click(move |_, _, app: &mut gpui::App| {
                    copy_thread.update(app, |view, cx| {
                        if let Some(markdown) = view.items.iter().find_map(|item| {
                            if let ThreadItem::AssistantMessage { id, markdown, .. } = item {
                                (id == &copy_item_id).then(|| markdown.clone())
                            } else {
                                None
                            }
                        }) {
                            cx.write_to_clipboard(markdown.into());
                        }
                    });
                })
        })
        .child(
            btn_icon_sm(element_key("assistant-helpful", item_id), icons::CHECK).tooltip("Helpful"),
        )
        .child(
            btn_icon_sm(element_key("assistant-unhelpful", item_id), icons::X_MARK)
                .tooltip("Needs work"),
        )
        .when(can_retry, |el| {
            let retry_agent = agent.clone();
            el.child(
                btn_icon_sm(element_key("assistant-retry", item_id), icons::ARROW_UP)
                    .tooltip("Retry last turn")
                    .when(!streaming, |button| {
                        button.on_click(move |_, _, app: &mut gpui::App| {
                            retry_agent.update(app, |view, cx| {
                                view.retry_last_user_turn(cx);
                            });
                        })
                    }),
            )
        })
        .when(can_open_diff, |el| {
            let diff_agent = agent.clone();
            el.child(
                btn_icon_sm(
                    element_key("assistant-open-diff", item_id),
                    icons::GIT_COMPARE,
                )
                .tooltip("Open changes")
                .on_click(move |_, _, app: &mut gpui::App| {
                    diff_agent.update(app, |view, cx| {
                        view.open_diff_panel(cx);
                    });
                }),
            )
        })
        .when(can_approve, |el| {
            el.child(
                div()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_tertiary())
                    .child("Approval pending"),
            )
        })
}

pub(crate) fn render_empty_thread_state() -> impl IntoElement {
    div()
        .id("thread-empty-state")
        .flex_1()
        .w_full()
        .max_w(px(Tokens::THREAD_MAX_WIDTH))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .max_w(px(Tokens::THREAD_EMPTY_COPY_WIDTH))
                .flex()
                .flex_col()
                .items_start()
                .gap(Tokens::spacing_2())
                .child(
                    div()
                        .text_size(Tokens::text_xl())
                        .line_height(Tokens::text_xl_leading())
                        .text_color(Tokens::text_primary())
                        .child("Start a thread"),
                )
                .child(
                    div()
                        .text_size(Tokens::text_md())
                        .line_height(Tokens::text_md_leading())
                        .text_color(Tokens::text_tertiary())
                        .child(
                            "Ask for a code change, inspect a file, or describe the task. The thread stays continuous once the first turn starts.",
                        ),
                )
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .line_height(Tokens::text_sm_leading())
                        .text_color(Tokens::text_faint())
                        .child("Tool runs, reasoning, and diff summaries will appear inline beneath the reply."),
                ),
        )
}

fn render_subagent_summary_row(
    item_id: &str,
    task: String,
    status: &crate::features::shell::state::AgentStatus,
    activity_summary: &str,
    is_selected: bool,
    group_pos: Option<ActivityGroupPos>,
    animate: bool,
    on_open: impl Fn(&mut gpui::App) + 'static,
) -> impl IntoElement {
    use crate::shared::components::collapsible_row::{activity_group_wrap, timeline_row};
    use crate::tokens::{activity_action_line_with_loading, icons};
    use gpui_component::Icon;

    let on_open = Rc::new(on_open);
    let on_open_button = on_open.clone();
    let on_open_row = on_open.clone();
    let status_label = status_label(status);
    let running = matches!(
        status,
        crate::features::shell::state::AgentStatus::RunningTool
            | crate::features::shell::state::AgentStatus::Thinking
    );
    let title = format!("Subagent {}", status_label.to_ascii_lowercase());
    let detail = if activity_summary.trim().is_empty() {
        Some(task)
    } else {
        Some(activity_summary.to_string())
    };
    let open_button = div()
        .id(element_key("subagent-open-tab", item_id))
        .h(px(Tokens::ROW_HEIGHT_XS))
        .px(Tokens::spacing_1())
        .flex()
        .items_center()
        .justify_center()
        .rounded(Tokens::radius_xs())
        .cursor_pointer()
        .text_color(Tokens::text_tertiary())
        .hover(|s| s.text_color(Tokens::text_primary()))
        .on_click(move |_, _, app| on_open_button(app))
        .child(
            Icon::new(icons::EXTERNAL_LINK)
                .size(px(13.0))
                .text_color(Tokens::text_tertiary()),
        );

    activity_group_wrap(
        div()
            .id(element_key("subagent-shell", item_id))
            .w_full()
            .when(is_selected, |el| {
                el.bg(Tokens::surface_hover().blend(Tokens::accent().opacity(0.05)))
                    .rounded(Tokens::radius_xs())
            })
            .child(timeline_row(
                element_key("subagent-header", item_id),
                div()
                    .min_w(px(0.0))
                    .child(
                        div().min_w(px(0.0)).overflow_hidden().child(
                            activity_action_line_with_loading(
                                &title,
                                detail.as_deref(),
                                running,
                                false,
                                animate,
                                item_id,
                            )
                            .into_any_element(),
                        ),
                    )
                    .into_any_element(),
                open_button.into_any_element(),
                move |_, _, app: &mut gpui::App| on_open_row(app),
            )),
        group_pos,
    )
}

fn render_subagent_detail_row(
    item_id: &str,
    summary: &str,
    activity_summary: &str,
    last_event: Option<&str>,
    group_pos: Option<ActivityGroupPos>,
) -> impl IntoElement {
    let summary_excerpt = summary
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(2)
        .collect::<Vec<_>>()
        .join(" ");
    activity_group_wrap(
        timeline_body(
            element_key("subagent-body-wrap", item_id),
            div()
                .id(element_key("subagent-body", item_id))
                .w_full()
                .pr(Tokens::spacing_1())
                .flex()
                .flex_col()
                .gap(Tokens::spacing_1())
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .line_height(Tokens::text_sm_leading_compact())
                        .text_color(Tokens::activity_detail_text())
                        .child(if summary_excerpt.trim().is_empty() {
                            "Investigating task in child run.".to_string()
                        } else {
                            summary_excerpt
                        }),
                )
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .line_height(Tokens::text_sm_leading_compact())
                        .text_color(Tokens::activity_meta_text())
                        .child(format!("Transcript · {activity_summary}")),
                )
                .when_some(last_event, |el, last| {
                    el.child(
                        div()
                            .text_size(Tokens::text_xs())
                            .line_height(Tokens::text_sm_leading_compact())
                            .text_color(Tokens::text_faint())
                            .child(format!("Last update · {last}")),
                    )
                }),
        ),
        group_pos,
    )
}

fn render_plan_status_row(
    item_id: &str,
    state: &crate::features::shell::state::PlanExecutionState,
    summary: &str,
    counts_summary: String,
    source_conversation_id: Option<&str>,
    group_pos: Option<ActivityGroupPos>,
    on_open: impl Fn(&mut gpui::App) + 'static,
) -> impl IntoElement {
    use crate::shared::components::collapsible_row::{activity_group_wrap, timeline_row};
    use crate::tokens::activity_action_line_with_loading;

    let meta = match source_conversation_id {
        Some(source) => format!("{counts_summary} · Source: {source}"),
        None => counts_summary,
    };

    activity_group_wrap(
        div()
            .id(element_key("plan-status-row", item_id))
            .child(timeline_row(
                element_key("plan-status-header", item_id),
                activity_action_line_with_loading(
                    &format!("Plan · {}", state.label()),
                    (!summary.trim().is_empty()).then_some(summary),
                    false,
                    false,
                    false,
                    item_id,
                )
                .into_any_element(),
                div()
                    .id(element_key("plan-status-meta", item_id))
                    .max_w(px(220.0))
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .truncate()
                    .text_size(Tokens::text_xs())
                    .text_color(Tokens::text_faint())
                    .hover(|s| s.text_color(Tokens::text_secondary()))
                    .child(meta)
                    .into_any_element(),
                move |_, _, app: &mut gpui::App| on_open(app),
            )),
        group_pos,
    )
}

fn status_label(status: &crate::features::shell::state::AgentStatus) -> &'static str {
    match status {
        crate::features::shell::state::AgentStatus::Thinking
        | crate::features::shell::state::AgentStatus::RunningTool => "Running",
        crate::features::shell::state::AgentStatus::WaitingApproval => "Waiting approval",
        crate::features::shell::state::AgentStatus::Completed => "Completed",
        crate::features::shell::state::AgentStatus::Failed => "Failed",
        crate::features::shell::state::AgentStatus::Idle => "Idle",
    }
}

pub(crate) fn stable_row_id(row_ref: RowRef, item: Option<&ThreadItem>) -> String {
    let item_id = item.map(ThreadItem::id).unwrap_or("end");
    match row_ref {
        RowRef::TimelineSection { phase } => format!("timeline-section-{phase}"),
        RowRef::UserMessage { .. } => format!("user-{item_id}"),
        RowRef::SubagentHeader { .. } => format!("subagent-{item_id}"),
        RowRef::SubagentBody { .. } => format!("subagent-body-{item_id}"),
        RowRef::ReasoningHeader { .. } => format!("reasoning-header-{item_id}"),
        RowRef::ReasoningPreviewLine { line_ix, .. } => {
            format!("reasoning-preview-{item_id}-{line_ix}")
        }
        RowRef::ReasoningBody { .. } => format!("reasoning-body-{item_id}"),
        RowRef::ToolHeader { .. } => format!("tool-header-{item_id}"),
        RowRef::ToolOutputLine { line_ix, .. } => format!("tool-line-{item_id}-{line_ix}"),
        RowRef::ToolOutputTruncated { .. } => format!("tool-truncated-{item_id}"),
        RowRef::DiffHeader { .. } => format!("diff-header-{item_id}"),
        RowRef::DiffFileLine { file_ix, .. } => format!("diff-file-{item_id}-{file_ix}"),
        RowRef::AssistantMessage { .. } => format!("assistant-{item_id}"),
        RowRef::Approval { .. } => format!("approval-{item_id}"),
        RowRef::RunError { .. } => format!("run-error-{item_id}"),
        RowRef::ChoiceRequest { .. } => format!("choice-{item_id}"),
        RowRef::PlanStatus { .. } => format!("plan-status-{item_id}"),
        RowRef::ContextTraceHeader { .. } => format!("context-trace-header-{item_id}"),
        RowRef::ContextTraceEntryLine { entry_ix, .. } => {
            format!("context-trace-entry-{item_id}-{entry_ix}")
        }
        RowRef::EndSpacer => "thread-end-spacer".to_string(),
    }
}

fn item_depth(item: &ThreadItem) -> Option<u8> {
    match item {
        ThreadItem::AssistantMessage { depth, .. }
        | ThreadItem::ReasoningStep { depth, .. }
        | ThreadItem::ToolCall { depth, .. }
        | ThreadItem::DiffSummary { depth, .. } => Some(*depth),
        ThreadItem::SubagentRun { .. } => Some(0),
        ThreadItem::PlanStatus { .. } => Some(0),
        _ => None,
    }
}

pub(crate) fn tail_signature(items: &[ThreadItem]) -> u64 {
    let mut sig = items.len() as u64;
    for item in items.iter().rev() {
        match item {
            ThreadItem::AssistantMessage {
                markdown,
                streaming,
                ..
            } => {
                sig = sig.wrapping_mul(31).wrapping_add(markdown.len() as u64);
                sig = sig.wrapping_add(if *streaming { 1 } else { 0 });
                break;
            }
            ThreadItem::ToolCall { output, .. } => {
                sig = sig
                    .wrapping_mul(31)
                    .wrapping_add(output.as_ref().map(|s| s.len()).unwrap_or(0) as u64);
                break;
            }
            ThreadItem::ReasoningStep { summary, .. } => {
                sig = sig.wrapping_mul(31).wrapping_add(summary.len() as u64);
                break;
            }
            _ => {}
        }
    }
    sig
}
