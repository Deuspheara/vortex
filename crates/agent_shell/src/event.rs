use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellEvent {
    ShellStarted {
        command: String,
        cwd: String,
    },
    ShellCommandStarted {
        command: String,
        cwd: String,
    },
    ShellStdoutChunk {
        chunk: String,
    },
    ShellStderrChunk {
        chunk: String,
    },
    ShellCommandCompleted {
        exit_code: i32,
        duration_ms: u64,
    },
    ShellCompleted {
        exit_code: i32,
        duration_ms: u64,
        truncated: bool,
    },
    ShellFailed {
        message: String,
    },
}
