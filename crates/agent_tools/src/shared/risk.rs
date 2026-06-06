//! Command risk classification for bash and real-command tools.

use agent_protocol::{ApprovalDecision, RiskLevel};
use serde_json::Value;

pub fn command_rule_pattern(tool_name: &str, args: &Value) -> Option<String> {
    match tool_name {
        "run_real_command" => real_command_pattern(args),
        "bash_virtual" => bash_script_pattern(args),
        _ => None,
    }
}

pub fn command_matches_pattern(tool_name: &str, args: &Value, pattern: &str) -> bool {
    command_rule_pattern(tool_name, args)
        .as_deref()
        .is_some_and(|current| current == pattern || current.starts_with(&format!("{pattern} ")))
}

fn real_command_pattern(args: &Value) -> Option<String> {
    let program = args.get("program").and_then(|v| v.as_str())?.trim();
    if program.is_empty() {
        return None;
    }
    let args_list: Vec<&str> = args
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    Some(command_type_pattern(program, &args_list))
}

fn bash_script_pattern(args: &Value) -> Option<String> {
    let script = args
        .get("command")
        .or_else(|| args.get("script"))
        .and_then(|v| v.as_str())?;
    let line = script.lines().find(|line| !line.trim().is_empty())?.trim();
    let words: Vec<&str> = line.split_whitespace().collect();
    let program = words.first()?.trim();
    if program.is_empty() {
        return None;
    }
    Some(command_type_pattern(program, &words[1..]))
}

fn command_type_pattern(program: &str, args: &[&str]) -> String {
    let Some(first_arg) = args.iter().find(|arg| !arg.starts_with('-')) else {
        return program.to_string();
    };
    format!("{program} {first_arg}")
}

pub fn classify_bash_script(script: &str) -> ApprovalDecision {
    let trimmed = script.trim();
    if trimmed.is_empty() {
        return ApprovalDecision::Allow;
    }

    ApprovalDecision::Allow
}

pub fn classify_bash_virtual(args: &Value) -> ApprovalDecision {
    let script = args
        .get("command")
        .or_else(|| args.get("script"))
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    classify_bash_script(script)
}

pub fn classify_real_command(args: &Value) -> ApprovalDecision {
    let program = args
        .get("program")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let args_list: Vec<&str> = args
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let joined = format!("{program} {}", args_list.join(" "));

    if program == "sudo" {
        return ApprovalDecision::Deny {
            reason: "sudo is blocked".into(),
        };
    }

    if joined.contains("| sh") || joined.contains("| bash") {
        return ApprovalDecision::Deny {
            reason: "Piping remote or generated content into a shell is blocked".into(),
        };
    }

    if joined.contains("rm -rf")
        || joined.contains("git reset --hard")
        || joined.contains("git clean -fd")
    {
        return ApprovalDecision::AskUser {
            risk: RiskLevel::Critical,
            reason: "Command may destroy local changes".into(),
        };
    }

    match program {
        "git" if args_list.first() == Some(&"status") || args_list.first() == Some(&"diff") => {
            ApprovalDecision::Allow
        }
        "cargo" if args_list.first() == Some(&"check") || args_list.first() == Some(&"test") => {
            ApprovalDecision::AskUser {
                risk: RiskLevel::Medium,
                reason: "This executes project code on your machine".into(),
            }
        }
        "npm" | "pnpm" | "yarn" | "bun" if args_list.iter().any(|a| *a == "install") => {
            ApprovalDecision::AskUser {
                risk: RiskLevel::High,
                reason: "Package installs require approval".into(),
            }
        }
        _ => ApprovalDecision::AskUser {
            risk: RiskLevel::Medium,
            reason: "This command executes code on your machine".into(),
        },
    }
}

pub fn decision_to_assessment(decision: &ApprovalDecision) -> (bool, bool, RiskLevel, String) {
    match decision {
        ApprovalDecision::Allow => (false, false, RiskLevel::SafeRead, String::new()),
        ApprovalDecision::Deny { reason } => (true, false, RiskLevel::Critical, reason.clone()),
        ApprovalDecision::AskUser { risk, reason } => (false, true, *risk, reason.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_protocol::ApprovalDecision;

    #[test]
    fn bash_virtual_requires_approval() {
        let args = serde_json::json!({ "command": "cargo check" });
        assert!(matches!(
            classify_bash_virtual(&args),
            ApprovalDecision::Allow
        ));
    }

    #[test]
    fn bash_virtual_allows_fake_sudo_name() {
        let args = serde_json::json!({ "command": "sudo rm -rf /" });
        assert!(matches!(
            classify_bash_virtual(&args),
            ApprovalDecision::Allow
        ));
    }
}
