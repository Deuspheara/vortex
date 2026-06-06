//! Retained cell buffer and row-level dirty tracking for canvas painting.

use crate::session::{TerminalCell, TerminalDamageFrame};

/// Monospace cell metrics — must match `Tokens::TERMINAL_CELL_*` in the app.
pub const CELL_WIDTH_PX: u32 = 8;
pub const CELL_HEIGHT_PX: u32 = 18;

/// Retained terminal grid for damage-based repaints.
#[derive(Clone, Debug, Default)]
pub struct TerminalRenderer {
    cols: usize,
    rows: usize,
    cells: Vec<TerminalCell>,
    dirty_rows: Vec<bool>,
    full_redraw: bool,
    pub default_fg: u32,
    pub default_bg: u32,
    pub cursor_col: Option<u16>,
    pub cursor_row: Option<u16>,
    pub cursor_visible: bool,
    pub scrollback_at_bottom: bool,
}

impl TerminalRenderer {
    pub fn apply_damage_frame(&mut self, frame: &TerminalDamageFrame) {
        let cols = frame.cols as usize;
        let rows = frame.rows as usize;
        let size_changed = self.cols != cols || self.rows != rows;

        if size_changed || frame.full_redraw || self.cells.is_empty() {
            self.cols = cols;
            self.rows = rows;
            self.cells = frame.cells.clone();
            self.dirty_rows = vec![true; rows];
            self.full_redraw = true;
        } else {
            // Accumulate damage across every frame applied since the last paint
            // (multiple frames may be drained between repaints). The painter
            // resets these via `clear_dirty` once the rows have been drawn.
            if self.dirty_rows.len() != rows {
                self.dirty_rows = vec![false; rows];
            }
            for (row, &dirty) in frame.dirty_rows.iter().enumerate() {
                if !dirty || row >= rows {
                    continue;
                }
                let start = row * cols;
                let end = start + cols;
                if end <= frame.cells.len() && end <= self.cells.len() {
                    self.cells[start..end].clone_from_slice(&frame.cells[start..end]);
                    if row < self.dirty_rows.len() {
                        self.dirty_rows[row] = true;
                    }
                }
            }
        }

        self.default_fg = frame.default_fg;
        self.default_bg = frame.default_bg;
        self.cursor_col = frame.cursor_col;
        self.cursor_row = frame.cursor_row;
        self.cursor_visible = frame.cursor_visible;
        self.scrollback_at_bottom = frame.scrollback_at_bottom;
    }

    pub fn clear_dirty(&mut self) {
        self.full_redraw = false;
        if !self.dirty_rows.is_empty() {
            self.dirty_rows.fill(false);
        }
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cells(&self) -> &[TerminalCell] {
        &self.cells
    }

    pub fn row_dirty(&self, row: usize) -> bool {
        self.full_redraw || self.dirty_rows.get(row).copied().unwrap_or(true)
    }

    pub fn any_dirty(&self) -> bool {
        self.full_redraw || self.dirty_rows.iter().any(|&d| d)
    }

    pub fn needs_full_paint(&self) -> bool {
        self.full_redraw
    }

    pub fn row_cells(&self, row: usize) -> &[TerminalCell] {
        if self.cols == 0 || row >= self.rows {
            return &[];
        }
        let start = row * self.cols;
        &self.cells[start..start + self.cols]
    }
}
