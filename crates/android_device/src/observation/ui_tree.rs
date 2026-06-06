use agent_protocol::{AndroidRectPx, AndroidUiNode};
use quick_xml::Reader;
use quick_xml::events::Event;

pub fn parse_ui_tree(xml: &str) -> Result<Vec<AndroidUiNode>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut nodes = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(event)) | Ok(Event::Start(event))
                if event.name().as_ref() == b"node" =>
            {
                let mut text = None;
                let mut resource_id = None;
                let mut content_desc = None;
                let mut class_name = String::new();
                let mut package = None;
                let mut clickable = false;
                let mut enabled = true;
                let mut visible = true;
                let mut bounds = None;

                for attr in event.attributes().flatten() {
                    let key = attr.key.as_ref();
                    let value = attr
                        .decode_and_unescape_value(reader.decoder())
                        .map_err(|e| e.to_string())?
                        .into_owned();
                    match key {
                        b"text" if !value.is_empty() => text = Some(value),
                        b"resource-id" if !value.is_empty() => resource_id = Some(value),
                        b"content-desc" if !value.is_empty() => content_desc = Some(value),
                        b"class" => class_name = value,
                        b"package" if !value.is_empty() => package = Some(value),
                        b"clickable" => clickable = value == "true",
                        b"enabled" => enabled = value != "false",
                        b"displayed" | b"visible-to-user" => visible = value != "false",
                        b"bounds" => bounds = parse_bounds(&value),
                        _ => {}
                    }
                }

                if let Some(bounds) = bounds {
                    let relevant = clickable
                        || text.is_some()
                        || resource_id.is_some()
                        || content_desc.is_some();
                    if relevant {
                        nodes.push(AndroidUiNode {
                            text,
                            resource_id,
                            content_desc,
                            class_name,
                            package,
                            clickable,
                            enabled,
                            visible,
                            bounds,
                        });
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => return Err(err.to_string()),
        }
        buf.clear();
    }

    Ok(nodes)
}

pub fn parse_bounds(value: &str) -> Option<AndroidRectPx> {
    let mut nums = Vec::with_capacity(4);
    let mut current = String::new();
    for ch in value.chars() {
        if ch.is_ascii_digit() || ch == '-' {
            current.push(ch);
        } else if !current.is_empty() {
            nums.push(current.parse::<f32>().ok()?);
            current.clear();
        }
    }
    if !current.is_empty() {
        nums.push(current.parse::<f32>().ok()?);
    }
    if nums.len() == 4 {
        Some(AndroidRectPx {
            left: nums[0],
            top: nums[1],
            right: nums[2],
            bottom: nums[3],
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_uiautomator_nodes() {
        let xml = r#"<hierarchy>
            <node text="Continue" resource-id="com.app:id/continue" class="android.widget.Button" package="com.app" clickable="true" enabled="true" bounds="[120,1800][960,1900]" />
            <node text="" class="android.view.View" clickable="false" bounds="[0,0][1,1]" />
        </hierarchy>"#;
        let nodes = parse_ui_tree(xml).expect("xml");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].text.as_deref(), Some("Continue"));
        assert_eq!(nodes[0].bounds.center().x, 540.0);
    }

    #[test]
    fn parses_bounds() {
        assert_eq!(
            parse_bounds("[120,1800][960,1900]"),
            Some(AndroidRectPx {
                left: 120.0,
                top: 1800.0,
                right: 960.0,
                bottom: 1900.0,
            })
        );
    }
}
