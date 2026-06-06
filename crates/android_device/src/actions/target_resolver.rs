use agent_protocol::{AndroidPointPx, AndroidRectPx, AndroidUiNode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextMatchMode {
    Exact,
    Contains,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AndroidTarget {
    Text {
        value: String,
        #[serde(default = "default_text_match")]
        match_mode: TextMatchMode,
    },
    ResourceId(String),
    ContentDescription(String),
    Bounds(AndroidRectPx),
    Point(AndroidPointPx),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetConfidence {
    High,
    Medium,
    Low,
    Fallback,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResolvedTarget {
    pub target: AndroidTarget,
    pub node: Option<AndroidUiNode>,
    pub point: AndroidPointPx,
    pub confidence: TargetConfidence,
    #[serde(default)]
    pub alternatives: Vec<AndroidUiNode>,
}

fn default_text_match() -> TextMatchMode {
    TextMatchMode::Exact
}

pub fn resolve_target(nodes: &[AndroidUiNode], target: AndroidTarget) -> Option<ResolvedTarget> {
    match target.clone() {
        AndroidTarget::Point(point) => Some(ResolvedTarget {
            target,
            node: None,
            point,
            confidence: TargetConfidence::Fallback,
            alternatives: Vec::new(),
        }),
        AndroidTarget::Bounds(bounds) => Some(ResolvedTarget {
            target,
            node: None,
            point: bounds.center(),
            confidence: TargetConfidence::Fallback,
            alternatives: Vec::new(),
        }),
        AndroidTarget::ResourceId(id) => resolve_with(
            nodes,
            target,
            |node| node.resource_id.as_deref() == Some(id.as_str()),
            TargetConfidence::High,
        ),
        AndroidTarget::ContentDescription(desc) => resolve_with(
            nodes,
            target,
            |node| node.content_desc.as_deref() == Some(desc.as_str()),
            TargetConfidence::High,
        ),
        AndroidTarget::Text { value, match_mode } => {
            let exact = resolve_with(
                nodes,
                target.clone(),
                |node| node.text.as_deref() == Some(value.as_str()),
                TargetConfidence::High,
            );
            if exact.is_some() || matches!(match_mode, TextMatchMode::Exact) {
                return exact;
            }
            let needle = value.to_ascii_lowercase();
            resolve_with(
                nodes,
                target,
                |node| {
                    node.text
                        .as_deref()
                        .map(|text| text.to_ascii_lowercase().contains(&needle))
                        .unwrap_or(false)
                },
                TargetConfidence::Medium,
            )
        }
    }
}

fn resolve_with(
    nodes: &[AndroidUiNode],
    target: AndroidTarget,
    predicate: impl Fn(&AndroidUiNode) -> bool,
    confidence: TargetConfidence,
) -> Option<ResolvedTarget> {
    let mut matches: Vec<AndroidUiNode> = nodes
        .iter()
        .filter(|node| predicate(node))
        .filter(|node| {
            node.visible
                && node.enabled
                && node.bounds.right > node.bounds.left
                && node.bounds.bottom > node.bounds.top
        })
        .cloned()
        .collect();
    let node = matches
        .iter()
        .find(|node| node.clickable && node.enabled && node.visible)
        .or_else(|| matches.iter().find(|node| node.clickable))
        .cloned()
        .or_else(|| matches.first().cloned())?;
    matches.retain(|candidate| candidate.bounds != node.bounds);
    Some(ResolvedTarget {
        target,
        point: node.bounds.center(),
        node: Some(node),
        confidence,
        alternatives: matches,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(text: &str, resource_id: &str, clickable: bool, y: f32) -> AndroidUiNode {
        AndroidUiNode {
            text: Some(text.into()),
            resource_id: Some(resource_id.into()),
            content_desc: None,
            class_name: "android.widget.Button".into(),
            package: Some("com.example".into()),
            clickable,
            enabled: true,
            visible: true,
            bounds: AndroidRectPx {
                left: 10.0,
                top: y,
                right: 110.0,
                bottom: y + 40.0,
            },
        }
    }

    #[test]
    fn prefers_clickable_exact_text_match() {
        let nodes = vec![
            node("Continue", "label", false, 10.0),
            node("Continue", "button", true, 100.0),
        ];
        let resolved = resolve_target(
            &nodes,
            AndroidTarget::Text {
                value: "Continue".into(),
                match_mode: TextMatchMode::Exact,
            },
        )
        .expect("target");
        assert_eq!(resolved.point.y, 120.0);
        assert_eq!(resolved.confidence, TargetConfidence::High);
    }

    #[test]
    fn contains_match_finds_partial_text() {
        let nodes = vec![node("Continue as guest", "guest", true, 20.0)];
        assert!(
            resolve_target(
                &nodes,
                AndroidTarget::Text {
                    value: "continue".into(),
                    match_mode: TextMatchMode::Contains,
                },
            )
            .is_some()
        );
    }

    #[test]
    fn prefers_visible_enabled_match() {
        let hidden = AndroidUiNode {
            visible: false,
            enabled: true,
            ..node("Settings", "hidden", true, 20.0)
        };
        let shown = node("Settings", "shown", true, 120.0);
        let resolved = resolve_target(
            &[hidden, shown.clone()],
            AndroidTarget::Text {
                value: "Settings".into(),
                match_mode: TextMatchMode::Exact,
            },
        )
        .expect("target");
        assert_eq!(resolved.node, Some(shown));
    }
}
