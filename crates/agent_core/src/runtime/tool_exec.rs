use super::*;

impl AgentRuntime {
    pub(crate) async fn handle_tool_call(
        &self,
        run_id: &RunId,
        call_id: &ToolCallId,
        name: &str,
        arguments: serde_json::Value,
        project_root: &PathBuf,
        project_id: &ProjectId,
        session_id: &SessionId,
        mode: &AgentMode,
    ) -> Result<ToolLoopAction, AgentError> {
        let sink = self.sink();
        let assess_ctx = ToolContext {
            project_root: project_root.clone(),
            project_id: project_id.clone(),
            session_id: session_id.clone(),
            run_id: run_id.clone(),
            mode: mode.clone(),
            output_sink: None,
        };
        if let Some(reason) = self.validate_android_execution_lane(run_id, name).await {
            self.finish_tool_assessment_error(run_id, call_id, name, &arguments, &reason)
                .await?;
            return Ok(ToolLoopAction::Continue);
        }
        let decision = match self
            .tools
            .classify(mode, name, &arguments, project_id, &assess_ctx)
            .await
        {
            Ok(decision) => decision,
            Err(reason) => {
                self.finish_tool_assessment_error(run_id, call_id, name, &arguments, &reason)
                    .await?;
                return Ok(ToolLoopAction::Continue);
            }
        };
        let risk = match &decision {
            ApprovalDecision::Allow => RiskLevel::SafeRead,
            ApprovalDecision::AskUser { risk, .. } => *risk,
            ApprovalDecision::Deny { .. } => RiskLevel::Critical,
        };

        let already_announced = self
            .active_runs
            .lock()
            .await
            .get(run_id)
            .is_some_and(|r| r.announced_tools.contains(call_id));

        if !already_announced {
            sink.emit_delta(
                run_id,
                AgentEvent::ToolCallStarted {
                    run_id: run_id.clone(),
                    call_id: call_id.clone(),
                    name: name.to_string(),
                    args_preview: args_preview(&self.tools.registry, name, &arguments),
                    dedupe_key: self.tools.registry.tool_call_dedupe_key(name, &arguments),
                    risk,
                },
            )
            .await
            .map_err(|e| AgentError::Store(e))?;
            if let Some(run) = self.active_runs.lock().await.get_mut(run_id) {
                run.announced_tools.insert(call_id.clone());
                run.in_flight_tools.insert(call_id.clone());
            }
        } else {
            sink.emit_delta(
                run_id,
                AgentEvent::ToolCallUpdated {
                    run_id: run_id.clone(),
                    call_id: call_id.clone(),
                    args_preview: args_preview(&self.tools.registry, name, &arguments),
                    dedupe_key: self.tools.registry.tool_call_dedupe_key(name, &arguments),
                },
            )
            .await
            .map_err(|e| AgentError::Store(e))?;
        }

        if let ApprovalDecision::Deny { reason } = decision {
            sink.emit(
                run_id,
                AgentEvent::ToolCallFinished {
                    run_id: Some(run_id.clone()),
                    call_id: call_id.clone(),
                    status: ToolStatus::Denied,
                    summary: reason.clone(),
                    body: None,
                },
            )
            .await
            .map_err(|e| AgentError::Store(e))?;
            if let Some(run) = self.active_runs.lock().await.get_mut(run_id) {
                run.in_flight_tools.remove(call_id);
            }
            self.append_tool_message(run_id, call_id, name, &reason, true)
                .await;
            return Ok(ToolLoopAction::Continue);
        }

        if let ApprovalDecision::AskUser { reason, risk } = decision {
            let approval_id = agent_protocol::ApprovalId::new(uuid::Uuid::new_v4().to_string());
            sink.emit(
                run_id,
                AgentEvent::ApprovalRequested {
                    run_id: Some(run_id.clone()),
                    approval_id: approval_id.clone(),
                    call_id: call_id.clone(),
                    risk,
                    reason,
                    command_preview: Some(args_preview(&self.tools.registry, name, &arguments))
                        .filter(|preview| !preview.is_empty()),
                    affected_paths: vec![project_root.clone()],
                },
            )
            .await
            .map_err(|e| AgentError::Store(e))?;
            self.store
                .update_run_status(run_id, RunStatus::PausedForApproval, None)
                .map_err(|e| AgentError::Store(e))?;
            let pending = PendingApprovalState {
                approval_id: approval_id.clone(),
                call_id: call_id.clone(),
                tool_name: name.to_string(),
                risk,
                command_pattern: agent_sandbox::command_rule_pattern(name, &arguments),
                arguments,
            };
            self.pending_approvals
                .lock()
                .await
                .insert(approval_id.0.clone(), (run_id.clone(), pending.clone()));
            if let Some(run) = self.active_runs.lock().await.get_mut(run_id) {
                run.pending_approval = Some(pending);
            }
            return Ok(ToolLoopAction::Paused);
        }

        self.execute_tool_call(
            run_id,
            call_id,
            name,
            arguments,
            project_root,
            project_id,
            session_id,
            mode,
        )
        .await?;
        if is_patch_proposing_tool(&self.tools.registry, name) {
            let awaiting_apply = self
                .active_runs
                .lock()
                .await
                .get(run_id)
                .is_some_and(|r| r.pending_patch.is_some());
            if awaiting_apply {
                if mode.auto_applies_patches() {
                    self.apply_pending_patch_inline(run_id, mode).await?;
                    return Ok(ToolLoopAction::Continue);
                }
                self.store
                    .update_run_status(run_id, RunStatus::PausedForApproval, None)
                    .map_err(|e| AgentError::Store(e))?;
                return Ok(ToolLoopAction::Paused);
            }
        }
        if self
            .tools
            .registry
            .has_category(name, agent_protocol::ToolCategory::AskUser)
        {
            let awaiting_choice = self
                .active_runs
                .lock()
                .await
                .get(run_id)
                .is_some_and(|r| r.pending_choice.is_some());
            if awaiting_choice {
                self.store
                    .update_run_status(run_id, RunStatus::PausedForApproval, None)
                    .map_err(|e| AgentError::Store(e))?;
                return Ok(ToolLoopAction::Paused);
            }
        }
        Ok(ToolLoopAction::Continue)
    }

