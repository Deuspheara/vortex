use std::path::PathBuf;

use agent_protocol::{
    AgentError, AttachmentKind, AttachmentSource, ContextAttachment, ContextBudget,
    ContextBudgetProfile, ContextSectionEstimate, ModelContentPart, ModelId, ModelMessage,
    ModelMessageContent, ModelMessageRole, ModelRequest, TaskClass, ToolPack, ToolResultSummary,
    ToolSpec,
};
use base64::Engine;

const MAX_IMAGE_ATTACHMENTS: usize = 5;
const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_TOTAL_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
const SUPPORTED_IMAGE_MIME_TYPES: &[&str] = &["image/png", "image/jpeg", "image/webp", "image/gif"];

pub struct BuiltContext {
    pub request: ModelRequest,
    pub token_estimate: usize,
    pub files: Vec<PathBuf>,
    pub summaries: Vec<String>,
    pub section_estimates: Vec<ContextSectionEstimate>,
}

#[derive(Clone, Debug, Default)]
pub struct ModelContextState {
    pub task_summary: String,
    pub active_plan: Vec<String>,
    pub decisions: Vec<String>,
    pub touched_files: Vec<PathBuf>,
    pub recent_user_constraints: Vec<String>,
    pub compact_turns: Vec<String>,
    pub tool_result_summaries: Vec<ToolResultSummary>,
}

impl ModelContextState {
    pub fn new(prompt: &str) -> Self {
        Self {
            task_summary: prompt
                .lines()
                .next()
                .unwrap_or(prompt)
                .chars()
                .take(240)
                .collect(),
            ..Self::default()
        }
    }

    pub fn record_tool_summary(&mut self, summary: ToolResultSummary) {
        for path in &summary.affected_paths {
            if !self.touched_files.contains(path) {
                self.touched_files.push(path.clone());
            }
        }
        self.tool_result_summaries.push(summary);
        const MAX_TOOL_SUMMARIES: usize = 24;
        if self.tool_result_summaries.len() > MAX_TOOL_SUMMARIES {
            let drop = self.tool_result_summaries.len() - MAX_TOOL_SUMMARIES;
            self.tool_result_summaries.drain(0..drop);
        }
    }

    fn to_context_block(&self, task_class: TaskClass, tool_pack: ToolPack) -> String {
        let mut out = String::new();
        out.push_str("[TASK_STATE]\n");
        out.push_str(&format!("class: {task_class:?}\n"));
        out.push_str(&format!("tool_pack: {tool_pack:?}\n"));
        if !self.task_summary.trim().is_empty() {
            out.push_str(&format!("objective: {}\n", self.task_summary.trim()));
        }
        append_list(&mut out, "active_plan", &self.active_plan);
        append_list(&mut out, "decisions", &self.decisions);
        if !self.touched_files.is_empty() {
            let files: Vec<String> = self
                .touched_files
                .iter()
                .take(12)
                .map(|p| p.display().to_string())
                .collect();
            append_list(&mut out, "touched_files", &files);
        }
        append_list(
            &mut out,
            "recent_user_constraints",
            &self.recent_user_constraints,
        );
        append_list(&mut out, "compact_turns", &self.compact_turns);
        if !self.tool_result_summaries.is_empty() {
            out.push_str("tool_results:\n");
            for summary in self.tool_result_summaries.iter().rev().take(10).rev() {
                out.push_str(&format!(
                    "- {} {}: {}",
                    summary.tool, summary.call_id.0, summary.summary
                ));
                if summary.truncated {
                    out.push_str(" [truncated]");
                }
                out.push('\n');
                for fact in summary.facts.iter().take(4) {
                    out.push_str(&format!("  fact: {fact}\n"));
                }
                if !summary.next_actions.is_empty() {
                    out.push_str(&format!(
                        "  next: {}\n",
                        summary
                            .next_actions
                            .iter()
                            .take(3)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("; ")
                    ));
                }
                out.push_str(&format!("  raw_handle: {}\n", summary.raw_handle));
                if let Some(evidence) = &summary.android_evidence {
                    out.push_str(&format!(
                        "  android_observation: {}\n",
                        evidence.observation_id
                    ));
                    if let Some(screen) =
                        evidence.activity.as_deref().or(evidence.package.as_deref())
                    {
                        out.push_str(&format!("  android_screen: {screen}\n"));
                    }
                    if let Some(action) = &evidence.action {
                        let target = action.target.as_deref().unwrap_or("screen");
                        out.push_str(&format!(
                            "  android_action: {} · {} · {}\n",
                            action.action, target, action.status
                        ));
                    }
                    for target in evidence.visible_targets.iter().take(8) {
                        out.push_str(&format!(
                            "  android_target: {} | id={} | text={} | resource_id={} | content_desc={} | clickable={} | enabled={} | visible={}\n",
                            target.label,
                            target.id,
                            target.text.as_deref().unwrap_or("-"),
                            target.resource_id.as_deref().unwrap_or("-"),
                            target.content_desc.as_deref().unwrap_or("-"),
                            target.clickable,
                            target.enabled,
                            target.visible,
                        ));
                    }
                }
            }
        }
        out
    }
}

