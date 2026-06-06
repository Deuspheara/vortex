//! PTY reader thread and command routing into the VT core thread.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use flume::{Receiver, Sender};
use tracing::warn;

use crate::input::encode_key_for_pty;
use crate::pty::{self, PtyPair};
use crate::session::{KeyPress, TerminalError};
use crate::vt_dispatch::VtCommand;

const READ_BUF: usize = 8192;

pub enum SessionCommand {
    Resize {
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    },
    Key(KeyPress),
    Write(Vec<u8>),
    Scroll(isize),
    ScrollToBottom,
    Kill,
}

pub struct IoHandle {
    pub cmd_tx: Sender<SessionCommand>,
    _reader: thread::JoinHandle<()>,
    _io_loop: thread::JoinHandle<()>,
}

struct PtyState {
    master: std::sync::Mutex<Box<dyn portable_pty::MasterPty + Send>>,
    writer: std::sync::Mutex<Box<dyn Write + Send>>,
    child: std::sync::Mutex<Box<dyn portable_pty::Child + Send + Sync>>,
}

fn write_pty(pty: &PtyState, data: &[u8]) {
    if data.is_empty() {
        return;
    }
    if let Ok(mut writer) = pty.writer.lock() {
        let _ = writer.write_all(data);
        let _ = writer.flush();
    }
}

fn handle_command(pty: &PtyState, vt_cmd: &Sender<VtCommand>, cmd: SessionCommand) -> bool {
    match cmd {
        SessionCommand::Resize {
            cols,
            rows,
            cell_width_px,
            cell_height_px,
        } => {
            if let Ok(master) = pty.master.lock() {
                pty::resize_master(master.as_ref(), cols, rows, cell_width_px, cell_height_px);
            }
            let _ = vt_cmd.send(VtCommand::Resize {
                cols,
                rows,
                cell_width_px,
                cell_height_px,
            });
            true
        }
        SessionCommand::Key(key) => {
            write_pty(pty, &encode_key_for_pty(&key));
            true
        }
        SessionCommand::Write(data) => {
            write_pty(pty, &data);
            true
        }
        SessionCommand::Scroll(delta) => {
            let _ = vt_cmd.send(VtCommand::Scroll(delta));
            true
        }
        SessionCommand::ScrollToBottom => {
            let _ = vt_cmd.send(VtCommand::ScrollToBottom);
            true
        }
        SessionCommand::Kill => {
            if let Ok(mut child) = pty.child.lock() {
                pty::kill_child(child.as_mut());
            }
            false
        }
    }
}

pub fn spawn_io(
    cwd: PathBuf,
    cols: u16,
    rows: u16,
    cell_width_px: u32,
    cell_height_px: u32,
    pty_to_vt_tx: Sender<Vec<u8>>,
    vt_cmd_tx: Sender<VtCommand>,
    wakeup_tx: Sender<()>,
    pty_write_rx: Receiver<Vec<u8>>,
) -> Result<IoHandle, TerminalError> {
    let PtyPair {
        master,
        child,
        mut reader,
        writer,
    } = pty::spawn_shell(&cwd, cols, rows, cell_width_px, cell_height_px)
        .map_err(|e| TerminalError::Spawn(e.to_string()))?;

    let pty = Arc::new(PtyState {
        master: std::sync::Mutex::new(master),
        writer: std::sync::Mutex::new(writer),
        child: std::sync::Mutex::new(child),
    });

    let wakeup_reader = wakeup_tx.clone();
    let reader_handle = thread::Builder::new()
        .name("vortex-pty-reader".into())
        .spawn(move || {
            let mut buf = [0u8; READ_BUF];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if pty_to_vt_tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                        let _ = wakeup_reader.send(());
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(err) => {
                        warn!("pty read error: {err}");
                        break;
                    }
                }
            }
        })
        .map_err(|e| TerminalError::Spawn(e.to_string()))?;

    let (cmd_tx, cmd_rx) = flume::unbounded();
    let pty_cmd = Arc::clone(&pty);
    let vt_cmd = vt_cmd_tx.clone();

    let io_loop = thread::Builder::new()
        .name("vortex-pty-io".into())
        .spawn(move || {
            loop {
                while let Ok(data) = pty_write_rx.try_recv() {
                    write_pty(&pty_cmd, &data);
                }

                while let Ok(cmd) = cmd_rx.try_recv() {
                    if !handle_command(&pty_cmd, &vt_cmd, cmd) {
                        return;
                    }
                }

                match cmd_rx.recv_timeout(Duration::from_millis(8)) {
                    Ok(cmd) => {
                        if !handle_command(&pty_cmd, &vt_cmd, cmd) {
                            break;
                        }
                    }
                    Err(flume::RecvTimeoutError::Timeout) => {}
                    Err(flume::RecvTimeoutError::Disconnected) => break,
                }
            }
        })
        .map_err(|e| TerminalError::Spawn(e.to_string()))?;

    Ok(IoHandle {
        cmd_tx,
        _reader: reader_handle,
        _io_loop: io_loop,
    })
}