    /// Merge the requested todos into run state and surface them via `TodoUpdated`.
    pub(crate) async fn execute_todo_write(
        &self,
        run_id: &RunId,
        call_id: &ToolCallId,
        arguments: &serde_json::Value,
    ) -> Result<(), AgentError> {
        let incoming: Vec<agent_protocol::TodoItem> = arguments
            .get("todos")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let merge = arguments
            .get("merge")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let todos = {
            let mut runs = self.active_runs.lock().await;
            let Some(run) = runs.get_mut(run_id) else {
                return Ok(());
            };
            run.todos = agent_protocol::merge_todos(&run.todos, incoming, merge);
            run.todos.clone()
        };

        let sink = self.sink();
        sink.emit(
            run_id,
            AgentEvent::TodoUpdated {
                run_id: run_id.clone(),
                todos: todos.clone(),
            },
        )
        .await
        .map_err(|e| AgentError::Store(e))?;

        let completed = todos
            .iter()
            .filter(|t| matches!(t.status, agent_protocol::TodoStatus::Completed))
            .count();
        let output = format!(
            "Todos updated: {} items ({} completed)",
            todos.len(),
            completed
        );
        let result = ToolResult {
            call_id: call_id.clone(),
            name: "todo_write".into(),
            output,
            is_error: false,
        };
        self.finish_tool(run_id, call_id, arguments, &result).await
    }

