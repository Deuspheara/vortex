//! Parse Hermes-style inline tool calls embedded in model text streams.
//!
//! Some providers (e.g. openrouter/free) emit tool syntax in SSE `content` instead of native
//! `tool_calls`. Markers: `<|tool_call|>`, `<tool_call>`, and `call:tool_name{...}` with
//! `<|"|>` string delimiters.

use std::collections::HashSet;

use serde_json::{Value, json};

const TOOL_CALL_MARKERS: &[&str] = &["<|tool_call|>", "<tool_call>"];
const HERMES_STRING: &str = "<|\"|>";

#[derive(Clone, Debug, PartialEq)]
pub struct InlineToolCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Default)]
pub struct InlineToolCallParser {
    buffer: String,
    next_id: u32,
    valid_tools: HashSet<String>,
}

#[derive(Debug)]
pub struct InlineParseOutput {
    pub clean_text: String,
    pub calls: Vec<InlineToolCall>,
}

impl InlineToolCallParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tool_names<I, S>(tool_names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            buffer: String::new(),
            next_id: 0,
            valid_tools: tool_names.into_iter().map(Into::into).collect(),
        }
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
    }

    /// Append a streaming chunk; returns clean text and any newly completed tool calls.
    pub fn push(&mut self, chunk: &str) -> InlineParseOutput {
        self.buffer.push_str(chunk);
        self.drain_complete()
    }

    fn drain_complete(&mut self) -> InlineParseOutput {
        let mut clean = String::new();
        let mut calls = Vec::new();

        loop {
            let Some(marker_pos) = find_next_marker(&self.buffer, &self.valid_tools) else {
                let (emit, keep) = split_trailing_partial_marker(&self.buffer);
                clean.push_str(&emit);
                self.buffer = keep;
                break;
            };

            clean.push_str(&self.buffer[..marker_pos]);
            let rest = self.buffer[marker_pos..].to_string();
            match try_parse_call(&rest, &self.valid_tools) {
                Some((call, consumed)) => {
                    calls.push(call);
                    self.buffer = rest[consumed..].to_string();
                }
                None => {
                    self.buffer = rest;
                    break;
                }
            }
        }

        InlineParseOutput {
            clean_text: clean,
            calls,
        }
    }

    pub fn next_tool_id(&mut self) -> String {
        let id = format!("inline-{}", self.next_id);
        self.next_id += 1;
        id
    }
}

/// One-shot extraction from accumulated text (runtime backstop).
pub fn extract_inline_tool_calls(text: &str) -> (String, Vec<InlineToolCall>) {
    let mut parser = InlineToolCallParser::new();
    let out = parser.push(text);
    let mut clean = out.clean_text;
    if !parser.buffer.is_empty() {
        if let Some(pos) = find_next_marker(&parser.buffer, &parser.valid_tools) {
            clean.push_str(&parser.buffer[..pos]);
        }
    }
    (clean, out.calls)
}

pub fn extract_inline_tool_calls_with_tools<I, S>(
    text: &str,
    tool_names: I,
) -> (String, Vec<InlineToolCall>)
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut parser = InlineToolCallParser::with_tool_names(tool_names);
    let out = parser.push(text);
    let mut clean = out.clean_text;
    if !parser.buffer.is_empty() {
        if let Some(pos) = find_next_marker(&parser.buffer, &parser.valid_tools) {
            clean.push_str(&parser.buffer[..pos]);
        }
    }
    (clean, out.calls)
}

/// Strip complete and trailing incomplete inline tool blocks from display text.
pub fn strip_inline_tool_blocks(text: &str) -> String {
    let mut parser = InlineToolCallParser::new();
    let out = parser.push(text);
    let mut clean = out.clean_text;
    if let Some(pos) = find_next_marker(&parser.buffer, &parser.valid_tools) {
        clean.push_str(&parser.buffer[..pos]);
    }
    clean
}

fn find_next_marker(s: &str, valid_tools: &HashSet<String>) -> Option<usize> {
    let mut best: Option<usize> = None;
    for marker in TOOL_CALL_MARKERS {
        if let Some(pos) = s.find(marker) {
            best = Some(best.map_or(pos, |b| b.min(pos)));
        }
    }
    if let Some(pos) = find_call_prefix(s, valid_tools) {
        best = Some(best.map_or(pos, |b| b.min(pos)));
    }
    best
}

