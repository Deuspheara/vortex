use std::sync::Arc;
use std::time::Instant;

use crate::{
    CommandContext, CommandRegistry, ExecutionLimits, ShellError, ShellPolicy, ShellResult,
    VirtualFs, VirtualPath, parse_script,
};

pub struct Shell {
    fs: Arc<dyn VirtualFs>,
    registry: CommandRegistry,
    limits: ExecutionLimits,
    policy: ShellPolicy,
    cwd: VirtualPath,
}

#[derive(Clone, Debug)]
pub struct ExecRequest {
    pub command: String,
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
    pub max_output_bytes: usize,
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub truncated: bool,
}

impl Shell {
    pub fn new(
        fs: Arc<dyn VirtualFs>,
        registry: CommandRegistry,
        limits: ExecutionLimits,
        policy: ShellPolicy,
        cwd: VirtualPath,
    ) -> Self {
        Self {
            fs,
            registry,
            limits,
            policy,
            cwd,
        }
    }

    pub fn exec(&mut self, request: ExecRequest) -> ExecResult {
        let started = Instant::now();
        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut exit_code = 0;
        let mut truncated = false;

        let cwd = match request.cwd.as_deref() {
            Some(cwd) => match VirtualPath::normalize(&self.cwd, cwd) {
                Ok(path) => path,
                Err(err) => return error_result(err, started),
            },
            None => self.cwd.clone(),
        };

        let commands = match parse_script(&request.command, &request.env) {
            Ok(commands) => commands,
            Err(err) => return error_result(err, started),
        };
        let max_commands = self
            .limits
            .max_command_count
            .min(self.policy.max_commands_per_exec);
        if commands.len() > max_commands {
            return error_result(
                ShellError::LimitExceeded(format!(
                    "command count {} exceeds limit {}",
                    commands.len(),
                    max_commands
                )),
                started,
            );
        }

        let max_output = request
            .max_output_bytes
            .min(self.limits.max_output_bytes)
            .min(self.policy.max_output_bytes);

        for command in commands {
            if started.elapsed().as_millis() as u64 > request.timeout_ms.min(self.limits.timeout_ms)
            {
                exit_code = 124;
                append_limited(
                    &mut stderr,
                    "fake shell timeout\n",
                    max_output,
                    &mut truncated,
                );
                break;
            }
            let Some(name) = command.words.first() else {
                continue;
            };
            let Some(builtin) = self.registry.get(name) else {
                exit_code = 127;
                append_limited(
                    &mut stderr,
                    &format!("{name}: command not found\n"),
                    max_output,
                    &mut truncated,
                );
                break;
            };
            let args = &command.words[1..];
            let mut ctx = CommandContext {
                fs: self.fs.clone(),
                cwd: cwd.clone(),
                policy: self.policy.clone(),
                limits: self.limits.clone(),
            };
            match builtin.run(&mut ctx, args) {
                Ok(output) => {
                    exit_code = output.exit_code;
                    append_limited(&mut stdout, &output.stdout, max_output, &mut truncated);
                    append_limited(&mut stderr, &output.stderr, max_output, &mut truncated);
                }
                Err(err) => {
                    exit_code = err.exit_code();
                    append_limited(&mut stderr, &format!("{err}\n"), max_output, &mut truncated);
                }
            }
            if exit_code != 0 {
                break;
            }
        }

        ExecResult {
            stdout,
            stderr,
            exit_code,
            duration_ms: started.elapsed().as_millis() as u64,
            truncated,
        }
    }
}

fn error_result(error: ShellError, started: Instant) -> ExecResult {
    ExecResult {
        stdout: String::new(),
        stderr: format!("{error}\n"),
        exit_code: error.exit_code(),
        duration_ms: started.elapsed().as_millis() as u64,
        truncated: false,
    }
}

fn append_limited(out: &mut String, chunk: &str, max: usize, truncated: &mut bool) {
    if out.len() >= max {
        *truncated = true;
        return;
    }
    let available = max - out.len();
    if chunk.len() <= available {
        out.push_str(chunk);
        return;
    }
    let mut end = available;
    while end > 0 && !chunk.is_char_boundary(end) {
        end -= 1;
    }
    out.push_str(&chunk[..end]);
    out.push_str("\n[output truncated]\n");
    *truncated = true;
}

pub(crate) fn ensure_writes_allowed(policy: &ShellPolicy) -> ShellResult<()> {
    if policy.allow_writes {
        Ok(())
    } else {
        Err(ShellError::AccessDenied(
            "writes are disabled by shell policy".into(),
        ))
    }
}