    /// Surface an `ask_user` prompt as a `ChoiceRequested` event and record the pending choice so
    /// a later `SubmitChoice` command can resume the run. Does not finish the tool call yet.
    pub(crate) async fn execute_ask_user(
        &self,
        run_id: &RunId,
        call_id: &ToolCallId,
        arguments: &serde_json::Value,
    ) -> Result<(), AgentError> {
        let prompt = arguments
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("Choose an option")
            .to_string();
        let summary = arguments
            .get("summary")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        let recommended_option_id = arguments
            .get("recommended_option_id")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        let allow_custom = arguments
            .get("allow_custom")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let blocking_reason = arguments
            .get("blocking_reason")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        let options: Vec<agent_protocol::ChoiceOption> = arguments
            .get("options")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let choice_id = uuid::Uuid::new_v4().to_string();

        if let Some(run) = self.active_runs.lock().await.get_mut(run_id) {
            run.pending_choice = Some(PendingChoiceState {
                choice_id: choice_id.clone(),
                call_id: call_id.clone(),
                tool_name: "ask_user".into(),
                options: options.clone(),
            });
        }

        let sink = self.sink();
        sink.emit(
            run_id,
            AgentEvent::ChoiceRequested {
                run_id: run_id.clone(),
                choice_id,
                prompt,
                options,
                summary,
                recommended_option_id,
                allow_custom,
                blocking_reason,
            },
        )
        .await
        .map_err(|e| AgentError::Store(e))?;
        Ok(())
    }

