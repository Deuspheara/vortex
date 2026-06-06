//! Dedicated thread owning libghostty-vt (`!Send`).

use std::thread;
use std::time::{Duration, Instant};

use flume::{Receiver, Sender};
use libghostty_vt::Error as VtError;
use libghostty_vt::render::{CellIterator, Dirty, RenderState, RowIterator};
use libghostty_vt::terminal::{
    ConformanceLevel, DeviceAttributeFeature, DeviceAttributes, DeviceType,
    PrimaryDeviceAttributes, ScrollViewport, SecondaryDeviceAttributes, SizeReportSize,
};
use libghostty_vt::{Terminal, TerminalOptions};
use tracing::warn;

use crate::renderer::{CELL_HEIGHT_PX, CELL_WIDTH_PX};
use crate::session::{TerminalCell, TerminalCellWidth, TerminalDamageFrame, rgb_pack};
use crate::theme::TerminalTheme;
use crate::vt_dispatch::VtCommand;

/// Minimum spacing between emitted frames (~60fps). The PTY can produce output
/// far faster than any display can show it, so we pace captures to one per frame
/// and rely on damage diffing to keep each capture cheap downstream.
const WAKEUP_COALESCE: Duration = Duration::from_millis(16);

pub fn spawn_core_thread(
    cols: u16,
    rows: u16,
    theme: TerminalTheme,
    pty_rx: Receiver<Vec<u8>>,
    cmd_rx: Receiver<VtCommand>,
    frame_tx: Sender<TerminalDamageFrame>,
    wakeup_rx: Receiver<()>,
    pty_write_tx: Sender<Vec<u8>>,
) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("vortex-vt".into())
        .spawn(move || {
            if let Err(err) = run_vt_loop(
                cols,
                rows,
                theme,
                pty_rx,
                cmd_rx,
                frame_tx,
                wakeup_rx,
                pty_write_tx,
            ) {
                warn!("vt thread exited: {err}");
            }
        })
        .expect("spawn vt thread")
}

fn run_vt_loop(
    cols: u16,
    rows: u16,
    theme: TerminalTheme,
    pty_rx: Receiver<Vec<u8>>,
    cmd_rx: Receiver<VtCommand>,
    frame_tx: Sender<TerminalDamageFrame>,
    wakeup_rx: Receiver<()>,
    pty_write_tx: Sender<Vec<u8>>,
) -> Result<(), VtError> {
    let mut terminal = Terminal::new(TerminalOptions {
        cols,
        rows,
        max_scrollback: 10_000,
    })?;
    terminal.resize(cols, rows, CELL_WIDTH_PX, CELL_HEIGHT_PX)?;

    let pty_tx = pty_write_tx.clone();
    terminal
        .on_pty_write(move |_t, data| {
            let _ = pty_tx.send(data.to_vec());
        })?
        .on_size(move |term| {
            let columns = term.cols().ok()?;
            let rows = term.rows().ok()?;
            Some(SizeReportSize {
                rows,
                columns,
                cell_width: CELL_WIDTH_PX,
                cell_height: CELL_HEIGHT_PX,
            })
        })?
        .on_device_attributes(|_term| {
            Some(DeviceAttributes {
                primary: PrimaryDeviceAttributes::new(
                    ConformanceLevel::VT220,
                    [
                        DeviceAttributeFeature::COLUMNS_132,
                        DeviceAttributeFeature::SELECTIVE_ERASE,
                        DeviceAttributeFeature::ANSI_COLOR,
                    ],
                ),
                secondary: SecondaryDeviceAttributes {
                    device_type: DeviceType::VT220,
                    firmware_version: 1,
                    rom_cartridge: 0,
                },
                tertiary: Default::default(),
            })
        })?
        .on_xtversion(|_term| Some("vortex"))?;

    let mut render_state = RenderState::new()?;
    let mut row_it = RowIterator::new()?;
    let mut cell_it = CellIterator::new()?;

    let mut screen_cols = cols;
    let mut screen_rows = rows;
    let mut last_frame = Instant::now();
    let mut dirty = true;

    // Previous emitted grid + cursor/scroll state, used to compute real damage
    // because libghostty-vt 0.1.1 reports `Dirty::Full` for every frame.
    let mut prev_cells: Vec<TerminalCell> = Vec::new();
    let mut prev_cursor: (Option<u16>, Option<u16>, bool) = (None, None, false);
    let mut prev_scroll_at_bottom = true;

    loop {
        while let Ok(data) = pty_rx.try_recv() {
            terminal.vt_write(&data);
            dirty = true;
        }

        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                VtCommand::Resize {
                    cols,
                    rows,
                    cell_width_px: cw,
                    cell_height_px: ch,
                } => {
                    screen_cols = cols;
                    screen_rows = rows;
                    terminal.resize(cols, rows, cw, ch)?;
                    dirty = true;
                }
                VtCommand::Scroll(delta) => {
                    terminal.scroll_viewport(ScrollViewport::Delta(delta));
                    dirty = true;
                }
                VtCommand::ScrollToBottom => {
                    terminal.scroll_viewport(ScrollViewport::Bottom);
                    dirty = true;
                }
            }
        }

        while wakeup_rx.try_recv().is_ok() {
            dirty = true;
        }

        if dirty && last_frame.elapsed() >= WAKEUP_COALESCE {
            if let Ok(mut frame) = capture_damage_frame(
                &terminal,
                &theme,
                &mut render_state,
                &mut row_it,
                &mut cell_it,
                screen_cols,
                screen_rows,
            ) {
                let cols = frame.cols as usize;
                let rows = frame.rows as usize;
                let size_changed = prev_cells.len() != frame.cells.len();

                let mut any_dirty = false;
                if size_changed {
                    frame.full_redraw = true;
                    for d in frame.dirty_rows.iter_mut() {
                        *d = true;
                    }
                    any_dirty = true;
                } else {
                    frame.full_redraw = false;
                    for row in 0..rows {
                        let start = row * cols;
                        let end = start + cols;
                        let changed = frame.cells[start..end] != prev_cells[start..end];
                        frame.dirty_rows[row] = changed;
                        any_dirty |= changed;
                    }
                }

                let cursor = (frame.cursor_col, frame.cursor_row, frame.cursor_visible);
                let cursor_changed = cursor != prev_cursor;
                let scroll_changed = frame.scrollback_at_bottom != prev_scroll_at_bottom;

                if any_dirty || cursor_changed || scroll_changed {
                    prev_cells = frame.cells.clone();
                    prev_cursor = cursor;
                    prev_scroll_at_bottom = frame.scrollback_at_bottom;
                    let _ = frame_tx.try_send(frame);
                }
            }
            dirty = false;
            last_frame = Instant::now();
        }

        if pty_rx.is_disconnected() && cmd_rx.is_disconnected() {
            break;
        }

        thread::sleep(Duration::from_millis(2));
    }

    Ok(())
}