fn append_list(out: &mut String, label: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    out.push_str(label);
    out.push_str(":\n");
    for item in items.iter().filter(|i| !i.trim().is_empty()).take(12) {
        out.push_str("- ");
        out.push_str(item.trim());
        out.push('\n');
    }
}

#[derive(Clone, Debug)]
pub struct ContextPacket {
    pub state: ModelContextState,
    pub task_class: TaskClass,
    pub budget_profile: ContextBudgetProfile,
    pub tool_pack: ToolPack,
    pub relevant_context: Vec<String>,
    pub recent_turns: Vec<ModelMessage>,
}

impl ContextPacket {
    pub fn from_history(prompt: &str, history: &[ModelMessage]) -> Self {
        Self {
            state: ModelContextState::new(prompt),
            task_class: classify_task(prompt),
            budget_profile: budget_profile_for_task(classify_task(prompt)),
            tool_pack: tool_pack_for_task(classify_task(prompt)),
            relevant_context: Vec::new(),
            recent_turns: history.to_vec(),
        }
    }
}

pub struct ContextBuilder {
    pub budget: ContextBudget,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self {
            budget: ContextBudget::default(),
        }
    }
}

/// Rough token estimate (chars/4). Centralised so budget math and `token_estimate` agree.
pub fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

pub fn message_tokens(message: &ModelMessage) -> usize {
    let mut tokens = message.content.estimated_chars().div_ceil(4) + 4;
    if let Some(calls) = &message.tool_calls {
        for call in calls {
            tokens += estimate_tokens(&call.name);
            tokens += estimate_tokens(&call.arguments.to_string());
            tokens += 8;
        }
    }
    tokens
}

pub fn classify_task(prompt: &str) -> TaskClass {
    let p = prompt.to_lowercase();
    if contains_any(
        &p,
        &[
            "dependency",
            "dependencies",
            "gradle",
            "maven",
            "version catalog",
            "libs.versions",
        ],
    ) {
        TaskClass::DependencyUpdate
    } else if contains_any(
        &p,
        &[
            "test failed",
            "failing test",
            "fix ci",
            "ci failure",
            "cargo test",
            "pytest",
            "jest",
        ],
    ) {
        TaskClass::TestFailure
    } else if contains_any(
        &p,
        &[
            "ui",
            "sidebar",
            "button",
            "component",
            "layout",
            "css",
            "gpui",
            "screenshot",
        ],
    ) {
        TaskClass::UiChange
    } else if contains_any(&p, &["bug", "fix", "error", "panic", "crash", "regression"]) {
        TaskClass::BugFix
    } else if contains_any(&p, &["refactor", "cleanup", "rename", "restructure"]) {
        TaskClass::Refactor
    } else if contains_any(&p, &["architecture", "design", "plan", "explain", "review"]) {
        TaskClass::ArchitectureQuestion
    } else {
        TaskClass::Unknown
    }
}

