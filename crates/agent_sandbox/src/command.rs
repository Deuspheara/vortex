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
    let script = args.get("script").and_then(|v| v.as_str())?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_pattern_groups_same_command_type() {
        let args = serde_json::json!({
            "program": "cargo",
            "args": ["check", "-p", "app"]
        });
        assert_eq!(
            command_rule_pattern("run_real_command", &args).as_deref(),
            Some("cargo check")
        );
        assert!(command_matches_pattern(
            "run_real_command",
            &args,
            "cargo check"
        ));
    }
}