fn find_call_prefix(s: &str, valid_tools: &HashSet<String>) -> Option<usize> {
    let mut search_from = 0;
    while let Some(rel) = s[search_from..].find("call:") {
        let pos = search_from + rel;
        let after = &s[pos + 5..];
        let _ = valid_tools;
        if parse_tool_name(after, &HashSet::new()).is_some() {
            return Some(pos);
        }
        search_from = pos + 5;
    }
    None
}

fn split_trailing_partial_marker(s: &str) -> (String, String) {
    let mut hold_from = s.len();
    for marker in TOOL_CALL_MARKERS {
        for i in 1..marker.len() {
            let prefix = &marker[..i];
            if s.ends_with(prefix) {
                hold_from = hold_from.min(s.len() - prefix.len());
            }
        }
    }
    if let Some(pos) = s.rfind('<') {
        let tail = &s[pos..];
        if !tail.contains('>') {
            let rest = &tail[1..];
            if rest.is_empty()
                || rest
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '|' || c == '_' || c == '/')
            {
                hold_from = hold_from.min(pos);
            }
        }
    }
    if let Some(pos) = s.rfind("call:") {
        let after = &s[pos + 5..];
        if parse_tool_name(after, &HashSet::new()).is_none() {
            if after.is_empty()
                || after
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
            {
                hold_from = hold_from.min(pos);
            }
        }
    }
    if hold_from < s.len() {
        (s[..hold_from].to_string(), s[hold_from..].to_string())
    } else {
        (s.to_string(), String::new())
    }
}

fn try_parse_call(s: &str, valid_tools: &HashSet<String>) -> Option<(InlineToolCall, usize)> {
    let mut idx = 0usize;
    let bytes = s.as_bytes();

    for marker in TOOL_CALL_MARKERS {
        if s[idx..].starts_with(marker) {
            idx += marker.len();
            break;
        }
    }

    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }

    if s[idx..].starts_with("call:") {
        idx += 5;
    }

    let name_start = idx;
    let _ = valid_tools;
    let (name, name_len) = parse_tool_name(
        std::str::from_utf8(&bytes[name_start..]).ok()?,
        &HashSet::new(),
    )?;
    idx = name_start + name_len;
    if name.is_empty() {
        return None;
    }

    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }

    if !s[idx..].starts_with('{') {
        return None;
    }

    let (body, body_len) = extract_braced_body(&s[idx..])?;
    idx += body_len;
    let arguments = parse_call_arguments(body)?;

    Some((
        InlineToolCall {
            name: name.to_string(),
            arguments,
        },
        idx,
    ))
}

fn parse_tool_name<'a>(s: &'a str, valid_tools: &HashSet<String>) -> Option<(&'a str, usize)> {
    let mut end = 0;
    for (i, c) in s.char_indices() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        return None;
    }
    let name = &s[..end];
    if valid_tools.is_empty() || valid_tools.contains(name) {
        Some((name, end))
    } else {
        None
    }
}

fn extract_braced_body(s: &str) -> Option<(&str, usize)> {
    if !s.starts_with('{') {
        return None;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if in_string {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&s[1..i], i + 1));
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_call_arguments(body: &str) -> Option<Value> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Some(json!({}));
    }
    let json_candidate = if trimmed.starts_with('{') {
        trimmed.to_string()
    } else {
        format!("{{{trimmed}}}")
    };
    if let Ok(v) = serde_json::from_str::<Value>(&json_candidate) {
        if v.is_object() {
            return Some(v);
        }
    }
    parse_hermes_args(trimmed).ok()
}

