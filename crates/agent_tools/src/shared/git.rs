use std::path::Path;

pub const NOT_A_GIT_REPO: &str =
    "This project is not a git repository. Use list_files, read_file, and search_project instead.";

pub fn is_git_repo(root: &Path) -> bool {
    std::process::Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn tool_error(name: &str, message: impl Into<String>) -> agent_protocol::ToolResult {
    agent_protocol::ToolResult {
        call_id: agent_protocol::ToolCallId::new(""),
        name: name.to_string(),
        output: message.into(),
        is_error: true,
    }
}
