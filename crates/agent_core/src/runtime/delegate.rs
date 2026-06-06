use super::*;

impl AgentRuntime {
    pub(crate) async fn execute_delegate_tool(
        &self,
        run_id: &RunId,
        call_id: &ToolCallId,
        arguments: &serde_json::Value,
        project_root: &PathBuf,
    ) -> Result<(), AgentError> {
        let task = arguments
            .get("task")
            .and_then(|v| v.as_str())
            .unwrap_or("Delegated task")
            .to_string();
        let (
            session_id,
            project_id,
            mode,
            parent_model,
            child_model,
            depth,
            attachments,
            parent_cancel,
        ) = {
            let runs = self.active_runs.lock().await;
            let run = runs
                .get(run_id)
                .ok_or(AgentError::Other("run not found".into()))?;
            let child_model = run
                .subagent_model
                .clone()
                .unwrap_or_else(|| run.model.clone());
            (
                run.session_id.clone(),
                run.project_id.clone(),
                run.mode.clone(),
                run.model.clone(),
                child_model,
                run.depth,
                run.attachments.clone(),
                run.cancel.clone(),
            )
        };
        if depth > 0 {
            let denied = ToolResult {
                call_id: call_id.clone(),
                name: "delegate".to_string(),
                output: "delegate is only allowed at depth 0".to_string(),
                is_error: true,
            };
            return self.finish_tool(run_id, call_id, arguments, &denied).await;
        }

        let child_run_id = RunId::new(uuid::Uuid::new_v4().to_string());
        self.store
            .create_run(&StoredRun {
                id: child_run_id.clone(),
                session_id: session_id.clone(),
                parent_run_id: Some(run_id.clone()),
                depth: 1,
                model: child_model.clone(),
                mode: mode.clone(),
                status: RunStatus::Running,
                started_at: Utc::now(),
                finished_at: None,
            })
            .map_err(AgentError::Store)?;

        let cancel = parent_cancel;
        let task_class = classify_task(&task);
        let tool_pack = tool_pack_for_task(task_class);
        self.active_runs.lock().await.insert(
            child_run_id.clone(),
            ActiveRun {
                session_id: session_id.clone(),
                project_id: project_id.clone(),
                _parent_run_id: Some(run_id.clone()),
                depth: 1,
                project_root: project_root.clone(),
                mode: mode.clone(),
                model: child_model.clone(),
                subagent_model: Some(child_model.clone()),
                cancel: cancel.clone(),
                prompt: task.clone(),
                attachments: attachments.clone(),
                message_history: Vec::new(),
                model_context_state: ModelContextState::new(&task),
                task_class,
                tool_pack,
                tool_results: Vec::new(),
                tool_call_count: 0,
                started_at: Instant::now(),
                pending_approval: None,
                announced_tools: HashSet::new(),
                in_flight_tools: HashSet::new(),
                read_cache: HashMap::new(),
                read_cache_hits: 0,
                pending_patch: None,
                todos: Vec::new(),
                pending_choice: None,
                android_lane: super::AndroidExecutionLane::default(),
            },
        );
        let sink = self.sink();
        sink.emit(
            &child_run_id,
            AgentEvent::RunStarted {
                run_id: child_run_id.clone(),
                session_id,
                model: child_model.clone(),
                mode,
                depth: 1,
                parent_run_id: Some(run_id.clone()),
            },
        )
        .await
        .map_err(AgentError::Store)?;
        sink.emit(
            run_id,
            AgentEvent::SubagentStarted {
                parent_run_id: run_id.clone(),
                child_run_id: child_run_id.clone(),
                call_id: call_id.clone(),
                model: child_model.clone(),
                task: task.clone(),
            },
        )
        .await
        .map_err(AgentError::Store)?;

        let run_result = std::pin::Pin::from(Box::new(self.run_loop(
            &child_run_id,
            task,
            attachments,
            cancel,
        )))
        .await;
        let child_status = self
            .store
            .get_run(&child_run_id)
            .map_err(AgentError::Store)?
            .map(|r| r.status)
            .unwrap_or(RunStatus::Failed);
        let summary = if run_result.is_ok() && matches!(child_status, RunStatus::Completed) {
            "Subagent completed delegated task".to_string()
        } else {
            "Subagent failed delegated task".to_string()
        };
        sink.emit(
            run_id,
            AgentEvent::SubagentFinished {
                parent_run_id: run_id.clone(),
                child_run_id: child_run_id.clone(),
                call_id: call_id.clone(),
                status: child_status.clone(),
                summary: summary.clone(),
            },
        )
        .await
        .map_err(AgentError::Store)?;

        let result = ToolResult {
            call_id: call_id.clone(),
            name: "delegate".to_string(),
            output: if parent_model == child_model {
                summary
            } else {
                format!("{summary} using {}", child_model.0)
            },
            is_error: !matches!(child_status, RunStatus::Completed),
        };
        self.finish_tool(run_id, call_id, arguments, &result).await
    }
}
