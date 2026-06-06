//! Public session handle — Send-safe facade over the VT thread.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::Instant;

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

use flume::{Receiver, Sender};
use thiserror::Error;

use crate::core::spawn_core_thread;
use crate::pty_io::{IoHandle, SessionCommand, spawn_io};
use crate::renderer::{CELL_HEIGHT_PX, CELL_WIDTH_PX};
use crate::theme::TerminalTheme;
use crate::vt_dispatch::VtCommand;

#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("terminal spawn: {0}")]
    Spawn(String),
    #[error("terminal channel closed")]
    Closed,
}

/// Lifecycle status for a terminal session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalSessionStatus {
    Starting,
    Running,
    Exited,
}

/// Session metadata exposed to the app layer.
#[derive(Clone, Debug)]
pub struct TerminalSessionMeta {
    pub id: String,
    pub cwd: PathBuf,
    pub shell: PathBuf,
    pub status: TerminalSessionStatus,
    pub title: String,
    pub last_activity: Instant,
}

/// Modifier keys for keyboard input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalMods {
    pub shift: bool,
    pub alt: bool,
    pub control: bool,
    pub super_key: bool,
}

/// Key press / release / repeat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    Press,
    Release,
    Repeat,
}

/// Keyboard event from the UI layer.
#[derive(Clone, Debug)]
pub struct KeyPress {
    pub key: String,
    pub mods: TerminalMods,
    pub action: KeyAction,
    pub text: Option<String>,
}

/// One rendered terminal cell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TerminalCellWidth {
    #[default]
    Narrow,
    Wide,
    SpacerTail,
    SpacerHead,
}

impl TerminalCellWidth {
    pub fn is_spacer(self) -> bool {
        matches!(self, Self::SpacerTail | Self::SpacerHead)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TerminalCell {
    pub text: String,
    pub fg: Option<u32>,
    pub bg: Option<u32>,
    pub width: TerminalCellWidth,
    pub bold: bool,
    pub inverse: bool,
    pub italic: bool,
    pub faint: bool,
    pub strikethrough: bool,
    pub underline: bool,
}

/// Damage-aware screen snapshot for GPUI canvas painting.
#[derive(Clone, Debug)]
pub struct TerminalDamageFrame {
    pub cols: u16,
    pub rows: u16,
    pub cells: Vec<TerminalCell>,
    pub dirty_rows: Vec<bool>,
    pub full_redraw: bool,
    pub cursor_col: Option<u16>,
    pub cursor_row: Option<u16>,
    pub cursor_visible: bool,
    pub default_fg: u32,
    pub default_bg: u32,
    pub scrollback_at_bottom: bool,
}

/// Alias for older call sites.
pub type TerminalFrame = TerminalDamageFrame;

/// Interactive shell session (one per project).
pub struct TerminalSession {
    meta: TerminalSessionMeta,
    frame_rx: Receiver<TerminalDamageFrame>,
    cmd_tx: Sender<SessionCommand>,
    _io: IoHandle,
    _vt: JoinHandle<()>,
}

impl TerminalSession {
    pub fn spawn(
        cwd: impl AsRef<Path>,
        cols: u16,
        rows: u16,
        theme: TerminalTheme,
    ) -> Result<Self, TerminalError> {
        let cwd = cwd.as_ref().to_path_buf();
        let shell = std::env::var("SHELL")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/bin/zsh"));
        let id = format!("term-{}", NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed));
        let meta = TerminalSessionMeta {
            id: id.clone(),
            cwd: cwd.clone(),
            shell,
            status: TerminalSessionStatus::Starting,
            title: String::new(),
            last_activity: Instant::now(),
        };

        let (pty_to_vt_tx, pty_to_vt_rx) = flume::bounded::<Vec<u8>>(256);
        let (pty_write_tx, pty_write_rx) = flume::bounded::<Vec<u8>>(64);
        let (vt_cmd_tx, vt_cmd_rx) = flume::bounded::<VtCommand>(64);
        let (frame_tx, frame_rx) = flume::bounded::<TerminalDamageFrame>(64);
        let (wakeup_tx, wakeup_rx) = flume::bounded::<()>(64);

        let io = spawn_io(
            cwd,
            cols,
            rows,
            CELL_WIDTH_PX,
            CELL_HEIGHT_PX,
            pty_to_vt_tx,
            vt_cmd_tx,
            wakeup_tx.clone(),
            pty_write_rx,
        )
        .map_err(|e| TerminalError::Spawn(e.to_string()))?;

        let vt = spawn_core_thread(
            cols,
            rows,
            theme,
            pty_to_vt_rx,
            vt_cmd_rx,
            frame_tx,
            wakeup_rx,
            pty_write_tx,
        );

        Ok(Self {
            meta,
            frame_rx,
            cmd_tx: io.cmd_tx.clone(),
            _io: io,
            _vt: vt,
        })
    }

    pub fn meta(&self) -> &TerminalSessionMeta {
        &self.meta
    }

    pub fn touch_activity(&mut self) {
        self.meta.last_activity = Instant::now();
    }

    pub fn set_status(&mut self, status: TerminalSessionStatus) {
        self.meta.status = status;
    }

    pub fn set_title(&mut self, title: String) {
        self.meta.title = title;
    }

    pub fn try_recv_frame(&self) -> Option<TerminalDamageFrame> {
        self.frame_rx.try_recv().ok()
    }

    pub fn frame_notifications(&self) -> Receiver<TerminalDamageFrame> {
        self.frame_rx.clone()
    }

    pub fn resize(&self, cols: u16, rows: u16, cell_width_px: u32, cell_height_px: u32) {
        let _ = self.cmd_tx.send(SessionCommand::Resize {
            cols,
            rows,
            cell_width_px,
            cell_height_px,
        });
    }

    pub fn send_key(&self, key: KeyPress) {
        let _ = self.cmd_tx.send(SessionCommand::Key(key));
    }

    pub fn send_bytes(&self, data: Vec<u8>) {
        let _ = self.cmd_tx.send(SessionCommand::Write(data));
    }

    pub fn scroll_viewport(&self, delta: isize) {
        let _ = self.cmd_tx.send(SessionCommand::Scroll(delta));
    }

    pub fn scroll_viewport_to_bottom(&self) {
        let _ = self.cmd_tx.send(SessionCommand::ScrollToBottom);
    }

    pub fn kill(&self) {
        let _ = self.cmd_tx.send(SessionCommand::Kill);
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.kill();
    }
}

pub(crate) fn rgb_pack(color: libghostty_vt::style::RgbColor) -> u32 {
    let libghostty_vt::style::RgbColor { r, g, b } = color;
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}
