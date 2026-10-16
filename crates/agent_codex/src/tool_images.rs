//! Extract bounded inline MCP images without opening URLs or reading host files.
use agent_protocol::{AgentEvent, RunId, ToolCallId};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;

const MAX_ENCODED_IMAGE: usize = 14 * 1024 * 1024;

pub fn extract(
    result: &mut Value,
    run_id: &RunId,
    call_id: &ToolCallId,
    source: &str,
) -> Option<AgentEvent> {
    let mut latest = None;
    visit(result, 0, &mut latest);
    latest.map(|(mime_type, data_base64)| AgentEvent::ToolImageObserved {
        run_id: run_id.clone(),
        call_id: call_id.clone(),
        source: source.into(),
        mime_type,
        data_base64,
    })
}

fn visit(value: &mut Value, depth: usize, latest: &mut Option<(String, String)>) {
    if depth > 8 {
        return;
    }
    if value["type"] == "image" {
        let mut available = false;
        if let (Some(mime), Some(data)) = (value["mimeType"].as_str(), value["data"].as_str())
            && data.len() <= MAX_ENCODED_IMAGE
            && matches!(mime, "image/png" | "image/jpeg" | "image/webp")
            && let Ok(bytes) = STANDARD.decode(data)
        {
            let valid = match mime {
                "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
                "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
                "image/webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
                _ => false,
            };
            if valid {
                *latest = Some((mime.into(), data.into()));
                available = true;
            }
        }
        // Never dump image payloads into the text transcript.
        value["data"] = if available {
            "[Image available in Screen preview]"
        } else {
            "[Image preview unavailable]"
        }
        .into();
        return;
    }
    if let Some(content) = value.get_mut("content").and_then(Value::as_array_mut) {
        for block in content {
            visit(block, depth + 1, latest);
        }
    }
    if let Some(text) = value.get("text").and_then(Value::as_str)
        && text.len() <= MAX_ENCODED_IMAGE
        && text.trim_start().starts_with('{')
        && let Ok(mut nested) = serde_json::from_str::<Value>(text)
    {
        visit(&mut nested, depth + 1, latest);
        value["text"] = nested.to_string().into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a3ioAAAAASUVORK5CYII=";

    #[test]
    fn inline_and_nested_images_are_observed_without_base64_in_the_transcript() {
        for nested in [false, true] {
            let content = json!({"content":[{"type":"text","text":"Captured browser"}, {"type":"image","mimeType":"image/png","data":PNG}]});
            let mut result = if nested {
                json!({"content":[{"type":"text","text":content.to_string()}]})
            } else {
                content
            };
            let event = extract(
                &mut result,
                &RunId::new("r"),
                &ToolCallId::new("c"),
                "computer / screenshot",
            )
            .unwrap();
            assert!(
                matches!(event, AgentEvent::ToolImageObserved { data_base64, source, .. } if data_base64 == PNG && source == "computer / screenshot")
            );
            assert!(!result.to_string().contains(PNG));
            assert!(result.to_string().contains("Captured browser"));
        }
    }

    #[test]
    fn urls_invalid_payloads_and_unsupported_formats_are_not_loaded() {
        for (mime, data) in [
            ("image/png", "https://example.com/screen.png"),
            ("image/png", "bad base64!"),
            ("image/png", "aGVsbG8="),
            ("image/svg+xml", PNG),
        ] {
            let mut result = json!({"content":[{"type":"image","mimeType":mime,"data":data}]});
            assert!(
                extract(
                    &mut result,
                    &RunId::new("r"),
                    &ToolCallId::new("c"),
                    "capture"
                )
                .is_none()
            );
        }
    }
}
