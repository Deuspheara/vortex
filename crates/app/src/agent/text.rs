//! Sanitize provider artifacts from streamed assistant text.

use agent_models::strip_inline_tool_blocks;

use crate::features::shell::state::ThreadItem;

const STRIP_TAGS: &[&str] = &[
    "</assistant>",
    "<assistant>",
    "</thinking>",
    "<thinking>",
    "</task>",
    "<task>",
    "</proposed_plan>",
    "<proposed_plan>",
    "</approved_plan>",
    "<approved_plan>",
];

/// Remove common XML-style role tags from a streaming delta.
pub fn sanitize_assistant_delta(delta: &str) -> String {
    let mut out = delta.to_string();
    for tag in STRIP_TAGS {
        out = out.replace(tag, "");
    }
    strip_trailing_partial_tag(&out)
}

/// Drop a trailing incomplete tag opener split across streaming deltas (e.g. `"<task"` + `"<task"`).
fn strip_trailing_partial_tag(s: &str) -> String {
    let Some(idx) = s.rfind('<') else {
        return s.to_string();
    };
    let tail = &s[idx..];
    if tail.contains('>') {
        return s.to_string();
    }
    let rest = &tail[1..];
    if rest.is_empty()
        || rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '_')
    {
        return s[..idx].to_string();
    }
    s.to_string()
}

/// Normalize accumulated assistant markdown before display / height estimates.
pub fn sanitize_assistant_text(text: &str) -> String {
    let mut out = sanitize_assistant_delta(text);
    out = strip_inline_tool_blocks(&out);
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    out.trim().to_string()
}

/// Sidebar / session title from the user's first message.
pub fn title_from_prompt(prompt: &str) -> String {
    let line = prompt.lines().next().unwrap_or(prompt).trim();
    if line.is_empty() {
        return "New conversation".into();
    }
    const MAX: usize = 48;
    let count = line.chars().count();
    if count <= MAX {
        line.to_string()
    } else {
        format!("{}…", line.chars().take(MAX).collect::<String>())
    }
}

pub fn is_default_conversation_title(title: &str) -> bool {
    title == "New Conversation" || title.starts_with("New Conversation ")
}

const MAX_CONVERSATION_CONTEXT_CHARS: usize = 12_000;

/// Prior thread turns for multi-message sessions (excludes the message being sent now).
pub fn conversation_context_from_thread(items: &[ThreadItem]) -> String {
    if items.len() <= 1 {
        return String::new();
    }
    let prior = match items.last() {
        Some(ThreadItem::UserMessage { .. }) => &items[..items.len() - 1],
        _ => items,
    };
    if prior.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();
    for item in prior {
        match item {
            ThreadItem::UserMessage {
                text, attachments, ..
            } => {
                if attachments.is_empty() {
                    lines.push(format!("User: {text}"));
                } else {
                    let labels = attachments
                        .iter()
                        .map(|attachment| attachment.label.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    lines.push(format!("User: {text}\nAttachments: {labels}"));
                }
            }
            ThreadItem::AssistantMessage { markdown, .. } => {
                let text = sanitize_assistant_text(markdown);
                if !text.is_empty() {
                    lines.push(format!("Assistant: {text}"));
                }
            }
            ThreadItem::ToolCall {
                tool_name, output, ..
            } => {
                if let Some(out) = output {
                    let preview = truncate_for_context(out, 500);
                    lines.push(format!("Tool {tool_name}: {preview}"));
                }
            }
            ThreadItem::RunError { message, .. } => {
                if !is_non_contextual_run_error(message) {
                    lines.push(format!("Run error: {message}"));
                }
            }
            _ => {}
        }
    }

    let mut out = lines.join("\n\n");
    if out.len() > MAX_CONVERSATION_CONTEXT_CHARS {
        let start = out.len().saturating_sub(MAX_CONVERSATION_CONTEXT_CHARS);
        out = out[start..].to_string();
    }
    out
}

pub fn prompt_with_conversation_context(context: &str, prompt: &str) -> String {
    if context.trim().is_empty() {
        prompt.to_string()
    } else {
        format!("[CONVERSATION_HISTORY]\n{context}\n\n[CURRENT_REQUEST]\n{prompt}")
    }
}

fn is_non_contextual_run_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("openrouter")
        && (lower.contains("http 401")
            || lower.contains("http 403")
            || lower.contains("forbidden")
            || lower.contains("api key")
            || lower.contains("authentication failed"))
}

fn truncate_for_context(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(max_chars).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_task_tags_and_partial_opener() {
        assert_eq!(sanitize_assistant_delta("<task>hello</task>"), "hello");
        assert_eq!(sanitize_assistant_delta("before<task"), "before");
        assert_eq!(sanitize_assistant_delta("<task"), "");
        assert_eq!(sanitize_assistant_delta("split<task"), "split");
    }

    #[test]
    fn strips_inline_tool_call_from_assistant_text() {
        let raw = r#"Done.
<|tool_call|>call:edit_file{path:<|"|>src/lib.rs<|"|>,old_string:<|"|>a<|"|>,new_string:<|"|>b<|"|>}"#;
        let out = sanitize_assistant_text(raw);
        assert!(out.contains("Done."));
        assert!(!out.contains("<|tool_call|>"));
        assert!(!out.contains("call:edit_file"));
    }

    #[test]
    fn strips_plan_wrapper_tags_from_assistant_text() {
        let out = sanitize_assistant_text("<proposed_plan>\n# Plan\n- Step\n</proposed_plan>");
        assert_eq!(out, "# Plan\n- Step");
    }
}
