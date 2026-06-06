use super::*;

impl AgentRuntime {
    pub(crate) async fn finish_tool(
        &self,
        run_id: &RunId,
        call_id: &ToolCallId,
        args: &serde_json::Value,
        result: &ToolResult,
    ) -> Result<(), AgentError> {
        let sink = self.sink();
        let status = if result.is_error {
            ToolStatus::Failed
        } else {
            ToolStatus::Completed
        };
        let summary = tool_finish_summary(
            &self.tools.registry,
            &result.name,
            args,
            &result.output,
            result.is_error,
        );
        let tool_policy = self.tools.registry.policy(&result.name);
        let model_summary = model_facing_tool_summary(
            run_id,
            call_id,
            &result.name,
            args,
            &result.output,
            &summary,
            result.is_error,
            &tool_policy,
        );
        let body = if self.tools.registry.persists_result_body(&result.name) {
            Some(result.output.clone())
        } else {
            None
        };
        sink.emit(
            run_id,
            AgentEvent::ToolCallFinished {
                run_id: Some(run_id.clone()),
                call_id: call_id.clone(),
                status: status.clone(),
                summary,
                body,
            },
        )
        .await
        .map_err(|e| AgentError::Store(e))?;
        self.tools
            .record_tool_finish(run_id, call_id, status)
            .map_err(|e| AgentError::Store(e))?;
        if let Some(run) = self.active_runs.lock().await.get_mut(run_id) {
            run.in_flight_tools.remove(call_id);
            run.tool_results.push(result.clone());
            run.model_context_state
                .record_tool_summary(model_summary.clone());
            update_android_execution_lane(
                &mut run.android_lane,
                tool_policy.android_lane,
                &model_summary,
            );
            run.tool_call_count += 1;
            run.message_history.push(ModelMessage {
                role: ModelMessageRole::Tool,
                content: model_summary_text(&model_summary).into(),
                tool_call_id: Some(call_id.clone()),
                name: Some(result.name.clone()),
                tool_calls: None,
            });
        }
        Ok(())
    }
}

fn update_android_execution_lane(
    lane: &mut super::AndroidExecutionLane,
    lane_policy: agent_protocol::AndroidLanePolicy,
    summary: &agent_protocol::ToolResultSummary,
) {
    if matches!(lane_policy, agent_protocol::AndroidLanePolicy::Observe) {
        if let Some(evidence) = &summary.android_evidence {
            lane.last_observation_id = Some(evidence.observation_id.clone());
            lane.action_since_observation = false;
        }
        return;
    }
    if matches!(lane_policy, agent_protocol::AndroidLanePolicy::Action) {
        if let Some(evidence) = &summary.android_evidence {
            lane.last_observation_id = Some(evidence.observation_id.clone());
            if evidence.action.is_some() {
                lane.action_since_observation = true;
            }
        }
    }
}

pub(crate) fn model_facing_tool_summary(
    run_id: &RunId,
    call_id: &ToolCallId,
    tool: &str,
    args: &serde_json::Value,
    output: &str,
    summary: &str,
    is_error: bool,
    policy: &agent_protocol::ToolPolicy,
) -> agent_protocol::ToolResultSummary {
    let affected_paths = affected_paths_from_args(policy, args, output);
    let ranges = ranges_from_summary_policy(args, policy.summary.arg_range.as_ref());
    let truncated = output.contains("[truncated") || output.contains("…[truncated");
    let android_evidence = android_tool_evidence(policy.runtime_family, output);
    let mut facts = Vec::new();
    if is_error {
        facts.push(format!("error: {}", first_line(output)));
    } else {
        facts.extend(extract_facts(
            policy.runtime_family,
            output,
            android_evidence.as_ref(),
        ));
    }
    let mut next_actions = Vec::new();
    if truncated {
        next_actions.push(
            "re-open the raw result or request a narrower file slice if more detail is needed"
                .into(),
        );
    }
    if policy.summary.prefer_line_bounded_follow_up
        && ranges.is_empty()
        && !affected_paths.is_empty()
    {
        next_actions.push("prefer a line-bounded read for follow-up inspection".into());
    }
    agent_protocol::ToolResultSummary {
        call_id: call_id.clone(),
        tool: tool.to_string(),
        summary: summary.to_string(),
        facts,
        affected_paths,
        ranges,
        raw_handle: format!("tool://{}/{}", run_id.0, call_id.0),
        token_cost: agent_context::estimate_tokens(output),
        truncated,
        next_actions,
        is_error,
        android_evidence,
    }
}

