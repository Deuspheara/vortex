use agent_protocol::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Saved with every event, so a resumed item only emits the missing text suffix.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunProjection {
    pub run_id: RunId,
    pub session_id: SessionId,
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub active: bool,
    #[serde(default)]
    pub usage: [u64; 3],
    #[serde(default)]
    pub context_usage: Option<(u64, Option<u64>)>,
    #[serde(default)]
    pub auto_review: bool,
    pub texts: HashMap<String, String>,
    pub started: HashSet<String>,
    pub completed: HashSet<String>,
    #[serde(default)]
    pub previews: HashMap<String, String>,
    #[serde(default)]
    pub paths: HashMap<String, Vec<String>>,
}
impl RunProjection {
    pub fn new(run_id: RunId, session_id: SessionId, thread_id: String) -> Self {
        Self {
            run_id,
            session_id,
            thread_id,
            turn_id: None,
            active: true,
            usage: [0; 3],
            context_usage: None,
            auto_review: false,
            texts: HashMap::new(),
            started: HashSet::new(),
            completed: HashSet::new(),
            previews: HashMap::new(),
            paths: HashMap::new(),
        }
    }
    pub fn usage_update(&mut self, total: &Value) -> Option<AgentEvent> {
        let next = [
            total["inputTokens"].as_u64().unwrap_or(0),
            total["outputTokens"].as_u64().unwrap_or(0),
            total["cachedInputTokens"].as_u64().unwrap_or(0),
        ];
        let delta = std::array::from_fn::<_, 3, _>(|i| next[i].saturating_sub(self.usage[i]));
        for (i, value) in next.into_iter().enumerate() {
            self.usage[i] = self.usage[i].max(value);
        }
        if delta == [0; 3] {
            return None;
        }
        Some(AgentEvent::UsageUpdated {
            run_id: self.run_id.clone(),
            input_tokens: delta[0],
            output_tokens: delta[1],
            cache_read_tokens: Some(delta[2]),
            cache_write_tokens: None,
            estimated_cost_usd: None,
        })
    }
    pub fn context_usage_update(&mut self, usage: &Value) -> Option<AgentEvent> {
        let used_tokens = usage["last"]["totalTokens"].as_u64()?;
        let max_tokens = usage["modelContextWindow"].as_u64().filter(|n| *n > 0);
        let snapshot = (used_tokens, max_tokens);
        if self.context_usage == Some(snapshot) {
            return None;
        }
        self.context_usage = Some(snapshot);
        Some(AgentEvent::ContextWindowUpdated {
            run_id: self.run_id.clone(),
            used_tokens,
            max_tokens,
        })
    }
    pub fn call_id(&self, item: &str) -> ToolCallId {
        ToolCallId::new(format!("codex/{}/{item}", self.run_id.0))
    }
    pub fn text(
        &mut self,
        key: &str,
        text: &str,
        snapshot: bool,
        reasoning: bool,
    ) -> Option<AgentEvent> {
        let separate = !self.texts.contains_key(key)
            && !reasoning
            && self
                .texts
                .keys()
                .any(|key| !key.starts_with("reasoning/") && !key.starts_with("plan/"));
        let seen = self.texts.entry(key.to_string()).or_default();
        let suffix = if snapshot {
            text.strip_prefix(seen.as_str())?
        } else {
            text
        };
        if suffix.is_empty() {
            return None;
        }
        let suffix = suffix.to_string();
        seen.push_str(&suffix);
        Some(if reasoning {
            AgentEvent::ReasoningDelta {
                run_id: self.run_id.clone(),
                text: suffix,
            }
        } else {
            AgentEvent::AssistantTextDelta {
                run_id: self.run_id.clone(),
                text: if separate {
                    format!("\n\n{suffix}")
                } else {
                    suffix
                },
            }
        })
    }
    /// Automatic reviews are persisted as explicitly labeled activity, never as
    /// a human approval response or proof that the reviewed command executed.
    pub fn approval_review(&mut self, params: &Value, complete: bool) -> Vec<AgentEvent> {
        let review_id = string(params, "reviewId");
        if review_id.is_empty() {
            return vec![];
        }
        let key = format!("approval-review/{review_id}");
        let action = &params["action"];
        let target = action["command"]
            .as_str()
            .unwrap_or_else(|| string(action, "type"));
        let preview = format!("Reviewing · {target}");
        let call_id = self.call_id(&key);
        let mut events = Vec::new();
        if self.started.insert(key.clone()) {
            self.previews.insert(key.clone(), preview.clone());
            events.push(AgentEvent::ToolCallStarted {
                run_id: self.run_id.clone(),
                call_id: call_id.clone(),
                name: "codex_approval_review".into(),
                args_preview: preview,
                dedupe_key: None,
                risk: RiskLevel::High,
            });
        }
        if complete && self.completed.insert(key) {
            let review = &params["review"];
            let (label, status) = match string(review, "status") {
                "approved" => ("Approved", ToolStatus::Completed),
                "denied" => ("Denied", ToolStatus::Denied),
                "timedOut" => ("Timed out", ToolStatus::Failed),
                "aborted" => ("Aborted", ToolStatus::Cancelled),
                _ => ("Review unavailable", ToolStatus::Failed),
            };
            events.push(AgentEvent::ToolCallFinished {
                run_id: Some(self.run_id.clone()),
                call_id,
                status,
                summary: format!("{label} · {target}"),
                body: Some(format!(
                    "{label}\nRisk: {}\nUser authorization: {}\n{}",
                    string(review, "riskLevel"),
                    string(review, "userAuthorization"),
                    review["rationale"]
                        .as_str()
                        .unwrap_or("No review rationale was provided.")
                )),
            });
        }
        events
    }

