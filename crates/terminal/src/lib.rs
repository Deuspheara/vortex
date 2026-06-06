//! PTY + libghostty-vt terminal session for Vortex.
//!
//! Build requirement: Zig 0.15.2 on PATH (see crate `README.md`).

mod core;
mod input;
mod pty;
mod pty_io;
mod renderer;
mod session;
mod theme;
mod vt_dispatch;

pub use input::{
    bracketed_paste_bytes, encode_key_for_pty, key_press_from_parts, normalize_paste,
    paste_needs_confirmation,
};
pub use renderer::{CELL_HEIGHT_PX, CELL_WIDTH_PX, TerminalRenderer};
pub use session::{
    KeyAction, KeyPress, TerminalCell, TerminalCellWidth, TerminalDamageFrame, TerminalFrame,
    TerminalMods, TerminalSession, TerminalSessionMeta, TerminalSessionStatus,
};
pub use theme::{TerminalPalette, TerminalTheme};