fn model_summary_text(summary: &agent_protocol::ToolResultSummary) -> String {
    serde_json::to_string_pretty(summary).unwrap_or_else(|_| summary.summary.clone())
}

fn affected_paths_from_args(
    policy: &agent_protocol::ToolPolicy,
    args: &serde_json::Value,
    output: &str,
) -> Vec<std::path::PathBuf> {
    let mut paths = summary_arg_paths(args, &policy.summary.arg_paths);
    paths.extend(output_paths_from_summary_policy(
        output,
        policy.summary.output_paths.as_ref(),
    ));
    paths.sort();
    paths.dedup();
    paths
}

fn summary_arg_paths(
    args: &serde_json::Value,
    config: &[agent_protocol::ToolSummaryArgPath],
) -> Vec<std::path::PathBuf> {
    config
        .iter()
        .filter_map(|entry| {
            let value = args.get(&entry.field).and_then(|v| v.as_str())?;
            summary_path_value(value, entry.kind)
        })
        .collect()
}

fn summary_path_value(
    value: &str,
    kind: agent_protocol::ToolSummaryArgPathKind,
) -> Option<std::path::PathBuf> {
    if value.is_empty() {
        return None;
    }
    match kind {
        agent_protocol::ToolSummaryArgPathKind::PlainPath => {
            if value.contains('#') || value.starts_with("git:") || value.starts_with("page:") {
                None
            } else {
                Some(std::path::PathBuf::from(value))
            }
        }
    }
}

fn output_paths_from_summary_policy(
    output: &str,
    config: Option<&agent_protocol::ToolSummaryOutputPaths>,
) -> Vec<std::path::PathBuf> {
    let Some(config) = config else {
        return Vec::new();
    };
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(output) else {
        return Vec::new();
    };
    payload
        .get(&config.array_field)
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            entry
                .get(&config.path_field)
                .and_then(|value| value.as_str())
        })
        .filter(|path| !path.is_empty())
        .map(std::path::PathBuf::from)
        .collect()
}

fn ranges_from_summary_policy(
    args: &serde_json::Value,
    config: Option<&agent_protocol::ToolSummaryArgRange>,
) -> Vec<agent_protocol::ToolResultRange> {
    let Some(config) = config else {
        return Vec::new();
    };
    let Some(path) = args
        .get(&config.path_field)
        .and_then(|v| v.as_str())
        .and_then(|value| {
            summary_path_value(value, agent_protocol::ToolSummaryArgPathKind::PlainPath)
        })
    else {
        return Vec::new();
    };
    let start_line = args
        .get(&config.start_line_field)
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    let end_line = args
        .get(&config.end_line_field)
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    if start_line.is_none() && end_line.is_none() {
        return Vec::new();
    }
    vec![agent_protocol::ToolResultRange {
        path,
        start_line,
        end_line,
    }]
}

fn extract_facts(
    runtime_family: agent_protocol::ToolRuntimeFamily,
    output: &str,
    android_evidence: Option<&agent_protocol::AndroidToolEvidence>,
) -> Vec<String> {
    if matches!(
        runtime_family,
        agent_protocol::ToolRuntimeFamily::AndroidDevice
    ) {
        return android_evidence
            .map(extract_android_facts)
            .unwrap_or_default();
    }
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with("[truncated") && !line.contains("…[truncated"))
        .filter(|line| !line.starts_with('<') && !line.starts_with("</"))
        .take(6)
        .map(|line| line.chars().take(180).collect())
        .collect()
}

