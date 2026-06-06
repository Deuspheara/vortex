//! Cross-platform PTY spawn.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use portable_pty::{CommandBuilder, ExitStatus, PtySize, native_pty_system};
use thiserror::Error;
use tracing::warn;

#[derive(Debug, Error)]
pub enum PtyError {
    #[error("pty: {0}")]
    Io(#[from] std::io::Error),
    #[error("pty spawn: {0}")]
    Spawn(String),
}

pub struct PtyPair {
    pub master: Box<dyn portable_pty::MasterPty + Send>,
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
}

fn default_shell() -> PathBuf {
    std::env::var("SHELL")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/bin/zsh"))
}

pub fn shell_command(cwd: &Path) -> CommandBuilder {
    let shell = default_shell();
    let mut cmd = CommandBuilder::new(shell.to_string_lossy().to_string());
    if shell_interactive_flag(&shell) {
        cmd.arg("-i");
    }
    cmd.cwd(cwd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd
}

fn shell_interactive_flag(shell: &Path) -> bool {
    matches!(
        shell.file_name().and_then(|name| name.to_str()),
        Some("zsh" | "bash" | "sh" | "ksh" | "dash")
    )
}

pub fn spawn_shell(
    cwd: &Path,
    cols: u16,
    rows: u16,
    cell_width_px: u32,
    cell_height_px: u32,
) -> Result<PtyPair, PtyError> {
    let pixel_width = (cols as u32 * cell_width_px).min(u16::MAX as u32) as u16;
    let pixel_height = (rows as u32 * cell_height_px).min(u16::MAX as u32) as u16;
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width,
            pixel_height,
        })
        .map_err(|e| PtyError::Spawn(e.to_string()))?;

    let cmd = shell_command(cwd);
    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| PtyError::Spawn(e.to_string()))?;

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| PtyError::Spawn(e.to_string()))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| PtyError::Spawn(e.to_string()))?;

    Ok(PtyPair {
        master: pair.master,
        child,
        reader,
        writer,
    })
}

pub fn resize_master(
    master: &dyn portable_pty::MasterPty,
    cols: u16,
    rows: u16,
    cell_width_px: u32,
    cell_height_px: u32,
) {
    let pixel_width = (cols as u32 * cell_width_px).min(u16::MAX as u32) as u16;
    let pixel_height = (rows as u32 * cell_height_px).min(u16::MAX as u32) as u16;
    if let Err(err) = master.resize(PtySize {
        rows,
        cols,
        pixel_width,
        pixel_height,
    }) {
        warn!("pty resize failed: {err}");
    }
}

pub fn kill_child(child: &mut dyn portable_pty::Child) {
    if let Err(err) = child.kill() {
        warn!("pty kill failed: {err}");
    }
    match child.try_wait() {
        Ok(Some(ExitStatus { .. })) => {}
        Ok(None) => {
            let _ = child.wait();
        }
        Err(err) => warn!("pty wait failed: {err}"),
    }
}
