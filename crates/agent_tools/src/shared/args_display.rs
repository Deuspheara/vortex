//! Human-readable tool argument previews — never leak raw JSON into the UI.

/// True when `s` looks like JSON (complete or streaming fragment), not a plain label.
pub fn looks_like_json_fragment(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    if t.starts_with('{') || t.starts_with('[') {
        return true;
    }
    // Streaming fragments often start mid-object.
    if t.starts_with('"') || t.contains("\":") || t.contains("\": \"") {
        return true;
    }
    false
}

/// Extract a string field from incomplete/streaming JSON tool arguments.
pub fn partial_json_string_field(input: &str, key: &str) -> Option<String> {
    let needle = format!(r#""{key}""#);
    let start = input.find(&needle)?;
    let mut after = input[start + needle.len()..].trim_start();
    if let Some(rest) = after.strip_prefix(':') {
        after = rest.trim_start();
    } else {
        return None;
    }
    if !after.starts_with('"') {
        return None;
    }
    let mut value = String::new();
    let mut chars = after[1..].chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                value.push(next);
            }
        } else if c == '"' {
            break;
        } else {
            value.push(c);
        }
    }
    if value.is_empty() { None } else { Some(value) }
}

pub fn partial_json_bool_field(input: &str, key: &str) -> Option<bool> {
    let needle = format!(r#""{key}""#);
    let start = input.find(&needle)?;
    let mut after = input[start + needle.len()..].trim_start();
    if let Some(rest) = after.strip_prefix(':') {
        after = rest.trim_start();
    } else {
        return None;
    }
    if let Some(rest) = after.strip_prefix("true") {
        if rest
            .chars()
            .next()
            .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_')
        {
            return Some(true);
        }
    }
    if let Some(rest) = after.strip_prefix("false") {
        if rest
            .chars()
            .next()
            .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_')
        {
            return Some(false);
        }
    }
    None
}

pub fn partial_args_from_schema(
    schema: &serde_json::Value,
    input: &str,
) -> Option<serde_json::Value> {
    let properties = schema.get("properties")?.as_object()?;
    let mut out = serde_json::Map::new();
    for (key, property) in properties {
        match property.get("type").and_then(|v| v.as_str()) {
            Some("string") => {
                if let Some(value) = partial_json_string_field(input, key) {
                    out.insert(key.clone(), serde_json::Value::String(value));
                }
            }
            Some("boolean") => {
                if let Some(value) = partial_json_bool_field(input, key) {
                    out.insert(key.clone(), serde_json::Value::Bool(value));
                }
            }
            _ => {}
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(out))
    }
}

/// Strip values that must not appear as thread row detail text.
pub fn sanitize_display_arg(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() || t == "{}" || t.contains("<|") || looks_like_json_fragment(t) {
        return None;
    }
    Some(if t.len() > 160 {
        format!("{}…", &t[..157])
    } else {
        t.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_json_fragments() {
        assert!(looks_like_json_fragment(r#"{"path":"a.html"#));
        assert!(!looks_like_json_fragment("index.html"));
    }

    #[test]
    fn partial_path_from_streaming_write() {
        let json = r#"{"path":"todo/index.html","content":"<!DOCTYPE"#;
        assert_eq!(
            partial_json_string_field(json, "path").as_deref(),
            Some("todo/index.html")
        );
    }

    #[test]
    fn partial_args_from_schema_extracts_string_fields() {
        let json = r#"{"path":"app.js","content":"console""#;
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "content": {"type": "string"}
            }
        });
        assert_eq!(
            partial_args_from_schema(&schema, json)
                .and_then(|value| value
                    .get("path")
                    .and_then(|v| v.as_str())
                    .map(str::to_string))
                .as_deref(),
            Some("app.js")
        );
    }

    #[test]
    fn sanitize_rejects_json() {
        assert!(sanitize_display_arg(r#"{"path":"x"}"#).is_none());
        assert_eq!(
            sanitize_display_arg("src/main.rs").as_deref(),
            Some("src/main.rs")
        );
    }

    #[test]
    fn partial_args_from_schema_extracts_bool_fields() {
        let json = r#"{"query":"foo","regex":true"#;
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"},
                "regex": {"type": "boolean"}
            }
        });
        let partial = partial_args_from_schema(&schema, json).expect("partial args");
        assert_eq!(partial.get("query").and_then(|v| v.as_str()), Some("foo"));
        assert_eq!(partial.get("regex").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn open_node_partial_arg_remains_sanitizable() {
        assert!(sanitize_display_arg("open_node<|channel|>analysis").is_none());
    }
}