fn extract_android_facts(evidence: &agent_protocol::AndroidToolEvidence) -> Vec<String> {
    let screen = evidence
        .activity
        .as_deref()
        .or(evidence.package.as_deref())
        .unwrap_or("screen");
    let mut facts = vec![
        format!("observation_id: {}", evidence.observation_id),
        format!("screen: {screen}"),
        format!("visible_targets: {}", evidence.visible_targets.len()),
    ];
    if let Some(action) = &evidence.action {
        let target = action.target.as_deref().unwrap_or("screen");
        facts.push(format!(
            "action: {} · {} · {}",
            action.action, target, action.status
        ));
    }
    facts.extend(
        evidence
            .visible_targets
            .iter()
            .take(4)
            .map(|target| format!("target: {} [{}]", target.label, target.id)),
    );
    facts
}

fn android_tool_evidence(
    runtime_family: agent_protocol::ToolRuntimeFamily,
    output: &str,
) -> Option<agent_protocol::AndroidToolEvidence> {
    if !matches!(
        runtime_family,
        agent_protocol::ToolRuntimeFamily::AndroidDevice
    ) {
        return None;
    }
    let payload = serde_json::from_str::<serde_json::Value>(output).ok()?;
    let observation = payload.get("observation").and_then(|value| {
        serde_json::from_value::<agent_protocol::AndroidObservation>(value.clone()).ok()
    })?;
    let action = payload
        .get("action_trace")
        .and_then(|value| {
            serde_json::from_value::<agent_protocol::AndroidActionTrace>(value.clone()).ok()
        })
        .map(|action| agent_protocol::AndroidActionEvidence {
            action: action.action,
            target: action.target,
            status: action.status,
            before_observation: action.before_observation,
            after_observation: action.after_observation,
        });
    Some(agent_protocol::AndroidToolEvidence {
        observation_id: observation.observation_id.clone(),
        package: observation.package.clone(),
        activity: observation.activity.clone(),
        visible_targets: observation
            .visible_targets
            .iter()
            .take(20)
            .enumerate()
            .map(
                |(index, node)| agent_protocol::AndroidVisibleTargetEvidence {
                    id: android_target_evidence_id(index, node),
                    label: android_target_label(node),
                    text: node.text.clone(),
                    resource_id: node.resource_id.clone(),
                    content_desc: node.content_desc.clone(),
                    clickable: node.clickable,
                    enabled: node.enabled,
                    visible: node.visible,
                },
            )
            .collect(),
        action,
    })
}

fn android_target_evidence_id(index: usize, node: &agent_protocol::AndroidUiNode) -> String {
    if let Some(resource_id) = node
        .resource_id
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        return format!("rid:{resource_id}");
    }
    if let Some(text) = node.text.as_deref().filter(|value| !value.is_empty()) {
        return format!("text:{text}");
    }
    if let Some(content_desc) = node
        .content_desc
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        return format!("desc:{content_desc}");
    }
    format!("node:{index}:{}", node.class_name)
}

fn android_target_label(node: &agent_protocol::AndroidUiNode) -> String {
    node.text
        .as_deref()
        .filter(|value| !value.is_empty())
        .or(node
            .content_desc
            .as_deref()
            .filter(|value| !value.is_empty()))
        .or(node
            .resource_id
            .as_deref()
            .filter(|value| !value.is_empty()))
        .unwrap_or(node.class_name.as_str())
        .to_string()
}

