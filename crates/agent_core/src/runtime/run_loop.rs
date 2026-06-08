use super::*;
use agent_models::extract_inline_tool_calls_with_tools;
use agent_protocol::ToolPhase;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

impl AgentRuntime {
    pub(crate) async fn run_loop(
        &self,
        run_id: &RunId,
        prompt: String,
        attachments: Vec<ContextAttachment>,
        cancel: CancellationToken,
    ) -> Result<(), AgentError> {
        let sink = self.sink();
        let mut loop_count = 0usize;

        // Workspace root + mode are fixed for the run; build the dynamic prompt prefix once.
        let (project_root_for_prompt, mode_for_prompt, model_for_selection) = {
            let runs = self.active_runs.lock().await;
            let run = runs.get(run_id);
            (
                run.map(|r| r.project_root.clone()),
                run.map(|r| r.mode.clone()),
                run.map(|r| r.model.clone()),
            )
        };

        let dynamic_prefix =
            project_root_for_prompt
                .zip(mode_for_prompt)
                .map(|(project_root, mode)| {
                    let tree = top_level_tree(&project_root);
                    let agents_md = std::fs::read_to_string(project_root.join("AGENTS.md")).ok();
                    // Avoid synchronously rebuilding the repo index before the first model turn.
                    // Large repos can spend multiple minutes scanning here, which blocks all visible
                    // progress after `RunStarted`. The richer repo-map path still exists behind the
                    // optional context-selection flow.
                    agent_context::dynamic_context_block(&agent_context::PromptContext {
                        workspace_root: &project_root.display().to_string(),
                        top_level_tree: &tree,
                        repo_map: None,
                        agents_md: agents_md.as_deref(),
                        mode,
                    })
                });

        let mut dynamic_prefix = dynamic_prefix;

        if super::context::context_selection_enabled()
            && should_run_context_selection(classify_task(&prompt), &prompt)
        {
            let (root, project_id, model, repo_index) = {
                let runs = self.active_runs.lock().await;
                let run = runs.get(run_id);
                (
                    run.map(|r| r.project_root.clone()),
                    run.map(|r| r.project_id.clone()),
                    model_for_selection,
                    run.and_then(|r| r.repo_index.clone()),
                )
            };
            if let (Some(root), Some(project_id), Some(model)) = (root, project_id, model) {
                let maybe_index =
                    repo_index.or_else(|| super::context::open_repo_index(&root, &project_id));
                if let Some(index) = maybe_index {
                    if let Some(run) = self.active_runs.lock().await.get_mut(run_id) {
                        run.repo_index = Some(index.clone());
                    }
                    let service = super::context::build_context_service(index, &root);
                    if let Ok((opened, _trace)) = super::context::run_context_selection(
                        self.provider.as_ref(),
                        &sink,
                        run_id,
                        model,
                        &prompt,
                        dynamic_prefix.clone(),
                        &service,
                        &cancel,
                    )
                    .await
                    {
                        if let Some(block) = opened {
                            dynamic_prefix = Some(match dynamic_prefix {
                                Some(prefix) => format!("{block}\n{prefix}"),
                                None => block,
                            });
                        }
                    }
                }
            }
        }

        loop {
            loop_count += 1;
            if loop_count > self.limits.max_model_loops {
                return self.fail_run(run_id, AgentError::LoopLimitExceeded).await;
            }
            cancel.check_cancelled()?;
            self.check_runtime_limit(run_id).await?;

            let (
                model,
                message_history,
                model_context_state,
                task_class,
                tool_pack,
                prompt_tools,
                project_root,
                project_id,
                session_id,
                mode,
                repo_index,
                previous_response_id,
                first_turn,
            ) = {
                let runs = self.active_runs.lock().await;
                let run = runs
                    .get(run_id)
                    .ok_or(AgentError::Other("run not found".into()))?;
                let exec_specs = self.tools.registry.tool_specs();
                let phase = infer_tool_phase(&run.message_history, run.task_class, &run.mode);
                let prompt_tool_specs = agent_tools::task_visible_prompt_tool_specs(
                    self.tools.registry.prompt_tool_specs(),
                    &exec_specs,
                    &run.mode,
                    run.depth,
                    is_git_repo(&run.project_root),
                    run.tool_pack,
                    phase,
                );
                (
                    run.model.clone(),
                    run.message_history.clone(),
                    run.model_context_state.clone(),
                    run.task_class,
                    run.tool_pack,
                    prompt_tool_specs,
                    run.project_root.clone(),
                    run.project_id.clone(),
                    run.session_id.clone(),
                    run.mode.clone(),
                    run.repo_index.clone(),
                    run.previous_response_id.clone(),
                    run.message_history.is_empty(),
                )
            };

            tracing::info!(
                run_id = %run_id.0,
                loop_count,
                history_len = message_history.len(),
                tools = prompt_tools.len(),
                "model loop iteration"
            );

            let repo_index = match repo_index {
                Some(index) => Some(index),
                None => {
                    let opened = super::context::open_repo_index(&project_root, &project_id);
                    if let Some(run) = self.active_runs.lock().await.get_mut(run_id) {
                        run.repo_index = opened.clone();
                    }
                    opened
                }
            };

            let (relevant_context, trace_entries) =
                super::context::deterministic_context_for_prompt(
                    repo_index.as_ref(),
                    &project_root,
                    &project_id,
                    &prompt,
                    task_class,
                    first_turn,
                );
            if !trace_entries.is_empty() {
                sink.emit(
                    run_id,
                    AgentEvent::ContextTrace {
                        run_id: run_id.clone(),
                        entries: trace_entries,
                    },
                )
                .await
                .map_err(|e| AgentError::Store(e))?;
            }

            let packet = ContextPacket {
                state: model_context_state,
                task_class,
                budget_profile: agent_context::budget_profile_for_task(task_class),
                tool_pack,
                relevant_context,
                recent_turns: message_history,
            };
            let inline_tool_names: Vec<String> =
                prompt_tools.iter().map(|tool| tool.name.clone()).collect();

            let mut built = self.context_builder.build_packet_with_dynamic(
                model.clone(),
                &prompt,
                &attachments,
                packet,
                prompt_tools,
                dynamic_prefix.clone(),
            )?;

            if self.provider.capabilities().supports_prompt_cache_key {
                built.request.prompt_cache_key =
                    Some(prompt_cache_key(&session_id, &mode, &built.request.tools));
            }
            if self.provider.capabilities().supports_stateful_turns {
                built.request.previous_response_id = previous_response_id;
            }

            sink.emit(
                run_id,
                AgentEvent::ContextBuilt {
                    run_id: run_id.clone(),
                    token_estimate: built.token_estimate,
                    files: built.files.clone(),
                    summaries: built.summaries.clone(),
                    section_estimates: built.section_estimates.clone(),
                },
            )
            .await
            .map_err(|e| AgentError::Store(e))?;

            let mut stream = self.provider.stream(built.request, cancel.clone()).await?;

            let mut assistant_text = String::new();
            let mut assistant_appended = false;
            let mut pending_tool: Option<(ToolCallId, String, String)> = None;
            let mut tool_args = String::new();
            let mut pending_completions: Vec<(ToolCallId, String, serde_json::Value)> = Vec::new();
            let mut last_patch_preview_len = 0usize;
            let mut last_patch_preview_at = std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(1))
                .unwrap_or_else(std::time::Instant::now);

            while let Some(delta) = stream.next().await {
                cancel.check_cancelled()?;
                match delta? {
                    ModelDelta::Text(text) => {
                        assistant_text.push_str(&text);
                        sink.emit_delta(
                            run_id,
                            AgentEvent::AssistantTextDelta {
                                run_id: run_id.clone(),
                                text,
                            },
                        )
                        .await
                        .map_err(|e| AgentError::Store(e))?;
                    }
                    ModelDelta::Reasoning(text) => {
                        sink.emit_delta(
                            run_id,
                            AgentEvent::ReasoningDelta {
                                run_id: run_id.clone(),
                                text,
                            },
                        )
                        .await
                        .map_err(|e| AgentError::Store(e))?;
                    }
                    ModelDelta::ToolCallStarted { id, name } => {
                        pending_tool = Some((id.clone(), name.clone(), String::new()));
                        tool_args.clear();
                        if !self
                            .active_runs
                            .lock()
                            .await
                            .get(run_id)
                            .is_some_and(|r| r.announced_tools.contains(&id))
                        {
                            sink.emit_delta(
                                run_id,
                                AgentEvent::ToolCallStarted {
                                    run_id: run_id.clone(),
                                    call_id: id.clone(),
                                    name: name.clone(),
                                    args_preview: String::new(),
                                    dedupe_key: None,
                                    risk: RiskLevel::SafeRead,
                                },
                            )
                            .await
                            .map_err(|e| AgentError::Store(e))?;
                            if let Some(run) = self.active_runs.lock().await.get_mut(run_id) {
                                run.announced_tools.insert(id.clone());
                                run.in_flight_tools.insert(id);
                            }
                        }
                    }
                    ModelDelta::ToolCallArgumentsDelta { id, json_delta } => {
                        if pending_tool.as_ref().is_some_and(|(pid, _, _)| pid == &id) {
                            tool_args.push_str(&json_delta);
                            let preview = pending_tool
                                .as_ref()
                                .map(|(_, name, _)| {
                                    args_preview_raw(&self.tools.registry, name, &tool_args)
                                })
                                .unwrap_or_default();
                            let dedupe_key = pending_tool.as_ref().and_then(|(_, name, _)| {
                                self.tools
                                    .registry
                                    .streaming_tool_call_dedupe_key(name, &tool_args)
                            });
                            sink.emit_delta(
                                run_id,
                                AgentEvent::ToolCallUpdated {
                                    run_id: run_id.clone(),
                                    call_id: id.clone(),
                                    args_preview: preview,
                                    dedupe_key,
                                },
                            )
                            .await
                            .map_err(|e| AgentError::Store(e))?;
                            if pending_tool.as_ref().is_some_and(|(_, name, _)| {
                                is_patch_proposing_tool(&self.tools.registry, name)
                            }) {
                                if let Some((_, name, _)) = pending_tool.as_ref() {
                                    if let Some(diff) =
                                        self.tools.registry.get(name).and_then(|tool| {
                                            tool.streaming_patch_preview(&tool_args, &project_root)
                                        })
                                    {
                                        if diff.len() > last_patch_preview_len
                                            && last_patch_preview_at.elapsed()
                                                >= std::time::Duration::from_millis(100)
                                        {
                                            last_patch_preview_len = diff.len();
                                            last_patch_preview_at = std::time::Instant::now();
                                            sink.emit_delta(
                                                run_id,
                                                AgentEvent::PatchPreviewUpdated {
                                                    call_id: id.clone(),
                                                    unified_diff: diff,
                                                },
                                            )
                                            .await
                                            .map_err(|e| AgentError::Store(e))?;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ModelDelta::ToolCallCompleted {
                        id,
                        name,
                        arguments,
                    } => {
                        pending_completions.push((id, name, arguments));
                        pending_tool = None;
                    }
                    ModelDelta::Usage(usage) => {
                        sink.emit(
                            run_id,
                            AgentEvent::UsageUpdated {
                                run_id: run_id.clone(),
                                input_tokens: usage.input_tokens,
                                output_tokens: usage.output_tokens,
                                cache_read_tokens: usage.cache_read_tokens,
                                cache_write_tokens: usage.cache_write_tokens,
                                estimated_cost_usd: usage.cost_usd,
                            },
                        )
                        .await
                        .map_err(|e| AgentError::Store(e))?;
                    }
                    ModelDelta::Done => {
                        drain_inline_tools_from_text(
                            &mut assistant_text,
                            &mut pending_completions,
                            &inline_tool_names,
                        );
                        if !assistant_text.is_empty() {
                            self.append_assistant_text(run_id, &assistant_text).await;
                            assistant_appended = true;
                        }
                        break;
                    }
                }
            }

            if !pending_completions.is_empty() {
                // Execute an all-read-only batch from a single turn concurrently; read-only tools
                // never pause, so ordering of approvals/patches is unaffected.
                let all_parallel = pending_completions.len() > 1
                    && pending_completions
                        .iter()
                        .all(|(_, name, _)| is_parallel_safe_tool(&self.tools.registry, name));
                if all_parallel {
                    for (id, name, arguments) in &pending_completions {
                        self.append_assistant_tool_call(run_id, id, name, arguments.clone())
                            .await;
                    }
                    let futures = pending_completions.iter().map(|(id, name, arguments)| {
                        self.handle_tool_call(
                            run_id,
                            id,
                            name,
                            arguments.clone(),
                            &project_root,
                            &project_id,
                            &session_id,
                            &mode,
                        )
                    });
                    for result in futures::future::join_all(futures).await {
                        result?;
                    }
                    continue;
                }
                for (id, name, arguments) in pending_completions {
                    self.append_assistant_tool_call(run_id, &id, &name, arguments.clone())
                        .await;
                    let result = self
                        .handle_tool_call(
                            run_id,
                            &id,
                            &name,
                            arguments,
                            &project_root,
                            &project_id,
                            &session_id,
                            &mode,
                        )
                        .await?;
                    if result == ToolLoopAction::Paused {
                        return Ok(());
                    }
                }
                continue;
            }

            if let Some((id, name, _)) = pending_tool.take() {
                if !tool_args.is_empty() {
                    let arguments = serde_json::from_str(&tool_args)
                        .unwrap_or_else(|_| serde_json::Value::String(tool_args.clone()));
                    self.append_assistant_tool_call(run_id, &id, &name, arguments.clone())
                        .await;
                    let result = self
                        .handle_tool_call(
                            run_id,
                            &id,
                            &name,
                            arguments,
                            &project_root,
                            &project_id,
                            &session_id,
                            &mode,
                        )
                        .await?;
                    if result == ToolLoopAction::Paused {
                        return Ok(());
                    }
                    continue;
                }
            }

            if pending_tool.is_none() && !assistant_text.is_empty() {
                drain_inline_tools_from_text(
                    &mut assistant_text,
                    &mut pending_completions,
                    &inline_tool_names,
                );
                if !pending_completions.is_empty() {
                    continue;
                }
                if !assistant_appended {
                    self.append_assistant_text(run_id, &assistant_text).await;
                }
                self.emit_plan_artifact_if_needed(run_id, &mode, &assistant_text)
                    .await?;
                // Text-only completion without explicit Done from some providers
                sink.emit(
                    run_id,
                    AgentEvent::RunFinished {
                        run_id: run_id.clone(),
                        status: RunStatus::Completed,
                    },
                )
                .await
                .map_err(|e| AgentError::Store(e))?;
                self.store
                    .update_run_status(run_id, RunStatus::Completed, Some(Utc::now()))
                    .map_err(|e| AgentError::Store(e))?;
                self.clear_pending_approvals_for_run(run_id).await;
                self.active_runs.lock().await.remove(run_id);
                return Ok(());
            }
        }
    }

    async fn emit_plan_artifact_if_needed(
        &self,
        run_id: &RunId,
        mode: &AgentMode,
        assistant_text: &str,
    ) -> Result<(), AgentError> {
        if !matches!(mode, AgentMode::PlanOnly) {
            return Ok(());
        }
        let Some(markdown) = extract_proposed_plan(assistant_text) else {
            return Ok(());
        };
        self.sink()
            .emit(
                run_id,
                AgentEvent::PlanUpdated {
                    run_id: run_id.clone(),
                    markdown,
                    created_at: Utc::now().to_rfc3339(),
                },
            )
            .await
            .map_err(|e| AgentError::Store(e))
    }
}

fn extract_proposed_plan(text: &str) -> Option<String> {
    let start_tag = "<proposed_plan>";
    let end_tag = "</proposed_plan>";

    let mut cursor = 0usize;
    let mut best: Option<String> = None;
    while let Some(rel_start) = text[cursor..].find(start_tag) {
        let start = cursor + rel_start + start_tag.len();
        let Some(rel_end) = text[start..].find(end_tag) else {
            break;
        };
        let end = start + rel_end;
        let markdown = text[start..end].trim();
        if !markdown.is_empty()
            && best
                .as_ref()
                .is_none_or(|current| markdown.len() >= current.len())
        {
            best = Some(markdown.to_string());
        }
        cursor = end + end_tag.len();
    }
    best
}

fn infer_tool_phase(
    history: &[ModelMessage],
    _task_class: agent_protocol::TaskClass,
    mode: &AgentMode,
) -> ToolPhase {
    if matches!(mode, AgentMode::PlanOnly | AgentMode::ChatOnly) {
        return ToolPhase::Explore;
    }

    let mut saw_tool = false;
    for message in history.iter().rev() {
        if let Some(name) = message.name.as_deref() {
            if matches!(
                name,
                "edit_file" | "write_file" | "delete_file" | "apply_patch"
            ) {
                return ToolPhase::Validate;
            }
            saw_tool = true;
        }
    }
    if saw_tool {
        ToolPhase::Edit
    } else {
        ToolPhase::Explore
    }
}

fn should_run_context_selection(task_class: agent_protocol::TaskClass, prompt: &str) -> bool {
    if matches!(task_class, agent_protocol::TaskClass::ArchitectureQuestion) {
        return true;
    }
    let lower = prompt.to_lowercase();
    ["understand", "explain", "review", "architecture"]
        .iter()
        .any(|needle| lower.contains(needle))
}

fn prompt_cache_key(
    session_id: &SessionId,
    mode: &AgentMode,
    tools: &[agent_protocol::PromptToolSpec],
) -> String {
    let mut hasher = DefaultHasher::new();
    agent_context::SYSTEM_PROMPT_VERSION.hash(&mut hasher);
    session_id.0.hash(&mut hasher);
    format!("{mode:?}").hash(&mut hasher);
    for tool in tools {
        tool.name.hash(&mut hasher);
        tool.description.hash(&mut hasher);
    }
    format!("vortex:{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::{extract_proposed_plan, infer_tool_phase};
    use agent_protocol::{
        AgentMode, ModelMessage, ModelMessageContent, ModelMessageRole, TaskClass, ToolPhase,
    };

    #[test]
    fn extracts_plan_markdown_from_wrapped_response() {
        let text = "Intro\n<proposed_plan>\n# Plan\n- Step\n</proposed_plan>\nTail";
        assert_eq!(
            extract_proposed_plan(text).as_deref(),
            Some("# Plan\n- Step")
        );
    }

    #[test]
    fn ignores_unwrapped_plan_text() {
        assert!(extract_proposed_plan("# Plan\n- Step").is_none());
    }

    #[test]
    fn prefers_later_non_placeholder_plan_block() {
        let text = "<proposed_plan>...</proposed_plan>\nnotes\n<proposed_plan>\n# Plan\n- Real step\n</proposed_plan>";
        assert_eq!(
            extract_proposed_plan(text).as_deref(),
            Some("# Plan\n- Real step")
        );
    }

    #[test]
    fn infer_tool_phase_moves_from_explore_to_edit_to_validate() {
        assert_eq!(
            infer_tool_phase(&[], TaskClass::BugFix, &AgentMode::ApplyWithApproval),
            ToolPhase::Explore
        );

        let search_history = vec![ModelMessage {
            role: ModelMessageRole::Tool,
            content: ModelMessageContent::text("searched"),
            tool_call_id: None,
            name: Some("search_project".into()),
            tool_calls: None,
        }];
        assert_eq!(
            infer_tool_phase(
                &search_history,
                TaskClass::BugFix,
                &AgentMode::ApplyWithApproval
            ),
            ToolPhase::Edit
        );

        let edit_history = vec![ModelMessage {
            role: ModelMessageRole::Tool,
            content: ModelMessageContent::text("edited"),
            tool_call_id: None,
            name: Some("edit_file".into()),
            tool_calls: None,
        }];
        assert_eq!(
            infer_tool_phase(
                &edit_history,
                TaskClass::BugFix,
                &AgentMode::ApplyWithApproval
            ),
            ToolPhase::Validate
        );
    }
}

/// Shallow listing of the workspace root for the dynamic system prompt. Directories get a trailing
/// `/`; hidden entries and heavy build dirs are skipped. Bounded and sorted for determinism.
fn top_level_tree(root: &std::path::Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || matches!(name.as_str(), "target" | "node_modules") {
                return None;
            }
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            Some(if is_dir { format!("{name}/") } else { name })
        })
        .collect();
    names.sort();
    names.truncate(60);
    names
}

/// If the model inlined tool syntax in assistant text, extract and queue execution.
fn drain_inline_tools_from_text(
    assistant_text: &mut String,
    pending_completions: &mut Vec<(ToolCallId, String, serde_json::Value)>,
    tool_names: &[String],
) {
    if !pending_completions.is_empty() {
        return;
    }
    let (clean, calls) =
        extract_inline_tool_calls_with_tools(assistant_text, tool_names.iter().cloned());
    if calls.is_empty() {
        return;
    }
    *assistant_text = clean;
    for (i, call) in calls.into_iter().enumerate() {
        let id = ToolCallId::new(format!("inline-backstop-{i}"));
        pending_completions.push((id, call.name, call.arguments));
    }
}