    pub(crate) async fn execute_tool_call(
        &self,
        run_id: &RunId,
        call_id: &ToolCallId,
        name: &str,
        arguments: serde_json::Value,
        project_root: &PathBuf,
        project_id: &ProjectId,
        session_id: &SessionId,
        mode: &AgentMode,
    ) -> Result<(), AgentError> {
        let tool_category = self.tools.registry.capabilities(name).category;
        if tool_category == agent_protocol::ToolCategory::Delegate {
            return self
                .execute_delegate_tool(run_id, call_id, &arguments, project_root)
                .await;
        }
        if name == "todo_write" {
            return self.execute_todo_write(run_id, call_id, &arguments).await;
        }
        if tool_category == agent_protocol::ToolCategory::AskUser {
            return self.execute_ask_user(run_id, call_id, &arguments).await;
        }
        tracing::info!(
            run_id = %run_id.0,
            call_id = %call_id.0,
            tool = name,
            "executing tool"
        );
        let sink = self.sink();
        self.tools
            .record_tool_start(run_id, call_id, name, &arguments, RiskLevel::Low)
            .map_err(|e| AgentError::Store(e))?;

        let output_emitted = Arc::new(AtomicBool::new(false));
        let emitted_flag = output_emitted.clone();
        let event_tx = self.event_tx.clone();
        let call_id_for_sink = call_id.clone();
        let output_sink = ToolOutputSink {
            emit: Arc::new(move |stream, chunk| {
                emitted_flag.store(true, Ordering::Relaxed);
                let _ = event_tx.send(AgentEvent::ToolOutputDelta {
                    run_id: None,
                    call_id: call_id_for_sink.clone(),
                    stream,
                    chunk,
                });
            }),
        };

        let ctx = ToolContext {
            project_root: project_root.clone(),
            project_id: project_id.clone(),
            session_id: session_id.clone(),
            run_id: run_id.clone(),
            mode: mode.clone(),
            output_sink: Some(output_sink),
        };

        if tool_category == agent_protocol::ToolCategory::PatchProposal {
            if let Ok(result) = self
                .tools
                .registry
                .execute(name, arguments.clone(), ctx.clone(), call_id)
                .await
            {
                if let Ok(proposal) =
                    serde_json::from_str::<agent_protocol::PatchProposal>(&result.output)
                {
                    if let Some(run) = self.active_runs.lock().await.get_mut(run_id) {
                        run.pending_patch = Some(proposal.clone());
                    }
                    sink.emit(
                        run_id,
                        AgentEvent::PatchProposed {
                            patch_id: proposal.id.clone(),
                            files: proposal.files.iter().map(|f| f.path.clone()).collect(),
                            unified_diff: proposal.unified_diff.clone(),
                            risk: proposal.risk,
                        },
                    )
                    .await
                    .map_err(|e| AgentError::Store(e))?;
                }
                self.finish_tool(run_id, call_id, &arguments, &result)
                    .await?;
                return Ok(());
            }
        }

        let args_for_finish = arguments.clone();
        let result = if self.tools.registry.caches_output(name) {
            if let Some(cache_key) = self.tools.registry.output_cache_key(name, &arguments) {
                let cached = self
                    .active_runs
                    .lock()
                    .await
                    .get(run_id)
                    .and_then(|run| run.read_cache.get(&cache_key).cloned());
                if let Some(output) = cached {
                    if let Some(run) = self.active_runs.lock().await.get_mut(run_id) {
                        run.read_cache_hits += 1;
                    }
                    ToolResult {
                        call_id: call_id.clone(),
                        name: name.into(),
                        output,
                        is_error: false,
                    }
                } else {
                    self.execute_tool(name, arguments, ctx, call_id).await
                }
            } else {
                self.execute_tool(name, arguments, ctx, call_id).await
            }
        } else {
            self.execute_tool(name, arguments, ctx, call_id).await
        };

        if tool_category == agent_protocol::ToolCategory::PatchApply {
            if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&result.output) {
                if let Some(checkpoint) = payload.get("checkpoint").and_then(|v| {
                    serde_json::from_value::<agent_protocol::WorkspaceCheckpoint>(v.clone()).ok()
                }) {
                    let _ = self.store.save_checkpoint(&checkpoint);
                }
                if !result.is_error {
                    let patch_id = payload
                        .get("patch_id")
                        .and_then(|v| v.as_str())
                        .map(agent_protocol::PatchId::new)
                        .unwrap_or_else(|| agent_protocol::PatchId::new(""));
                    let files: Vec<std::path::PathBuf> = payload
                        .get("files")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(std::path::PathBuf::from))
                                .collect()
                        })
                        .unwrap_or_default();
                    sink.emit(run_id, AgentEvent::PatchApplied { patch_id, files })
                        .await
                        .map_err(|e| AgentError::Store(e))?;
                }
            }
        }

        self.emit_android_panel_events(run_id, &result).await?;

        if !output_emitted.load(Ordering::Relaxed) && !result.output.is_empty() {
            let skip_ui_stream =
                self.tools.registry.suppresses_live_output(name) || name.starts_with("android.");
            if !skip_ui_stream {
                emit_tool_output_live(
                    &self.event_tx,
                    run_id,
                    call_id,
                    OutputStreamKind::Stdout,
                    &result.output,
                )
                .await;
            }
            if !skip_ui_stream {
                sink.persist_only(
                    run_id,
                    AgentEvent::ToolOutputDelta {
                        run_id: Some(run_id.clone()),
                        call_id: call_id.clone(),
                        stream: OutputStreamKind::Stdout,
                        chunk: result.output.clone(),
                    },
                )
                .await
                .map_err(|e| AgentError::Store(e))?;
            }
        }

        if self.tools.registry.caches_output(name) && !result.is_error {
            if let Some(path) = self.tools.registry.output_cache_key(name, &args_for_finish) {
                if let Some(run) = self.active_runs.lock().await.get_mut(run_id) {
                    run.read_cache.insert(path, result.output.clone());
                }
            }
        }

        self.finish_tool(run_id, call_id, &args_for_finish, &result)
            .await
    }

    async fn emit_android_panel_events(
        &self,
        run_id: &RunId,
        result: &ToolResult,
    ) -> Result<(), AgentError> {
        if !self.tools.registry.is_android_tool(&result.name) {
            return Ok(());
        }
        let Ok(payload) = serde_json::from_str::<serde_json::Value>(&result.output) else {
            return Ok(());
        };
        let sink = self.sink();
        if let Some(observation) = payload.get("observation").and_then(|value| {
            serde_json::from_value::<agent_protocol::AndroidObservation>(value.clone()).ok()
        }) {
            sink.emit(
                run_id,
                AgentEvent::AndroidObservationUpdated {
                    run_id: run_id.clone(),
                    observation,
                },
            )
            .await
            .map_err(AgentError::Store)?;
        }
        if let Some(device) = payload.get("device").and_then(|value| {
            serde_json::from_value::<agent_protocol::AndroidDeviceRef>(value.clone()).ok()
        }) {
            sink.emit(
                run_id,
                AgentEvent::AndroidSessionUpdated {
                    run_id: run_id.clone(),
                    session: agent_protocol::AndroidSessionState {
                        device: Some(device),
                        status: "Ready".into(),
                        ..Default::default()
                    },
                },
            )
            .await
            .map_err(AgentError::Store)?;
        }
        if let Some(action) = payload.get("action_visualization").and_then(|value| {
            serde_json::from_value::<agent_protocol::AndroidActionVisualization>(value.clone()).ok()
        }) {
            sink.emit(
                run_id,
                AgentEvent::AndroidActionPreviewed {
                    run_id: run_id.clone(),
                    action,
                },
            )
            .await
            .map_err(AgentError::Store)?;
        }
        if let Some(action) = payload.get("action_trace").and_then(|value| {
            serde_json::from_value::<agent_protocol::AndroidActionTrace>(value.clone()).ok()
        }) {
            sink.emit(
                run_id,
                AgentEvent::AndroidActionCompleted {
                    run_id: run_id.clone(),
                    action,
                },
            )
            .await
            .map_err(AgentError::Store)?;
        }
        if let Some(journey) = payload.get("journey").and_then(|value| {
            serde_json::from_value::<agent_protocol::AndroidJourney>(value.clone()).ok()
        }) {
            sink.emit(
                run_id,
                AgentEvent::AndroidJourneyUpdated {
                    run_id: run_id.clone(),
                    journey,
                },
            )
            .await
            .map_err(AgentError::Store)?;
        }
        Ok(())
    }

    async fn validate_android_execution_lane(
        &self,
        run_id: &RunId,
        tool_name: &str,
    ) -> Option<String> {
        if !self.tools.registry.is_android_tool(tool_name) {
            return None;
        }
        let runs = self.active_runs.lock().await;
        let run = runs.get(run_id)?;
        android_lane_violation(
            &run.android_lane,
            self.tools.registry.policy(tool_name).android_lane,
        )
    }

    pub(crate) async fn execute_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
        ctx: ToolContext,
        call_id: &ToolCallId,
    ) -> ToolResult {
        match self
            .tools
            .registry
            .execute(name, arguments, ctx, call_id)
            .await
        {
            Ok(result) => result,
            Err(message) => ToolResult {
                call_id: call_id.clone(),
                name: name.to_string(),
                output: message,
                is_error: true,
            },
        }
    }

    async fn finish_tool_assessment_error(
        &self,
        run_id: &RunId,
        call_id: &ToolCallId,
        name: &str,
        arguments: &serde_json::Value,
        reason: &str,
    ) -> Result<(), AgentError> {
        let sink = self.sink();
        let already_announced = self
            .active_runs
            .lock()
            .await
            .get(run_id)
            .is_some_and(|r| r.announced_tools.contains(call_id));

        if !already_announced {
            sink.emit_delta(
                run_id,
                AgentEvent::ToolCallStarted {
                    run_id: run_id.clone(),
                    call_id: call_id.clone(),
                    name: name.to_string(),
                    args_preview: args_preview(&self.tools.registry, name, arguments),
                    dedupe_key: self.tools.registry.tool_call_dedupe_key(name, arguments),
                    risk: RiskLevel::SafeRead,
                },
            )
            .await
            .map_err(AgentError::Store)?;
            if let Some(run) = self.active_runs.lock().await.get_mut(run_id) {
                run.announced_tools.insert(call_id.clone());
                run.in_flight_tools.insert(call_id.clone());
            }
        } else {
            sink.emit_delta(
                run_id,
                AgentEvent::ToolCallUpdated {
                    run_id: run_id.clone(),
                    call_id: call_id.clone(),
                    args_preview: args_preview(&self.tools.registry, name, arguments),
                    dedupe_key: self.tools.registry.tool_call_dedupe_key(name, arguments),
                },
            )
            .await
            .map_err(AgentError::Store)?;
        }

        let summary = tool_finish_summary(&self.tools.registry, name, arguments, reason, true);
        sink.emit(
            run_id,
            AgentEvent::ToolCallFinished {
                run_id: Some(run_id.clone()),
                call_id: call_id.clone(),
                status: ToolStatus::Failed,
                summary,
                body: Some(reason.to_string()),
            },
        )
        .await
        .map_err(AgentError::Store)?;
        if let Some(run) = self.active_runs.lock().await.get_mut(run_id) {
            run.in_flight_tools.remove(call_id);
        }
        self.append_tool_message(run_id, call_id, name, reason, true)
            .await;
        Ok(())
    }

    /// Apply a proposed patch immediately (Agent mode) without pausing the run loop.
    async fn apply_pending_patch_inline(
        &self,
        run_id: &RunId,
        mode: &AgentMode,
    ) -> Result<(), AgentError> {
        let (proposal, project_root, project_id, session_id) = {
            let mut runs = self.active_runs.lock().await;
            let run = runs
                .get_mut(run_id)
                .ok_or_else(|| AgentError::Other("run not found".into()))?;
            let proposal = run
                .pending_patch
                .take()
                .ok_or_else(|| AgentError::Other("no pending patch".into()))?;
            (
                proposal,
                run.project_root.clone(),
                run.project_id.clone(),
                run.session_id.clone(),
            )
        };

        let call_id = ToolCallId::new(uuid::Uuid::new_v4().to_string());
        let args = serde_json::json!({ "unified_diff": proposal.unified_diff });
        tracing::info!(
            run_id = %run_id.0,
            patch_id = %proposal.id.0,
            "auto-applying patch"
        );
        self.execute_tool_call(
            run_id,
            &call_id,
            "apply_patch",
            args,
            &project_root,
            &project_id,
            &session_id,
            mode,
        )
        .await
    }
}

