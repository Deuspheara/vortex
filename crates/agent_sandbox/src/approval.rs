use agent_protocol::{AgentMode, ApprovalDecision, RiskLevel, ToolAssessment};
use agent_store::StoredApprovalRule;

use crate::command::command_matches_pattern;

pub struct ApprovalEngine;

impl ApprovalEngine {
    pub fn decide(
        _mode: &AgentMode,
        tool_name: &str,
        args: &serde_json::Value,
        assessment: &ToolAssessment,
        rules: &[StoredApprovalRule],
        project_tool_match: Option<&StoredApprovalRule>,
    ) -> ApprovalDecision {
        if assessment.denied {
            return ApprovalDecision::Deny {
                reason: assessment.reason.clone(),
            };
        }

        let base = if assessment.requires_approval {
            ApprovalDecision::AskUser {
                risk: assessment.risk,
                reason: assessment.reason.clone(),
            }
        } else {
            ApprovalDecision::Allow
        };

        if matches!(base, ApprovalDecision::Deny { .. }) {
            return base;
        }

        let matching_rule = project_tool_match
            .filter(|rule| approval_rule_matches(rule, tool_name, args))
            .or_else(|| {
                rules
                    .iter()
                    .find(|rule| approval_rule_matches(rule, tool_name, args))
            });

        if let Some(rule) = matching_rule {
            if matches!(base, ApprovalDecision::AskUser { risk, .. } if risk <= rule.max_risk) {
                return ApprovalDecision::Allow;
            }
        }

        base
    }
}

fn approval_rule_matches(
    rule: &StoredApprovalRule,
    tool_name: &str,
    args: &serde_json::Value,
) -> bool {
    if rule.tool_name != tool_name {
        return false;
    }
    match rule.command_pattern.as_deref() {
        Some(pattern) => command_matches_pattern(tool_name, args, pattern),
        None => true,
    }
}

pub fn risk_from_decision(decision: &ApprovalDecision) -> RiskLevel {
    match decision {
        ApprovalDecision::Allow => RiskLevel::SafeRead,
        ApprovalDecision::AskUser { risk, .. } => *risk,
        ApprovalDecision::Deny { .. } => RiskLevel::Critical,
    }
}