fn first_line(output: &str) -> String {
    output
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("tool failed")
        .chars()
        .take(180)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_summary_uses_handle_and_not_raw_output_blob() {
        let run_id = RunId::new("run-1");
        let call_id = ToolCallId::new("call-1");
        let raw = "line 1\nline 2\nline 3\n[truncated: showing lines 1-3 of 100]";
        let summary = model_facing_tool_summary(
            &run_id,
            &call_id,
            "read_file",
            &serde_json::json!({
                "path": "src/lib.rs",
                "start_line": 1,
                "end_line": 3
            }),
            raw,
            "Read src/lib.rs",
            false,
            &agent_protocol::ToolPolicy {
                summary: agent_protocol::ToolSummaryPolicy {
                    prefer_line_bounded_follow_up: true,
                    arg_paths: vec![agent_protocol::ToolSummaryArgPath {
                        field: "path".into(),
                        ..agent_protocol::ToolSummaryArgPath::default()
                    }],
                    arg_range: Some(agent_protocol::ToolSummaryArgRange {
                        path_field: "path".into(),
                        start_line_field: "start_line".into(),
                        end_line_field: "end_line".into(),
                    }),
                    ..agent_protocol::ToolSummaryPolicy::default()
                },
                ..agent_protocol::ToolPolicy::default()
            },
        );
        assert_eq!(summary.raw_handle, "tool://run-1/call-1");
        assert_eq!(
            summary.affected_paths[0],
            std::path::PathBuf::from("src/lib.rs")
        );
        assert!(summary.truncated);
        assert_eq!(summary.ranges.len(), 1);
        let text = model_summary_text(&summary);
        assert!(text.contains("Read src/lib.rs"));
        assert!(!text.contains("showing lines 1-3 of 100"));
    }

    #[test]
    fn android_tool_summary_extracts_evidence() {
        let run_id = RunId::new("run-android");
        let call_id = ToolCallId::new("call-android");
        let raw = serde_json::json!({
            "summary": "Android · observed MainActivity · 2 targets",
            "observation": {
                "observation_id": "obs-1",
                "device": null,
                "package": "com.example",
                "activity": "MainActivity",
                "screen": {"width": 1080.0, "height": 1920.0},
                "visible_targets": [
                    {
                        "text": "Settings",
                        "resource_id": "com.example:id/settings",
                        "content_desc": null,
                        "class_name": "android.widget.TextView",
                        "package": "com.example",
                        "clickable": true,
                        "enabled": true,
                        "visible": true,
                        "bounds": {"left": 0.0, "top": 0.0, "right": 100.0, "bottom": 40.0}
                    }
                ],
                "screenshot_ref": null,
                "ui_tree_ref": null,
                "timestamp_ms": 1
            }
        })
        .to_string();
        let summary = model_facing_tool_summary(
            &run_id,
            &call_id,
            "android.observe",
            &serde_json::json!({}),
            &raw,
            "Android · observed",
            false,
            &agent_protocol::ToolPolicy {
                runtime_family: agent_protocol::ToolRuntimeFamily::AndroidDevice,
                ..agent_protocol::ToolPolicy::default()
            },
        );
        let evidence = summary.android_evidence.expect("android evidence");
        assert_eq!(evidence.observation_id, "obs-1");
        assert_eq!(
            evidence.visible_targets[0].id,
            "rid:com.example:id/settings"
        );
        assert_eq!(evidence.visible_targets[0].label, "Settings");
    }

    #[test]
    fn summary_policy_can_extract_affected_paths_from_output() {
        let run_id = RunId::new("run-gradle");
        let call_id = ToolCallId::new("call-gradle");
        let raw = serde_json::json!({
            "files": [
                {"path": "app/build.gradle.kts"},
                {"path": "gradle/libs.versions.toml"}
            ]
        })
        .to_string();
        let summary = model_facing_tool_summary(
            &run_id,
            &call_id,
            "inspect_gradle_dependencies",
            &serde_json::json!({}),
            &raw,
            "Inspected Gradle dependencies",
            false,
            &agent_protocol::ToolPolicy {
                summary: agent_protocol::ToolSummaryPolicy {
                    output_paths: Some(agent_protocol::ToolSummaryOutputPaths {
                        array_field: "files".into(),
                        path_field: "path".into(),
                    }),
                    ..agent_protocol::ToolSummaryPolicy::default()
                },
                ..agent_protocol::ToolPolicy::default()
            },
        );
        assert_eq!(
            summary.affected_paths,
            vec![
                std::path::PathBuf::from("app/build.gradle.kts"),
                std::path::PathBuf::from("gradle/libs.versions.toml"),
            ]
        );
    }
}
