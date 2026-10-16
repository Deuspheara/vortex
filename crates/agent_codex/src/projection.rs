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
    pub texts: HashMap<String, String>,
    pub started: HashSet<String>,
    pub completed: HashSet<String>,
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
            texts: HashMap::new(),
            started: HashSet::new(),
            completed: HashSet::new(),
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
                text: suffix,
            }
        })
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
        if kind == "reasoning" || kind == "userMessage" {
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
                        .map(|c| {
                            format!(
                                "--- a/{}\n+++ b/{}\n{}",
                                string(c, "path"),
                                string(c, "path"),
                                string(c, "diff")
                            )
                        })
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
                "declined" => ToolStatus::Denied,
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
pub fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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