fn capture_damage_frame<'a>(
    terminal: &Terminal<'a, 'a>,
    theme: &TerminalTheme,
    render_state: &mut RenderState<'a>,
    row_it: &mut RowIterator<'a>,
    cell_it: &mut CellIterator<'a>,
    cols: u16,
    rows: u16,
) -> Result<TerminalDamageFrame, VtError> {
    let snapshot = render_state.update(terminal)?;
    // libghostty-vt 0.1.1 reports `Dirty::Full` (or errors) for every frame after
    // a vt_write, so its damage info is unusable. We capture the entire grid here
    // and let the VT loop compute real per-row damage by diffing against the
    // previously emitted frame.
    let colors = snapshot.colors()?;
    let default_fg = rgb_pack(colors.foreground);
    let default_bg = rgb_pack(colors.background);
    let default_fg = if default_fg == 0 {
        theme.default_fg()
    } else {
        default_fg
    };
    let default_bg = if default_bg == 0 {
        theme.default_bg()
    } else {
        default_bg
    };

    let cols_usize = cols as usize;
    let rows_usize = rows as usize;
    let mut cells = vec![TerminalCell::default(); cols_usize * rows_usize];
    // Damage is resolved by the caller via diffing; default to clean here.
    let dirty_rows = vec![false; rows_usize];

    let mut row_it = row_it.update(&snapshot)?;
    let mut row_y = 0usize;
    while let Some(row) = row_it.next() {
        if row_y >= rows_usize {
            break;
        }
        let mut cell_it = cell_it.update(row)?;
        let mut col_x = 0usize;
        while let Some(cell) = cell_it.next() {
            if col_x >= cols_usize {
                break;
            }
            let mut fg = cell.fg_color()?.map(rgb_pack);
            let mut bg = cell.bg_color()?.map(rgb_pack);
            let raw_cell = cell.raw_cell()?;
            let width = match raw_cell.wide()? {
                libghostty_vt::screen::CellWide::Narrow => TerminalCellWidth::Narrow,
                libghostty_vt::screen::CellWide::Wide => TerminalCellWidth::Wide,
                libghostty_vt::screen::CellWide::SpacerTail => TerminalCellWidth::SpacerTail,
                libghostty_vt::screen::CellWide::SpacerHead => TerminalCellWidth::SpacerHead,
            };
            let style = cell.style()?;
            let bold = style.bold;
            let italic = style.italic;
            let faint = style.faint;
            let inverse = style.inverse;
            let strikethrough = style.strikethrough;
            let underline = !matches!(style.underline, libghostty_vt::style::Underline::None);
            if inverse {
                std::mem::swap(&mut fg, &mut bg);
            }
            let grapheme_len = cell.graphemes_len()?;
            let text = if width.is_spacer() || grapheme_len == 0 {
                String::new()
            } else {
                let mut buf = vec!['\0'; grapheme_len];
                cell.graphemes_buf(&mut buf)?;
                buf.into_iter().filter(|ch| *ch != '\0').collect()
            };
            let idx = row_y * cols_usize + col_x;
            cells[idx] = TerminalCell {
                text,
                fg,
                bg,
                width,
                bold,
                inverse,
                italic,
                faint,
                strikethrough,
                underline,
            };
            col_x += 1;
        }
        row_y += 1;
    }

    let cursor_visible = snapshot.cursor_visible()?;
    let (cursor_col, cursor_row) = snapshot
        .cursor_viewport()?
        .map(|vp| (Some(vp.x), Some(vp.y)))
        .unwrap_or((None, None));

    let scrollback_at_bottom = scrollback_at_bottom(terminal)?;

    let frame = TerminalDamageFrame {
        cols,
        rows,
        cells,
        dirty_rows,
        full_redraw: false,
        cursor_col,
        cursor_row,
        cursor_visible,
        default_fg,
        default_bg,
        scrollback_at_bottom,
    };
    let _ = snapshot.set_dirty(Dirty::Clean);
    Ok(frame)
}

fn scrollback_at_bottom(terminal: &Terminal<'_, '_>) -> Result<bool, VtError> {
    let bar = terminal.scrollbar()?;
    Ok(bar.offset + bar.len >= bar.total)
}