    pub fn item(&mut self, item: &Value, complete: bool) -> Vec<AgentEvent> {
        let id = string(item, "id");
        let kind = string(item, "type");
        let mut events = Vec::new();
        if kind == "agentMessage" {
            if complete && let Some(e) = self.text(id, string(item, "text"), true, false) {
                events.push(e);
            }
            return events;
        }
        if kind == "reasoning" {
            if complete {
                for (index, summary) in item["summary"].as_array().into_iter().flatten().enumerate()
                {
                    if let Some(e) = self.text(
                        &reasoning_key(id, index),
                        summary.as_str().unwrap_or(""),
                        true,
                        true,
                    ) {
                        events.push(e);
                    }
                }
                if item["summary"].as_array().is_none_or(|v| v.is_empty()) {
                    for (index, content) in
                        item["content"].as_array().into_iter().flatten().enumerate()
                    {
                        if let Some(event) = self.text(
                            &format!("reasoning/raw/{id}/{index}"),
                            content.as_str().unwrap_or(""),
                            true,
                            true,
                        ) {
                            events.push(event);
                        }
                    }
                }
            }
            return events;
        }
        if kind == "userMessage" {
            return events;
        }
        if kind == "plan" {
            if complete && self.completed.insert(id.into()) {
                events.push(AgentEvent::PlanUpdated {
                    run_id: self.run_id.clone(),
                    markdown: string(item, "text").into(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                });
            }
            return events;
        }
        let name = match kind {
            "commandExecution" => "codex_command",
            "fileChange" => "codex_file_change",
            "mcpToolCall" => "codex_mcp",
            "webSearch" => "codex_web_search",
            _ => "codex_activity",
        };
        let preview = match kind {
            "commandExecution" => string(item, "command").to_string(),
            "fileChange" => item["changes"]
                .as_array()
                .map(|changes| {
                    changes
                        .iter()
                        .filter_map(|c| c["path"].as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default(),
            "mcpToolCall" => format!("{} / {}", string(item, "server"), string(item, "tool")),
            "webSearch" => string(item, "query").to_string(),
            _ => kind.to_string(),
        };
        let call_id = self.call_id(id);
        self.previews.insert(id.into(), preview.clone());
        if kind == "fileChange" {
            self.paths.insert(
                id.into(),
                item["changes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| c["path"].as_str().map(str::to_string))
                    .collect(),
            );
        }
        if self.started.insert(id.to_string()) {
            events.push(AgentEvent::ToolCallStarted {
                run_id: self.run_id.clone(),
                call_id: call_id.clone(),
                name: name.into(),
                args_preview: preview.clone(),
                dedupe_key: None,
                risk: RiskLevel::Medium,
            });
        }
        if kind == "fileChange" && !self.completed.contains(id) {
            let diff = item["changes"]
                .as_array()
                .map(|changes| {
                    changes
                        .iter()
                        .map(|c| change_diff(c))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            if !diff.is_empty() {
                events.push(AgentEvent::FileChangesObserved {
                    run_id: self.run_id.clone(),
                    applied: complete && string(item, "status") == "completed",
                    call_id: call_id.clone(),
                    unified_diff: diff,
                });
            }
        }
        if complete && self.completed.insert(id.to_string()) {
            let status = match string(item, "status") {
                "failed" => ToolStatus::Failed,
                "declined" | "denied" => ToolStatus::Denied,
                "interrupted" | "cancelled" => ToolStatus::Cancelled,
                _ if kind == "commandExecution"
                    && item["exitCode"].as_i64().is_some_and(|code| code != 0) =>
                {
                    ToolStatus::Failed
                }
                _ => ToolStatus::Completed,
            };
            let body = item["aggregatedOutput"]
                .as_str()
                .map(str::to_string)
                .or_else(|| {
                    item.get("result")
                        .filter(|v| !v.is_null())
                        .map(Value::to_string)
                });
            let body = body.or_else(|| {
                item.get("error")
                    .filter(|v| !v.is_null())
                    .map(Value::to_string)
            });
            events.push(AgentEvent::ToolCallFinished {
                run_id: Some(self.run_id.clone()),
                call_id,
                status,
                summary: preview,
                body,
            });
        }
        events
    }
}
pub fn reasoning_key(id: &str, index: usize) -> String {
    if index == 0 {
        format!("reasoning/{id}")
    } else {
        format!("reasoning/{id}/{index}")
    }
}

fn change_diff(change: &Value) -> String {
    let path = string(change, "path");
    let diff = string(change, "diff");
    // Codex sends literal contents for adds/deletes, and unified hunks for updates.
    let kind = string(&change["kind"], "type");
    match kind {
        "add" | "delete" => {
            let lines = diff.lines().collect::<Vec<_>>();
            let (old, new, hunk, prefix) = if kind == "add" {
                (
                    "/dev/null".to_string(),
                    format!("b/{path}"),
                    format!("@@ -0,0 +1,{} @@", lines.len()),
                    '+',
                )
            } else {
                (
                    format!("a/{path}"),
                    "/dev/null".to_string(),
                    format!("@@ -1,{} +0,0 @@", lines.len()),
                    '-',
                )
            };
            format!(
                "--- {old}\n+++ {new}\n{hunk}\n{}",
                lines
                    .into_iter()
                    .map(|l| format!("{prefix}{l}\n"))
                    .collect::<String>()
            )
        }
        _ if diff.starts_with("diff --git ") || diff.starts_with("--- ") => diff.to_string(),
        _ => format!("--- a/{path}\n+++ b/{path}\n{diff}"),
    }
}
pub fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn automatic_reviews_fail_closed_and_survive_projection_replay() {
        for (wire_status, expected) in [
            ("approved", ToolStatus::Completed),
            ("denied", ToolStatus::Denied),
            ("timedOut", ToolStatus::Failed),
            ("aborted", ToolStatus::Cancelled),
            ("futureStatus", ToolStatus::Failed),
        ] {
            let mut p = RunProjection::new(RunId::new("r"), SessionId::new("s"), "t".into());
            p.auto_review = true;
            let mut params = json!({"reviewId":"review-1","action":{"type":"command","command":"echo test"},"review":{"status":"inProgress"}});
            assert!(
                matches!(&p.approval_review(&params, false)[0], AgentEvent::ToolCallStarted{name,..} if name == "codex_approval_review")
            );
            let mut p: RunProjection =
                serde_json::from_value(serde_json::to_value(p).unwrap()).unwrap();
            assert!(p.auto_review);
            assert!(p.approval_review(&params, false).is_empty());
            params["review"] = json!({"status":wire_status,"riskLevel":"low","userAuthorization":"medium","rationale":"Fixture review rationale"});
            let events = p.approval_review(&params, true);
            assert!(
                matches!(&events[..], [AgentEvent::ToolCallFinished{status,body:Some(body),..}] if *status == expected && body.contains("Fixture review rationale"))
            );
            assert!(p.approval_review(&params, true).is_empty());
            // Completed-only notification still produces a visible correlated activity pair.
            let mut fresh = RunProjection::new(RunId::new("r"), SessionId::new("s"), "t".into());
            assert_eq!(fresh.approval_review(&params, true).len(), 2);
            assert!(fresh.approval_review(&json!({}), true).is_empty());
        }
    }

    #[test]
    fn snapshot_after_stream_and_restart_only_emits_missing_suffix() {
        let mut p = RunProjection::new(RunId::new("r"), SessionId::new("s"), "t".into());
        assert!(p.text("i", "hé", false, false).is_some());
        let mut p: RunProjection =
            serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert!(
            matches!(p.text("i","héllo",true,false),Some(AgentEvent::AssistantTextDelta{text,..}) if text=="llo")
        );
        assert!(p.text("i", "héllo", true, false).is_none());
        assert!(p.text("i", "different", true, false).is_none());
    }
    #[test]
    fn completed_tool_is_not_replayed() {
        let mut p = RunProjection::new(RunId::new("r"), SessionId::new("s"), "t".into());
        let item = json!({"id":"i","type":"commandExecution","command":"pwd","status":"completed","aggregatedOutput":"/tmp"});
        assert_eq!(p.item(&item, true).len(), 2);
        assert!(p.item(&item, true).is_empty());
    }
    #[test]
    fn literal_new_file_contents_become_reviewable_diff() {
        let diff = change_diff(
            &json!({"path":"proof.txt","kind":{"type":"add"},"diff":"proof version one\n"}),
        );
        assert!(diff.contains("+++ b/proof.txt\n@@ -0,0 +1,1 @@\n+proof version one"));
    }
    #[test]
    fn reasoning_snapshot_reconciles_each_summary_part() {
        let mut p = RunProjection::new(RunId::new("r"), SessionId::new("s"), "t".into());
        p.text(&reasoning_key("i", 0), "First", false, true);
        let item = json!({"id":"i","type":"reasoning","summary":["First part", "Second part"]});
        let events = p.item(&item, true);
        assert!(matches!(&events[0], AgentEvent::ReasoningDelta{text,..} if text == " part"));
        assert!(matches!(&events[1], AgentEvent::ReasoningDelta{text,..} if text == "Second part"));
        assert!(p.item(&item, true).is_empty());
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn completed_file_changes_are_observed_once_not_proposed_again() {
        let mut p = RunProjection::new(RunId::new("r"), SessionId::new("s"), "t".into());
        let item = json!({"id":"edit","type":"fileChange","status":"completed","changes":[{"path":"test.txt","diff":"@@ -1 +1 @@\n-old\n+new"}]});
        let events = p.item(&item, true);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::FileChangesObserved { applied: true, .. }))
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, AgentEvent::PatchProposed { .. }))
        );
        assert!(p.item(&item, true).is_empty());
    }
    #[test]
    fn repeated_usage_snapshots_do_not_double_count() {
        let mut p = RunProjection::new(RunId::new("r"), SessionId::new("s"), "t".into());
        let usage = json!({"inputTokens":100,"outputTokens":10,"cachedInputTokens":40});
        assert!(p.usage_update(&usage).is_some());
        assert!(p.usage_update(&usage).is_none());
        assert!(matches!(
            p.usage_update(&json!({"inputTokens":120,"outputTokens":15,"cachedInputTokens":40})),
            Some(AgentEvent::UsageUpdated {
                input_tokens: 20,
                output_tokens: 5,
                ..
            })
        ));
    }
}

#[cfg(test)]
mod context_usage_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn context_is_last_request_including_output_not_cumulative_or_uncached() {
        let mut p = RunProjection::new(RunId::new("r"), SessionId::new("s"), "t".into());
        let usage = json!({"total":{"totalTokens":104888},
            "last":{"inputTokens":22045,"cachedInputTokens":19456,"outputTokens":53,"totalTokens":22098},
            "modelContextWindow":475000});
        assert!(matches!(
            p.context_usage_update(&usage),
            Some(AgentEvent::ContextWindowUpdated {
                used_tokens: 22098,
                max_tokens: Some(475000),
                ..
            })
        ));
        let mut restored: RunProjection =
            serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert!(restored.context_usage_update(&usage).is_none());
        // Compaction can lower occupancy even when cumulative totals do not change.
        assert!(matches!(
            restored.context_usage_update(
                &json!({"last":{"totalTokens":12000},"modelContextWindow":475000})
            ),
            Some(AgentEvent::ContextWindowUpdated {
                used_tokens: 12000,
                ..
            })
        ));
    }
    #[test]
    fn missing_last_usage_is_not_misreported_as_an_empty_context() {
        let mut p = RunProjection::new(RunId::new("r"), SessionId::new("s"), "t".into());
        assert!(
            p.context_usage_update(&json!({"total":{"totalTokens":104888}}))
                .is_none()
        );
        assert!(matches!(
            p.context_usage_update(&json!({"last":{"totalTokens":1234},"modelContextWindow":null})),
            Some(AgentEvent::ContextWindowUpdated {
                used_tokens: 1234,
                max_tokens: None,
                ..
            })
        ));
    }
}
