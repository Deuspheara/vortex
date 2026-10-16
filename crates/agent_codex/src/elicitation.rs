//! Confirmation-only MCP forms. Never invent answers to text, credential, or URL flows.
use serde_json::{Map, Value, json};

pub fn permission(params: &Value) -> Option<(String, Value)> {
    if !matches!(
        params["mode"].as_str()?,
        "form" | "openai/form" | "openaiForm"
    ) {
        return None;
    }
    let server = params["serverName"]
        .as_str()
        .filter(|s| !s.trim().is_empty())?;
    let message = params["message"]
        .as_str()
        .filter(|s| !s.trim().is_empty())?;
    let schema = params["requestedSchema"].as_object()?;
    // Rich forms may carry conditions or require authenticated proofs. Only plain
    // permission forms can be represented faithfully by Allow / Deny.
    if schema.get("type")? != "object"
        || schema.keys().any(|key| {
            !matches!(
                key.as_str(),
                "type" | "properties" | "required" | "$schema" | "additionalProperties"
            )
        })
    {
        return None;
    }
    let properties = schema.get("properties")?.as_object()?;
    if let Some(required) = schema.get("required").filter(|v| !v.is_null()) {
        for key in required.as_array()? {
            if !properties.contains_key(key.as_str()?) {
                return None;
            }
        }
    }
    let mut content = Map::new();
    let mut description = format!("{server} requests permission\n\n{message}");
    for (name, field) in properties {
        let field = field.as_object()?;
        if field.get("type")? != "boolean"
            || field
                .keys()
                .any(|key| !matches!(key.as_str(), "type" | "title" | "description" | "default"))
        {
            return None;
        }
        let label = field.get("title").and_then(Value::as_str).unwrap_or(name);
        description.push_str(&format!("\n\n• {label}"));
        if let Some(detail) = field.get("description").and_then(Value::as_str) {
            description.push_str(&format!(" — {detail}"));
        }
        content.insert(name.clone(), json!(true));
    }
    Some((description, Value::Object(content)))
}

pub fn response(content: &Value, accept: bool) -> Value {
    if accept {
        json!({"action":"accept", "content":content})
    } else {
        json!({"action":"decline"})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_schemas_never_synthesize_answers() {
        let base = json!({"mode":"form", "serverName":"computer-use", "message":"Share screen?",
            "requestedSchema":{"type":"object","properties":{}}});
        assert_eq!(permission(&base).unwrap().1, json!({}));
        for schema in [
            json!({"type":"object","properties":{"token":{"type":"string"}}}),
            json!({"type":"object","properties":{},"required":["missing"]}),
            json!({"type":"object","properties":{},"required":"malformed"}),
            json!({"type":"object","properties":{},"oneOf":[{}]}),
            json!({"type":"object","properties":{"screen":{"type":"boolean","const":false}}}),
            json!({"type":"object"}),
        ] {
            let mut request = base.clone();
            request["requestedSchema"] = schema;
            assert!(permission(&request).is_none(), "{request}");
        }
        for mode in ["url", "openai/userVerification", "unknown"] {
            let mut request = base.clone();
            request["mode"] = json!(mode);
            assert!(permission(&request).is_none());
        }
    }
}
