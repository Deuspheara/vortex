use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use agent_protocol::{AgentError, CancellationToken, OutputStreamKind, RealCommandRequest};
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

pub struct ProcessOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub struct RealProcessExecutor;

async fn read_stream<R: AsyncReadExt + Unpin>(
    mut reader: R,
    stream: OutputStreamKind,
    mut on_delta: impl FnMut(OutputStreamKind, String),
) -> String {
    let mut collected = String::new();
    let mut buf = vec![0u8; 4096];
    loop {
        let n = reader.read(&mut buf).await.unwrap_or(0);
        if n == 0 {
            break;
        }
        let chunk = String::from_utf8_lossy(&buf[..n]).into_owned();
        on_delta(stream.clone(), chunk.clone());
        collected.push_str(&chunk);
        if collected.len() > 256_000 {
            break;
        }
    }
    collected
}

impl RealProcessExecutor {
    pub async fn run<F>(
        request: RealCommandRequest,
        cancel: CancellationToken,
        mut on_delta: F,
    ) -> Result<ProcessOutput, AgentError>
    where
        F: FnMut(OutputStreamKind, String) + Send,
    {
        cancel.check_cancelled()?;

        let mut cmd = Command::new(&request.program);
        cmd.args(&request.args)
            .current_dir(&request.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if request.stdin.is_some() {
            cmd.stdin(Stdio::piped());
        }

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.as_std_mut().process_group(0);
        }

        let mut child = cmd.spawn().map_err(|e| AgentError::Other(e.to_string()))?;
        let timeout_d = Duration::from_secs(request.timeout_secs.max(1));

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let result = timeout(timeout_d, async {
            let stdout_collected = if let Some(out) = stdout {
                read_stream(BufReader::new(out), OutputStreamKind::Stdout, &mut on_delta).await
            } else {
                String::new()
            };
            let stderr_collected = if let Some(err) = stderr {
                read_stream(BufReader::new(err), OutputStreamKind::Stderr, &mut on_delta).await
            } else {
                String::new()
            };
            let status = child
                .wait()
                .await
                .map_err(|e| AgentError::Other(e.to_string()))?;
            Ok::<_, AgentError>((
                stdout_collected,
                stderr_collected,
                status.code().unwrap_or(-1),
            ))
        })
        .await;

        match result {
            Ok(Ok((stdout, stderr, exit_code))) => Ok(ProcessOutput {
                stdout,
                stderr,
                exit_code,
            }),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                let _ = child.kill().await;
                Err(AgentError::Other("command timed out".into()))
            }
        }
    }

    pub fn validate_cwd(project_root: &Path, cwd: &Path) -> Result<(), AgentError> {
        let root = project_root
            .canonicalize()
            .map_err(|e| AgentError::Other(e.to_string()))?;
        let canonical = cwd
            .canonicalize()
            .map_err(|e| AgentError::Other(e.to_string()))?;
        if !canonical.starts_with(&root) {
            return Err(AgentError::Other("cwd escapes project root".into()));
        }
        Ok(())
    }
}