pub fn budget_profile_for_task(task_class: TaskClass) -> ContextBudgetProfile {
    match task_class {
        TaskClass::DependencyUpdate => ContextBudgetProfile::SmallTask,
        TaskClass::UiChange | TaskClass::BugFix | TaskClass::TestFailure => {
            ContextBudgetProfile::Normal
        }
        TaskClass::ArchitectureQuestion | TaskClass::Refactor | TaskClass::Unknown => {
            ContextBudgetProfile::Normal
        }
    }
}

pub fn tool_pack_for_task(task_class: TaskClass) -> ToolPack {
    match task_class {
        TaskClass::DependencyUpdate => ToolPack::Dependency,
        TaskClass::UiChange => ToolPack::UiBrowser,
        TaskClass::TestFailure => ToolPack::GitCi,
        TaskClass::ArchitectureQuestion => ToolPack::Planning,
        TaskClass::BugFix | TaskClass::Refactor => ToolPack::CodeEdit,
        TaskClass::Unknown => ToolPack::General,
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn tools_tokens(tools: &[ToolSpec]) -> usize {
    serde_json::to_string(tools)
        .map(|s| estimate_tokens(&s))
        .unwrap_or(0)
}

/// Truncate a single message's content to a per-message token cap, preserving head + tail so the
/// model still sees the shape of a large tool result.
fn cap_message_content(message: &ModelMessage, max_tokens: usize) -> ModelMessage {
    let max_chars = max_tokens.saturating_mul(4);
    if message.content.estimated_chars() <= max_chars || max_chars == 0 {
        return message.clone();
    }
    let mut capped = message.clone();
    capped.content = capped.content.cap_text(max_chars);
    capped
}

/// Fit `history` into `budget` tokens, keeping the most recent messages and summarising older ones
/// into a single compact note. Avoids leaving an orphan tool result at the window boundary.
pub fn fit_history(history: &[ModelMessage], budget_tokens: usize) -> (Vec<ModelMessage>, usize) {
    if history.is_empty() {
        return (Vec::new(), 0);
    }
    let mut kept_rev: Vec<ModelMessage> = Vec::new();
    let mut used = 0usize;
    let mut cut = history.len();
    for (ix, message) in history.iter().enumerate().rev() {
        let cost = message_tokens(message);
        if used + cost > budget_tokens && !kept_rev.is_empty() {
            cut = ix + 1;
            break;
        }
        used += cost;
        kept_rev.push(message.clone());
        cut = ix;
    }
    // Drop leading orphan tool results (their assistant tool_call was trimmed away).
    while kept_rev
        .last()
        .is_some_and(|m| matches!(m.role, ModelMessageRole::Tool))
    {
        kept_rev.pop();
        cut += 1;
    }
    kept_rev.reverse();
    let dropped = cut;
    (kept_rev, dropped)
}

/// Build a compact summary message for `dropped` earlier messages.
fn compaction_summary(dropped: &[ModelMessage]) -> ModelMessage {
    let tool_calls = dropped.iter().filter(|m| m.tool_calls.is_some()).count();
    let mut recent_topics: Vec<String> = dropped
        .iter()
        .filter(|m| matches!(m.role, ModelMessageRole::User | ModelMessageRole::Assistant))
        .filter(|m| !m.content.to_text_lossy().trim().is_empty())
        .rev()
        .take(5)
        .map(|m| {
            let text = m.content.to_text_lossy();
            let line = text.lines().next().unwrap_or_default();
            line.chars().take(120).collect::<String>()
        })
        .collect();
    recent_topics.reverse();
    let body = if recent_topics.is_empty() {
        format!(
            "{} earlier messages ({} tool calls) were summarised to save context.",
            dropped.len(),
            tool_calls
        )
    } else {
        format!(
            "{} earlier messages ({} tool calls) were summarised to save context. Recent points:\n- {}",
            dropped.len(),
            tool_calls,
            recent_topics.join("\n- ")
        )
    };
    ModelMessage {
        role: ModelMessageRole::System,
        content: ModelMessageContent::text(format!("[CONTEXT SUMMARY]\n{body}")),
        tool_call_id: None,
        name: None,
        tool_calls: None,
    }
}

/// Compact a full history down to fit `budget_tokens`, prepending a summary of what was dropped.
/// Exposed for the runtime's `CompactSession` command.
pub fn compact_history(history: &[ModelMessage], budget_tokens: usize) -> Vec<ModelMessage> {
    let (kept, dropped) = fit_history(history, budget_tokens);
    if dropped == 0 {
        return kept;
    }
    let mut out = Vec::with_capacity(kept.len() + 1);
    out.push(compaction_summary(&history[..dropped]));
    out.extend(kept);
    out
}

impl ContextBuilder {
    pub fn build(
        &self,
        model: ModelId,
        prompt: &str,
        attachments: &[ContextAttachment],
        history: &[ModelMessage],
        tools: Vec<ToolSpec>,
    ) -> Result<BuiltContext, AgentError> {
        self.build_with_dynamic(model, prompt, attachments, history, tools, None)
    }

    /// Build the model request, enforcing [`ContextBudget`]. `dynamic_prefix` is an optional block
    /// (workspace tree, AGENTS.md, mode) prepended to the system prompt.
    pub fn build_with_dynamic(
        &self,
        model: ModelId,
        prompt: &str,
        attachments: &[ContextAttachment],
        history: &[ModelMessage],
        tools: Vec<ToolSpec>,
        dynamic_prefix: Option<String>,
    ) -> Result<BuiltContext, AgentError> {
        let packet = ContextPacket::from_history(prompt, history);
        self.build_packet_with_dynamic(model, prompt, attachments, packet, tools, dynamic_prefix)
    }

    pub fn build_packet_with_dynamic(
        &self,
        model: ModelId,
        prompt: &str,
        attachments: &[ContextAttachment],
        packet: ContextPacket,
        tools: Vec<ToolSpec>,
        dynamic_prefix: Option<String>,
    ) -> Result<BuiltContext, AgentError> {
        let mut system_prompt = super::prompt::system_prompt_with_tools(&tools);
        if let Some(prefix) = dynamic_prefix {
            system_prompt = format!("{prefix}\n{system_prompt}");
        }
        let system_msg = ModelMessage {
            role: ModelMessageRole::System,
            content: ModelMessageContent::text(system_prompt),
            tool_call_id: None,
            name: None,
            tool_calls: None,
        };

        let mut user_content = format!("[USER_REQUEST]\n{prompt}");
        let mut files = Vec::new();
        let mut parts = Vec::new();
        let mut image_count = 0usize;
        let mut total_image_bytes = 0u64;
        for attachment in attachments {
            if let Some(path) = attachment.path() {
                files.push(path.clone());
            }
            match attachment.kind {
                AttachmentKind::Image => {
                    let image =
                        build_image_part(attachment, &mut image_count, &mut total_image_bytes)?;
                    user_content.push_str(&format!(
                        "\n\n[IMAGE_ATTACHMENT: {}]",
                        attachment.display_label()
                    ));
                    parts.push(image);
                }
                _ => {
                    user_content
                        .push_str(&format!("\n\n[ATTACHMENT: {}]", attachment.display_label()));
                }
            }
        }
        let user_content = if parts.is_empty() {
            ModelMessageContent::text(user_content)
        } else {
            let mut content_parts = Vec::with_capacity(parts.len() + 1);
            content_parts.push(ModelContentPart::Text { text: user_content });
            content_parts.extend(parts);
            ModelMessageContent::Parts(content_parts)
        };
        let user_msg = ModelMessage {
            role: ModelMessageRole::User,
            content: user_content,
            tool_call_id: None,
            name: None,
            tool_calls: None,
        };

        let mut packet_context = packet
            .state
            .to_context_block(packet.task_class, packet.tool_pack);
        if !packet.relevant_context.is_empty() {
            packet_context.push_str("\n[RELEVANT_CONTEXT]\n");
            for block in &packet.relevant_context {
                packet_context.push_str(block.trim());
                packet_context.push('\n');
            }
        }
        let packet_msg = ModelMessage {
            role: ModelMessageRole::System,
            content: ModelMessageContent::text(packet_context),
            tool_call_id: None,
            name: None,
            tool_calls: None,
        };

        // Cap any oversized individual message first. Tool output messages should already be
        // summaries, but this remains a hard stop for older hydrated histories.
        let capped: Vec<ModelMessage> = packet
            .recent_turns
            .iter()
            .map(|m| cap_message_content(m, self.budget.max_file_tokens))
            .collect();

        // Reserve room for system + user + response + tools; the rest is the history budget.
        let fixed =
            message_tokens(&system_msg) + message_tokens(&user_msg) + message_tokens(&packet_msg);
        let tools_cost = tools_tokens(&tools).min(self.budget.reserved_for_tools);
        let available = self
            .budget
            .max_tokens
            .saturating_sub(self.budget.reserved_for_response)
            .saturating_sub(self.budget.reserved_for_tools)
            .saturating_sub(fixed);
        let profile_history_cap = match packet.budget_profile {
            ContextBudgetProfile::SmallTask => 3_000,
            ContextBudgetProfile::Normal => self.budget.max_history_tokens,
            ContextBudgetProfile::Deep => self.budget.max_history_tokens.saturating_mul(2),
        };
        let history_budget = available
            .min(self.budget.max_history_tokens)
            .min(profile_history_cap);

        let system_tokens = message_tokens(&system_msg);
        let user_tokens = message_tokens(&user_msg);
        let packet_tokens = message_tokens(&packet_msg);

        let (kept, dropped) = fit_history(&capped, history_budget);

        let mut messages = vec![system_msg, user_msg, packet_msg];
        let mut summaries = Vec::new();
        if dropped > 0 {
            messages.push(compaction_summary(&capped[..dropped]));
            summaries.push(format!("compacted {dropped} earlier messages"));
        }
        messages.extend(kept);

        let history_tokens: usize = messages.iter().skip(3).map(message_tokens).sum();
        let token_estimate = fixed + tools_cost + history_tokens;
        summaries.push(format!("{} messages in context", messages.len()));
        summaries.push(format!("task class: {:?}", packet.task_class));
        summaries.push(format!("tool pack: {:?}", packet.tool_pack));

        let section_estimates = vec![
            ContextSectionEstimate {
                name: "system".into(),
                tokens: system_tokens,
            },
            ContextSectionEstimate {
                name: "user_request".into(),
                tokens: user_tokens,
            },
            ContextSectionEstimate {
                name: "task_state_and_context".into(),
                tokens: packet_tokens,
            },
            ContextSectionEstimate {
                name: "recent_turns".into(),
                tokens: history_tokens,
            },
            ContextSectionEstimate {
                name: "tool_specs".into(),
                tokens: tools_cost,
            },
        ];

        Ok(BuiltContext {
            request: ModelRequest {
                model,
                messages,
                tools,
                temperature: Some(0.2),
                max_tokens: Some(4096),
            },
            token_estimate,
            files,
            summaries,
            section_estimates,
        })
    }
}

fn build_image_part(
    attachment: &ContextAttachment,
    image_count: &mut usize,
    total_image_bytes: &mut u64,
) -> Result<ModelContentPart, AgentError> {
    *image_count += 1;
    if *image_count > MAX_IMAGE_ATTACHMENTS {
        return Err(AgentError::Other(format!(
            "too many image attachments: max {MAX_IMAGE_ATTACHMENTS}"
        )));
    }

    let mime_type = attachment
        .mime_type
        .as_deref()
        .ok_or_else(|| AgentError::Other("image attachment is missing a MIME type".into()))?;
    if !SUPPORTED_IMAGE_MIME_TYPES.contains(&mime_type) {
        return Err(AgentError::Other(format!(
            "unsupported image MIME type: {mime_type}"
        )));
    }

    let bytes = match &attachment.source {
        AttachmentSource::Path(path) => {
            let metadata = std::fs::metadata(path).map_err(|err| {
                AgentError::Other(format!(
                    "could not read image metadata {}: {err}",
                    path.display()
                ))
            })?;
            if metadata.len() > MAX_IMAGE_BYTES {
                return Err(AgentError::Other(format!(
                    "image attachment is too large: {} exceeds {} bytes",
                    path.display(),
                    MAX_IMAGE_BYTES
                )));
            }
            std::fs::read(path).map_err(|err| {
                AgentError::Other(format!(
                    "could not read image attachment {}: {err}",
                    path.display()
                ))
            })?
        }
        AttachmentSource::Bytes(bytes) => {
            if bytes.len() as u64 > MAX_IMAGE_BYTES {
                return Err(AgentError::Other(format!(
                    "image attachment is too large: {} exceeds {} bytes",
                    attachment.display_label(),
                    MAX_IMAGE_BYTES
                )));
            }
            bytes.clone()
        }
    };

    *total_image_bytes += bytes.len() as u64;
    if *total_image_bytes > MAX_TOTAL_IMAGE_BYTES {
        return Err(AgentError::Other(format!(
            "image attachments are too large: total exceeds {MAX_TOTAL_IMAGE_BYTES} bytes"
        )));
    }

    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(ModelContentPart::ImageUrl {
        url: format!("data:{mime_type};base64,{encoded}"),
        mime_type: mime_type.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_protocol::{AttachmentKind, AttachmentSource};

    fn image_attachment(mime_type: &str, bytes: Vec<u8>) -> ContextAttachment {
        let size_bytes = bytes.len() as u64;
        ContextAttachment {
            source: AttachmentSource::Bytes(bytes),
            kind: AttachmentKind::Image,
            mime_type: Some(mime_type.into()),
            display_name: Some("image.png".into()),
            size_bytes: Some(size_bytes),
        }
    }

    #[test]
    fn build_adds_image_parts_after_text() {
        let attachment = image_attachment("image/png", vec![1, 2, 3]);
        let built = ContextBuilder::default()
            .build(
                ModelId::new("test"),
                "describe it",
                &[attachment],
                &[],
                Vec::new(),
            )
            .expect("context build");

        let user = &built.request.messages[1];
        let ModelMessageContent::Parts(parts) = &user.content else {
            panic!("expected multipart user content");
        };
        assert!(matches!(parts[0], ModelContentPart::Text { .. }));
        let ModelContentPart::ImageUrl { url, mime_type } = &parts[1] else {
            panic!("expected image part");
        };
        assert_eq!(mime_type, "image/png");
        assert!(url.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn build_rejects_unsupported_image_mime_type() {
        let attachment = image_attachment("image/svg+xml", vec![1, 2, 3]);
        let err = match ContextBuilder::default().build(
            ModelId::new("test"),
            "describe it",
            &[attachment],
            &[],
            Vec::new(),
        ) {
            Ok(_) => panic!("unsupported image should fail"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("unsupported image MIME type"));
    }

    #[test]
    fn build_rejects_too_many_images() {
        let attachments = (0..(MAX_IMAGE_ATTACHMENTS + 1))
            .map(|_| image_attachment("image/png", vec![1, 2, 3]))
            .collect::<Vec<_>>();
        let err = match ContextBuilder::default().build(
            ModelId::new("test"),
            "describe it",
            &attachments,
            &[],
            Vec::new(),
        ) {
            Ok(_) => panic!("too many images should fail"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("too many image attachments"));
    }

    #[test]
    fn build_rejects_oversized_image() {
        let attachment = image_attachment("image/png", vec![0; (MAX_IMAGE_BYTES + 1) as usize]);
        let err = match ContextBuilder::default().build(
            ModelId::new("test"),
            "describe it",
            &[attachment],
            &[],
            Vec::new(),
        ) {
            Ok(_) => panic!("oversized image should fail"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("image attachment is too large"));
    }

    #[test]
    fn build_rejects_total_image_bytes_over_limit() {
        let attachments = vec![
            image_attachment("image/png", vec![0; MAX_IMAGE_BYTES as usize]),
            image_attachment("image/png", vec![0; MAX_IMAGE_BYTES as usize]),
            image_attachment("image/png", vec![0; 1]),
        ];
        let err = match ContextBuilder::default().build(
            ModelId::new("test"),
            "describe it",
            &attachments,
            &[],
            Vec::new(),
        ) {
            Ok(_) => panic!("total oversized images should fail"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("image attachments are too large"));
    }

    #[test]
    fn message_tokens_include_image_url_size() {
        let message = ModelMessage {
            role: ModelMessageRole::User,
            content: ModelMessageContent::Parts(vec![
                ModelContentPart::Text { text: "hi".into() },
                ModelContentPart::ImageUrl {
                    url: "data:image/png;base64,abcdabcd".into(),
                    mime_type: "image/png".into(),
                },
            ]),
            tool_call_id: None,
            name: None,
            tool_calls: None,
        };

        assert!(message_tokens(&message) > estimate_tokens("hi") + 4);
    }

    #[test]
    fn classifies_dependency_update_and_selects_small_budget() {
        let task = classify_task("Update Android Gradle dependencies and libs.versions.toml");
        assert_eq!(task, TaskClass::DependencyUpdate);
        assert_eq!(
            budget_profile_for_task(task),
            ContextBudgetProfile::SmallTask
        );
        assert_eq!(tool_pack_for_task(task), ToolPack::Dependency);
    }

    #[test]
    fn packet_build_preserves_task_state_before_history() {
        let mut state = ModelContextState::new("fix failing tests");
        state
            .decisions
            .push("Use the narrowest failing test first".into());
        state.touched_files.push(PathBuf::from("src/lib.rs"));
        let packet = ContextPacket {
            state,
            task_class: TaskClass::TestFailure,
            budget_profile: ContextBudgetProfile::Normal,
            tool_pack: ToolPack::GitCi,
            relevant_context: vec![
                "<file_slice path=\"src/lib.rs\">fn main(){}</file_slice>".into(),
            ],
            recent_turns: vec![ModelMessage {
                role: ModelMessageRole::Tool,
                content: "raw output that should be separately budgeted".into(),
                tool_call_id: None,
                name: None,
                tool_calls: None,
            }],
        };
        let built = ContextBuilder::default()
            .build_packet_with_dynamic(
                ModelId::new("test"),
                "fix failing tests",
                &[],
                packet,
                Vec::new(),
                None,
            )
            .expect("context build");
        let task_msg = built.request.messages[2].content.to_text_lossy();
        assert!(task_msg.contains("[TASK_STATE]"));
        assert!(task_msg.contains("src/lib.rs"));
        assert!(task_msg.contains("[RELEVANT_CONTEXT]"));
        assert!(
            built
                .section_estimates
                .iter()
                .any(|section| section.name == "task_state_and_context")
        );
    }

    #[test]
    fn task_state_renders_android_evidence_targets() {
        let mut state = ModelContextState::new("tap the settings button");
        state.record_tool_summary(ToolResultSummary {
            call_id: agent_protocol::ToolCallId::new("call-1"),
            tool: "android.observe".into(),
            summary: "Android · observed MainActivity · 1 targets".into(),
            facts: Vec::new(),
            affected_paths: Vec::new(),
            ranges: Vec::new(),
            raw_handle: "tool://run/call-1".into(),
            token_cost: 10,
            truncated: false,
            next_actions: Vec::new(),
            is_error: false,
            android_evidence: Some(agent_protocol::AndroidToolEvidence {
                observation_id: "obs-1".into(),
                package: Some("com.example".into()),
                activity: Some("MainActivity".into()),
                visible_targets: vec![agent_protocol::AndroidVisibleTargetEvidence {
                    id: "rid:com.example:id/settings".into(),
                    label: "Settings".into(),
                    text: Some("Settings".into()),
                    resource_id: Some("com.example:id/settings".into()),
                    content_desc: None,
                    clickable: true,
                    enabled: true,
                    visible: true,
                }],
                action: None,
            }),
        });
        let block = state.to_context_block(TaskClass::UiChange, ToolPack::UiBrowser);
        assert!(block.contains("android_observation: obs-1"));
        assert!(block.contains("android_target: Settings"));
        assert!(block.contains("resource_id=com.example:id/settings"));
    }
}