fn parse_hermes_args(body: &str) -> Result<Value, ()> {
    let mut map = serde_json::Map::new();
    let mut i = 0;
    let chars: Vec<char> = body.chars().collect();
    while i < chars.len() {
        while i < chars.len() && (chars[i] == ',' || chars[i].is_whitespace()) {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        let key_start = i;
        while i < chars.len() && chars[i] != ':' {
            i += 1;
        }
        if i >= chars.len() {
            return Err(());
        }
        let key: String = chars[key_start..i]
            .iter()
            .collect::<String>()
            .trim()
            .to_string();
        i += 1;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let remaining: String = chars[i..].iter().collect();
        if remaining.starts_with(HERMES_STRING) {
            let val_start = i + HERMES_STRING.chars().count();
            let tail: String = chars[val_start..].iter().collect();
            if let Some(end_rel) = tail.find(HERMES_STRING) {
                let end = val_start + end_rel;
                let val: String = chars[val_start..end].iter().collect();
                i = end + HERMES_STRING.chars().count();
                map.insert(key, Value::String(val));
            } else {
                return Err(());
            }
        } else {
            let val_start = i;
            while i < chars.len() && chars[i] != ',' {
                i += 1;
            }
            let val: String = chars[val_start..i]
                .iter()
                .collect::<String>()
                .trim()
                .to_string();
            map.insert(key, parse_scalar(&val));
        }
    }
    Ok(Value::Object(map))
}

fn parse_scalar(s: &str) -> Value {
    if s == "true" {
        Value::Bool(true)
    } else if s == "false" {
        Value::Bool(false)
    } else if s == "null" {
        Value::Null
    } else if let Ok(n) = s.parse::<i64>() {
        Value::Number(n.into())
    } else if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))
    {
        Value::String(s[1..s.len() - 1].to_string())
    } else {
        Value::String(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREENSHOT_SAMPLE: &str = r#"I'll update the file now.
<|tool_call|>call:edit_file{path:<|"|>crates/app/src/agent/text.rs<|"|>,old_string:<|"|>    pub fn sanitize_assistant_text(text: &str) -> String {
    let mut out = sanitize_assistant_delta(text);
<|"|>,new_string:<|"|>    pub fn sanitize_assistant_text(text: &str) -> String {
    let mut out = sanitize_assistant_delta(text);
    out = strip_inline_tool_blocks(&out);
<|"|>}"#;

    #[test]
    fn inline_tool_parses_edit_file_screenshot_pattern() {
        let mut parser = InlineToolCallParser::with_tool_names(["edit_file"]);
        let out = parser.push(SCREENSHOT_SAMPLE);
        assert!(
            out.clean_text.contains("I'll update the file now."),
            "clean text: {:?}",
            out.clean_text
        );
        assert!(!out.clean_text.contains("<|tool_call|>"));
        assert_eq!(out.calls.len(), 1);
        let call = &out.calls[0];
        assert_eq!(call.name, "edit_file");
        assert_eq!(
            call.arguments.get("path").and_then(|v| v.as_str()),
            Some("crates/app/src/agent/text.rs")
        );
        assert!(call.arguments.get("old_string").is_some());
        assert!(call.arguments.get("new_string").is_some());
    }

    #[test]
    fn inline_tool_buffers_incomplete_call_across_chunks() {
        let mut parser = InlineToolCallParser::with_tool_names(["read_file"]);
        let sample = "<|tool_call|>call:read_file{path:<|\"|>README.md<|\"|>}";
        let mid = sample.len() / 2;
        let a = parser.push(&sample[..mid]);
        assert!(a.calls.is_empty());
        assert!(parser.buffer.len() > 0);
        let b = parser.push(&sample[mid..]);
        assert_eq!(b.calls.len(), 1);
        assert_eq!(b.calls[0].name, "read_file");
    }

    #[test]
    fn strip_inline_tool_blocks_removes_complete_call() {
        let stripped = strip_inline_tool_blocks(SCREENSHOT_SAMPLE);
        assert!(!stripped.contains("<|tool_call|>"));
        assert!(!stripped.contains("call:edit_file"));
        assert!(stripped.contains("I'll update the file now."));
    }

    #[test]
    fn parse_json_style_call() {
        let text = r#"call:read_file{"path":"src/lib.rs"}"#;
        let (_, calls) = extract_inline_tool_calls_with_tools(text, ["read_file"]);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].arguments["path"], "src/lib.rs");
    }

    #[test]
    fn parse_dotted_tool_name() {
        let text = r#"call:android.observe{}"#;
        let (_, calls) = extract_inline_tool_calls_with_tools(text, ["android.observe"]);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "android.observe");
    }

    #[test]
    fn unknown_dotted_tool_is_extracted_for_runtime_rejection() {
        let text = r#"call:android.unknown{}"#;
        let (clean, calls) = extract_inline_tool_calls_with_tools(text, ["android.observe"]);
        assert!(clean.is_empty());
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "android.unknown");
    }
}