fn android_lane_violation(
    lane: &super::AndroidExecutionLane,
    policy: agent_protocol::AndroidLanePolicy,
) -> Option<String> {
    match policy {
        agent_protocol::AndroidLanePolicy::None => None,
        agent_protocol::AndroidLanePolicy::DenyInAgentMode => {
            Some("raw tap not allowed in agent mode".into())
        }
        agent_protocol::AndroidLanePolicy::Observe | agent_protocol::AndroidLanePolicy::Utility => {
            None
        }
        agent_protocol::AndroidLanePolicy::Action => {
            if lane.last_observation_id.is_none() {
                return Some("observe required before Android actions".into());
            }
            if lane.action_since_observation {
                return Some("post-action observe required before another Android action".into());
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_lane_requires_observe_before_action() {
        let lane = super::super::AndroidExecutionLane::default();
        assert_eq!(
            android_lane_violation(&lane, agent_protocol::AndroidLanePolicy::Action).as_deref(),
            Some("observe required before Android actions")
        );
    }

    #[test]
    fn android_lane_requires_post_action_observe() {
        let lane = super::super::AndroidExecutionLane {
            last_observation_id: Some("obs-1".into()),
            action_since_observation: true,
        };
        assert_eq!(
            android_lane_violation(&lane, agent_protocol::AndroidLanePolicy::Action).as_deref(),
            Some("post-action observe required before another Android action")
        );
    }

    #[test]
    fn android_lane_allows_observe_after_action() {
        let lane = super::super::AndroidExecutionLane {
            last_observation_id: Some("obs-1".into()),
            action_since_observation: true,
        };
        assert_eq!(
            android_lane_violation(&lane, agent_protocol::AndroidLanePolicy::Observe),
            None
        );
    }
}
