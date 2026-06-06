//! Interactive terminal surface — single GPUI canvas viewport over libghostty frames.

use std::sync::Arc;

use gpui::{
    App, BorderStyle, Bounds, Context, ElementInputHandler, EntityInputHandler, FocusHandle,
    Focusable, Hsla, IntoElement, KeyDownEvent, KeyUpEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, Render, ScrollWheelEvent, SharedString,
    StrikethroughStyle, TextRun, UTF16Selection, UnderlineStyle, Window, canvas, div, fill, font,
    outline, point, prelude::*, px, rgb, size,
};
use std::ops::Range;
use terminal::{
    KeyAction, KeyPress, TerminalCellWidth, TerminalDamageFrame, TerminalRenderer, TerminalSession,
    bracketed_paste_bytes, key_press_from_parts, normalize_paste, paste_needs_confirmation,
};

use crate::tokens::Tokens;

/// Snap a logical-pixel coordinate to the nearest physical pixel on this
/// display.  Prevents fractional pixel drift that causes uneven character
/// spacing in the terminal grid.
fn snap_px(value: f32, scale: f32) -> f32 {
    (value * scale).round() / scale
}

/// Snap a logical-pixel length to the nearest physical pixel on this display.
fn snap_len(value: f32, scale: f32) -> f32 {
    (value * scale).round() / scale
}

fn is_terminal_graphics_text(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(
            ch as u32,
            0x2500..=0x259F // box drawing + block elements
                | 0x2800..=0x28FF // braille patterns
                | 0xE000..=0xF8FF // private use (Nerd Font / Powerline)
        )
    })
}

/// Non-blocking scrollback search state (highlights painted as overlay rects).
#[derive(Clone, Debug, Default)]
pub struct TerminalSearchState {
    pub query: String,
    pub match_rows: Vec<usize>,
    pub active_match: usize,
}

/// Cached style data for a single terminal cell. Glyphs are shaped
/// individually and painted at exact snapped grid positions — cell
/// layout is never delegated to the text engine.
struct CellPaint {
    text: String,
    fg: u32,
    width: TerminalCellWidth,
    bold: bool,
    italic: bool,
    faint: bool,
    underline: bool,
    strikethrough: bool,
}

/// Cached paint data for a single terminal row.
struct RowPaint {
    cells: Vec<CellPaint>,
    /// Background fills as `(start_col, span_len, rgb)` for non-default
    /// cells. Painted as quad overlays before text.
    bg_spans: Vec<(usize, usize, u32)>,
}

#[derive(Clone, Copy, Debug)]
struct TerminalMetrics {
    scale: f32,
    font_size: f32,
    adv_0: f32,
    adv_m: f32,
    adv_i: f32,
    adv_w: f32,
    adv_space: f32,
    cell_width: f32,
    cell_height: f32,
}

/// Drag selection in grid coordinates.
#[derive(Clone, Copy, Debug)]
struct TerminalSelection {
    anchor_col: usize,
    anchor_row: usize,
    end_col: usize,
    end_row: usize,
}

pub struct TerminalView {
    session: Option<Arc<TerminalSession>>,
    renderer: TerminalRenderer,
    render_generation: usize,
    cols: u16,
    rows: u16,
    content_width: f32,
    content_height: f32,
    cell_width: f32,
    cell_height: f32,
    cell_metrics_ready: bool,
    resized_cell_w: u32,
    resized_cell_h: u32,
    focus_handle: FocusHandle,
    listener_started: bool,
    flush_scheduled: bool,
    row_cache: Vec<Option<RowPaint>>,
    scroll_frozen: bool,
    metrics: Option<TerminalMetrics>,
    pub search: TerminalSearchState,
    selection: Option<TerminalSelection>,
    selecting: bool,
    paste_pending: Option<String>,
    bracketed_paste: bool,
}

impl TerminalView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            session: None,
            renderer: TerminalRenderer::default(),
            render_generation: 0,
            cols: 80,
            rows: 24,
            content_width: 640.0,
            content_height: 400.0,
            cell_width: Tokens::TERMINAL_CELL_WIDTH,
            cell_height: Tokens::TERMINAL_CELL_HEIGHT,
            cell_metrics_ready: false,
            resized_cell_w: 0,
            resized_cell_h: 0,
            focus_handle: cx.focus_handle(),
            listener_started: false,
            flush_scheduled: false,
            row_cache: Vec::new(),
            scroll_frozen: false,
            metrics: None,
            search: TerminalSearchState::default(),
            selection: None,
            selecting: false,
            paste_pending: None,
            bracketed_paste: true,
        }
    }

    pub fn attach_session(&mut self, session: Arc<TerminalSession>, cx: &mut Context<Self>) {
        let session_changed = !self
            .session
            .as_ref()
            .is_some_and(|existing| Arc::ptr_eq(existing, &session));
        if !session_changed && self.listener_started {
            return;
        }
        if session_changed {
            self.listener_started = false;
            self.renderer = TerminalRenderer::default();
            self.row_cache.clear();
        }
        self.session = Some(session.clone());
        self.drain_frames();
        self.start_frame_listener(session, cx);
        self.schedule_flush(cx);
    }

    pub fn detach_session(&mut self, cx: &mut Context<Self>) {
        self.session = None;
        self.renderer = TerminalRenderer::default();
        self.row_cache.clear();
        self.listener_started = false;
        cx.notify();
    }

    pub fn set_content_size(&mut self, width: f32, height: f32, cx: &mut Context<Self>) {
        self.content_width = width.max(1.0);
        self.content_height = height.max(1.0);
        self.recompute_grid(cx);
    }

    pub fn set_search(&mut self, search: TerminalSearchState, cx: &mut Context<Self>) {
        self.search = search;
        self.render_generation = self.render_generation.wrapping_add(1);
        cx.notify();
    }

    pub fn update_search_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.search.query = query.clone();
        self.search.match_rows.clear();
        self.search.active_match = 0;
        if !query.is_empty() {
            for row in 0..self.renderer.rows() {
                let line: String = self
                    .renderer
                    .row_cells(row)
                    .iter()
                    .map(|c| {
                        if c.text.is_empty() || c.width.is_spacer() {
                            " "
                        } else {
                            c.text.as_str()
                        }
                    })
                    .collect();
                if line.contains(&query) {
                    self.search.match_rows.push(row);
                }
            }
        }
        self.render_generation = self.render_generation.wrapping_add(1);
        cx.notify();
    }

    pub fn show_jump_to_latest(&self) -> bool {
        self.scroll_frozen || !self.renderer.scrollback_at_bottom
    }

    pub fn jump_to_latest(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = &self.session {
            session.scroll_viewport_to_bottom();
        }
        self.scroll_frozen = false;
        cx.notify();
    }

    fn recompute_grid(&mut self, cx: &mut Context<Self>) {
        let cw = self.cell_width.max(1.0);
        let ch = self.cell_height.max(1.0);
        let cols = (self.content_width / cw).floor().max(2.0) as u16;
        let rows = (self.content_height / ch).floor().max(1.0) as u16;
        let scale = self.metrics.map(|m| m.scale).unwrap_or(1.0);
        let cell_w_px = (cw * scale).round().max(1.0) as u32;
        let cell_h_px = (ch * scale).round().max(1.0) as u32;
        if self.cols == cols
            && self.rows == rows
            && self.resized_cell_w == cell_w_px
            && self.resized_cell_h == cell_h_px
        {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        self.resized_cell_w = cell_w_px;
        self.resized_cell_h = cell_h_px;
        // Geometry changed: cached row layouts no longer line up.
        self.row_cache.clear();
        if let Some(session) = &self.session {
            session.resize(cols, rows, cell_w_px, cell_h_px);
        }
        if let Some(metrics) = self.metrics {
            tracing::info!(
                "terminal resize: cols={} rows={} content_width={} content_height={} scale={} font_size={} adv_0={} adv_M={} adv_i={} adv_W={} adv_space={} cell_width={} cell_height={}",
                cols,
                rows,
                self.content_width,
                self.content_height,
                metrics.scale,
                metrics.font_size,
                metrics.adv_0,
                metrics.adv_m,
                metrics.adv_i,
                metrics.adv_w,
                metrics.adv_space,
                metrics.cell_width,
                metrics.cell_height,
            );
        }
        cx.notify();
    }

    fn ensure_cell_metrics(&mut self, window: &mut Window, cx: &mut App) -> bool {
        if self.cell_metrics_ready {
            return false;
        }
        let metrics = Self::measure_cell_metrics(window, cx);
        self.cell_width = metrics.cell_width;
        self.cell_height = metrics.cell_height;
        self.metrics = Some(metrics);
        self.cell_metrics_ready = true;
        self.row_cache.clear();
        true
    }

    fn measure_cell_metrics(window: &mut Window, _cx: &mut App) -> TerminalMetrics {
        let scale = window.scale_factor();
        let font_size = Tokens::terminal_font_size();
        let font_family = Tokens::terminal_font_family();

        let adv_0 = Self::measure_terminal_advance(window, font_family, font_size, "0", scale);
        let adv_m = Self::measure_terminal_advance(window, font_family, font_size, "M", scale);
        let adv_i = Self::measure_terminal_advance(window, font_family, font_size, "i", scale);
        let adv_w = Self::measure_terminal_advance(window, font_family, font_size, "W", scale);
        let adv_space = Self::measure_terminal_advance(window, font_family, font_size, " ", scale);

        let height_probe =
            Self::shape_terminal_sample(window, font_family, font_size, "0000000000");
        let raw_height = f32::from(height_probe.ascent) + f32::from(height_probe.descent);
        let cell_height = snap_len(raw_height.ceil().max(16.0), scale);

        // Prefer the terminal-safe digit advance as the canonical cell width.
        let cell_width = snap_len(adv_0.max(1.0), scale);

        let metrics = TerminalMetrics {
            scale,
            font_size: f32::from(font_size),
            adv_0,
            adv_m,
            adv_i,
            adv_w,
            adv_space,
            cell_width,
            cell_height,
        };

        tracing::info!(
            "terminal metrics: scale={} font_size={} adv_0={} adv_M={} adv_i={} adv_W={} adv_space={} cell_width={} cell_height={}",
            metrics.scale,
            metrics.font_size,
            metrics.adv_0,
            metrics.adv_m,
            metrics.adv_i,
            metrics.adv_w,
            metrics.adv_space,
            metrics.cell_width,
            metrics.cell_height,
        );

        Self::verify_monospace_metrics(metrics);
        metrics
    }

    fn shape_terminal_sample(
        window: &mut Window,
        font_family: &'static str,
        font_size: Pixels,
        sample: &str,
    ) -> gpui::ShapedLine {
        let sample = SharedString::from(sample.to_string());
        let runs = vec![TextRun {
            len: sample.len(),
            font: font(font_family),
            color: Tokens::text_primary(),
            background_color: None,
            underline: Default::default(),
            strikethrough: Default::default(),
        }];
        window
            .text_system()
            .shape_line(sample, font_size, &runs, None)
    }

    fn measure_terminal_advance(
        window: &mut Window,
        font_family: &'static str,
        font_size: Pixels,
        glyph: &str,
        scale: f32,
    ) -> f32 {
        let shaped = Self::shape_terminal_sample(window, font_family, font_size, &glyph.repeat(10));
        let raw_advance = f32::from(shaped.width) / 10.0;
        snap_len(raw_advance, scale)
    }

    /// Diagnostics: ensure the active terminal font produces equal advances
    /// for narrow and wide ASCII glyphs. Logs a warning if the measured
    /// advances do not match the cell width closely enough for a stable grid.
    fn verify_monospace_metrics(metrics: TerminalMetrics) {
        for (glyph, advance) in [
            ("M", metrics.adv_m),
            ("W", metrics.adv_w),
            ("i", metrics.adv_i),
            (" ", metrics.adv_space),
        ] {
            if (advance - metrics.cell_width).abs() > 0.5 {
                tracing::warn!(
                    "Terminal font may not be monospace in paint units: glyph '{}' advance {:.2} != cell_width {:.2}. Check font fallback chain, ligatures, and metric normalization.",
                    glyph,
                    advance,
                    metrics.cell_width,
                );
            }
        }
    }

    /// Apply one damage frame into the retained grid and track scroll freeze.
    fn apply_frame(&mut self, frame: &TerminalDamageFrame) {
        if !frame.scrollback_at_bottom {
            self.scroll_frozen = true;
        } else {
            // User scrolled back to the bottom (or terminal output caught up).
            self.scroll_frozen = false;
        }
        self.renderer.apply_damage_frame(frame);
    }

    /// Drain every frame currently queued on the session and apply each in order.
    /// Damage frames are cumulative, so none may be dropped or coalesced.
    fn drain_frames(&mut self) -> bool {
        let mut changed = false;
        if let Some(session) = self.session.clone() {
            while let Some(frame) = session.try_recv_frame() {
                self.apply_frame(&frame);
                changed = true;
            }
        }
        changed
    }

    /// Coalesce repaints: schedule a single deferred `notify`, regardless of how
    /// many frames were applied since the last paint.
    fn schedule_flush(&mut self, cx: &mut Context<Self>) {
        if self.flush_scheduled {
            return;
        }
        self.flush_scheduled = true;
        let entity = cx.entity();
        cx.defer(move |cx| {
            entity.update(cx, |view, cx| {
                view.flush_scheduled = false;
                view.drain_frames();
                view.render_generation = view.render_generation.wrapping_add(1);
                cx.notify();
            });
        });
    }

    fn start_frame_listener(&mut self, session: Arc<TerminalSession>, cx: &mut Context<Self>) {
        if self.listener_started {
            return;
        }
        self.listener_started = true;
        let rx = session.frame_notifications();
        cx.spawn(async move |entity, cx| {
            while let Ok(frame) = rx.recv_async().await {
                let _ = entity.update(cx, |view, cx| {
                    view.apply_frame(&frame);
                    view.drain_frames();
                    view.schedule_flush(cx);
                });
            }
        })
        .detach();
    }

    fn key_press_from_down(event: &KeyDownEvent) -> KeyPress {
        let k = &event.keystroke;
        key_press_from_parts(
            k.key.clone(),
            k.modifiers.shift,
            k.modifiers.alt,
            k.modifiers.control,
            k.modifiers.platform,
            KeyAction::Press,
            k.key_char.clone(),
        )
    }

    fn key_press_from_up(event: &KeyUpEvent) -> KeyPress {
        let k = &event.keystroke;
        key_press_from_parts(
            k.key.clone(),
            k.modifiers.shift,
            k.modifiers.alt,
            k.modifiers.control,
            k.modifiers.platform,
            KeyAction::Release,
            k.key_char.clone(),
        )
    }

    fn is_special_key(key: &str) -> bool {
        matches!(
            key,
            "enter"
                | "return"
                | "tab"
                | "backspace"
                | "escape"
                | "delete"
                | "space"
                | "up"
                | "down"
                | "left"
                | "right"
                | "home"
                | "end"
                | "pageup"
                | "pagedown"
                | "f1"
                | "f2"
                | "f3"
                | "f4"
                | "f5"
                | "f6"
                | "f7"
                | "f8"
                | "f9"
                | "f10"
                | "f11"
                | "f12"
        )
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let k = &event.keystroke;
        // Printable text is delivered via EntityInputHandler (insertText) on macOS,
        // but special keys like Enter must always go to the PTY even when key_char is set.
        if !Self::is_special_key(&k.key)
            && k.key_char.is_some()
            && !k.modifiers.control
            && !k.modifiers.alt
            && !k.modifiers.function
        {
            return;
        }
        if let Some(session) = &self.session {
            session.send_key(Self::key_press_from_down(event));
            cx.stop_propagation();
        }
    }

    fn on_key_up(&mut self, event: &KeyUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let k = &event.keystroke;
        if !Self::is_special_key(&k.key)
            && k.key_char.is_some()
            && !k.modifiers.control
            && !k.modifiers.alt
            && !k.modifiers.function
        {
            return;
        }
        if let Some(session) = &self.session {
            session.send_key(Self::key_press_from_up(event));
            cx.stop_propagation();
        }
    }

    fn send_typed_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(session) = &self.session {
            let bytes: Vec<u8> = text
                .bytes()
                .map(|b| if b == b'\n' { b'\r' } else { b })
                .collect();
            session.send_bytes(bytes);
        }
    }

    fn paste_text(&mut self, text: String, cx: &mut Context<Self>) {
        let normalized = normalize_paste(&text);
        if paste_needs_confirmation(&normalized) {
            self.paste_pending = Some(normalized);
            cx.notify();
            return;
        }
        self.send_paste(normalized);
    }

    fn send_paste(&mut self, text: String) {
        if let Some(session) = &self.session {
            let bytes = if self.bracketed_paste {
                bracketed_paste_bytes(&text)
            } else {
                text.into_bytes()
            };
            session.send_bytes(bytes);
        }
    }

    fn confirm_paste(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = self.paste_pending.take() {
            self.send_paste(text);
            cx.notify();
        }
    }

    fn paint_grid(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let scale = window.scale_factor();
        let cw = px(self.cell_width);
        let ch = px(self.cell_height);
        // Snap the grid origin to physical pixels. Every cell position,
        // background, cursor, and selection rect uses the same snapped
        // coordinate system so they never drift apart.
        let origin_x = snap_px(f32::from(bounds.origin.x), scale);
        let origin_y = snap_px(f32::from(bounds.origin.y), scale);

        let theme = Tokens::terminal_theme();
        let default_fg = theme.foreground;
        let default_bg = theme.background;
        let terminal_bg = rgb(theme.background);
        let _ = window.paint_quad(fill(bounds, terminal_bg));

        if self.renderer.cols() == 0 {
            return;
        }

        let font_size = Tokens::terminal_font_size();
        let rows = self.renderer.rows();
        if self.row_cache.len() != rows {
            self.row_cache.clear();
            self.row_cache.resize_with(rows, || None);
        }

        let full = self.renderer.needs_full_paint();
        let cw_f = f32::from(cw);
        for row in 0..rows {
            if full || self.renderer.row_dirty(row) || self.row_cache[row].is_none() {
                let painted = build_row_paint(
                    self.renderer.row_cells(row),
                    default_fg,
                    default_bg,
                    font_size,
                    window,
                );
                self.row_cache[row] = Some(painted);
            }
            // Re-borrow immutably after the mutable borrow from the `if` block
            // above has ended, so the borrow checker is happy.
            let cache_entry = self.row_cache[row].as_ref();
            let Some(painted) = cache_entry else {
                continue;
            };
            let y = snap_px(origin_y + f32::from(ch) * row as f32, scale);

            // 1. Paint background fills (quad overlays behind text).
            for &(start, len, color) in &painted.bg_spans {
                let bg_x = snap_px(origin_x + cw_f * start as f32, scale);
                let bg_bounds =
                    Bounds::new(point(px(bg_x), px(y)), size(px(cw_f * len as f32), ch));
                let _ = window.paint_quad(fill(bg_bounds, rgb(color)));
            }

            // 2. Text — paint anchored style runs at the terminal grid so any
            // metric mismatch cannot accumulate across the full row.
            Self::paint_row_runs(
                &painted.cells,
                point(px(origin_x), px(y)),
                cw,
                ch,
                font_size,
                window,
                cx,
            );
        }

        // 3. Paint cursor as a hollow blue outline over the active cell.
        if self.renderer.cursor_visible {
            if let (Some(col), Some(row)) = (self.renderer.cursor_col, self.renderer.cursor_row) {
                let row = row as usize;
                let col = col as usize;
                let cur_x = snap_px(origin_x + cw_f * col as f32, scale);
                let cur_y = snap_px(origin_y + f32::from(ch) * row as f32, scale);
                let cursor_origin = point(px(cur_x), px(cur_y));
                let cursor_bounds = Bounds::new(cursor_origin, size(cw, ch));
                let _ = window.paint_quad(outline(
                    cursor_bounds,
                    Tokens::info(),
                    BorderStyle::default(),
                ));
            }
        }

        // 4. Search highlights.
        for &row in &self.search.match_rows {
            if row >= self.renderer.rows() {
                continue;
            }
            let hl_y = snap_px(origin_y + f32::from(ch) * row as f32, scale);
            let highlight = Bounds::new(
                point(px(origin_x), px(hl_y)),
                size(px(f32::from(bounds.size.width)), ch),
            );
            let _ = window.paint_quad(fill(highlight, Tokens::accent().alpha(0.15)));
        }

        // 5. Selection highlights.
        if let Some(sel) = self.selection {
            let (r0, r1, c0, c1) = selection_bounds(sel);
            for row in r0..=r1 {
                let x0 = if row == r0 { c0 } else { 0 };
                let x1 = if row == r1 {
                    c1
                } else {
                    self.renderer.cols().saturating_sub(1)
                };
                let sel_x = snap_px(origin_x + cw_f * x0 as f32, scale);
                let sel_y = snap_px(origin_y + f32::from(ch) * row as f32, scale);
                let sel_bounds = Bounds::new(
                    point(px(sel_x), px(sel_y)),
                    size(px(cw_f * (x1 - x0 + 1) as f32), ch),
                );
                let _ = window.paint_quad(fill(sel_bounds, Tokens::accent().alpha(0.25)));
            }
        }

        self.renderer.clear_dirty();
    }

    fn paint_cell_text(
        text: &str,
        origin: Point<Pixels>,
        ch: Pixels,
        fg: u32,
        bold: bool,
        italic: bool,
        faint: bool,
        underline: bool,
        strikethrough: bool,
        font_size: Pixels,
        use_symbol_font: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mut cell_font = if use_symbol_font {
            font(Tokens::terminal_symbol_font_family())
        } else {
            font(Tokens::terminal_font_family())
        };
        if bold {
            cell_font = cell_font.bold();
        }
        if italic {
            cell_font = cell_font.italic();
        }
        let mut run_color: Hsla = rgb(fg).into();
        if faint {
            run_color = Hsla {
                a: run_color.a * Tokens::terminal_theme().dim_opacity,
                ..run_color
            };
        }
        let underline_px = px(1.0);
        let ul = if underline {
            Some(UnderlineStyle {
                thickness: underline_px,
                color: Some(run_color),
                wavy: false,
            })
        } else {
            Default::default()
        };
        let st = if strikethrough {
            Some(StrikethroughStyle {
                thickness: underline_px,
                color: Some(run_color),
            })
        } else {
            Default::default()
        };
        let runs = vec![TextRun {
            len: text.len(),
            font: cell_font,
            color: run_color,
            background_color: None,
            underline: ul,
            strikethrough: st,
        }];
        let shaped = window.text_system().shape_line(
            SharedString::from(text.to_string()),
            font_size,
            &runs,
            None,
        );
        let _ = shaped.paint(origin, ch, window, cx);
    }

    fn paint_row_runs(
        cells: &[CellPaint],
        origin: Point<Pixels>,
        cell_width: Pixels,
        line_height: Pixels,
        font_size: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) {
        if cells.is_empty() {
            return;
        }
        let mut col = 0usize;
        while col < cells.len() {
            let cell = &cells[col];
            if cell.width.is_spacer() {
                col += 1;
                continue;
            }
            if is_terminal_graphics_text(&cell.text) {
                let cell_origin = point(origin.x + cell_width * col as f32, origin.y);
                Self::paint_cell_text(
                    &cell.text,
                    cell_origin,
                    line_height,
                    cell.fg,
                    cell.bold,
                    cell.italic,
                    cell.faint,
                    cell.underline,
                    cell.strikethrough,
                    font_size,
                    true,
                    window,
                    cx,
                );
                col += 1;
                continue;
            }
            let mut run_len = 1usize;
            while col + run_len < cells.len() {
                let next = &cells[col + run_len];
                if next.width.is_spacer() || is_terminal_graphics_text(&next.text) {
                    break;
                }
                if next.fg != cell.fg
                    || next.bold != cell.bold
                    || next.italic != cell.italic
                    || next.faint != cell.faint
                    || next.underline != cell.underline
                    || next.strikethrough != cell.strikethrough
                {
                    break;
                }
                run_len += 1;
            }

            let mut run_text = String::new();
            for run_cell in &cells[col..col + run_len] {
                if run_cell.width.is_spacer() || run_cell.text.is_empty() {
                    run_text.push(' ');
                } else {
                    run_text.push_str(&run_cell.text);
                }
            }

            let mut run_font = font(Tokens::terminal_font_family());
            if cell.bold {
                run_font = run_font.bold();
            }
            if cell.italic {
                run_font = run_font.italic();
            }
            let mut run_color: Hsla = rgb(cell.fg).into();
            if cell.faint {
                run_color = Hsla {
                    a: run_color.a * Tokens::terminal_theme().dim_opacity,
                    ..run_color
                };
            }
            let underline_px = px(1.0);
            let underline = if cell.underline {
                Some(UnderlineStyle {
                    thickness: underline_px,
                    color: Some(run_color),
                    wavy: false,
                })
            } else {
                Default::default()
            };
            let strikethrough = if cell.strikethrough {
                Some(StrikethroughStyle {
                    thickness: underline_px,
                    color: Some(run_color),
                })
            } else {
                Default::default()
            };
            let runs = vec![TextRun {
                len: run_text.len(),
                font: run_font,
                color: run_color,
                background_color: None,
                underline,
                strikethrough,
            }];
            let shaped = window.text_system().shape_line(
                SharedString::from(run_text),
                font_size,
                &runs,
                None,
            );
            let run_origin = point(origin.x + cell_width * col as f32, origin.y);
            let _ = shaped.paint(run_origin, line_height, window, cx);
            col += run_len;
        }
    }
}

/// Build cached paint data for a single row: per-cell style extraction
/// and background-fill spans. Text is painted cell-by-cell at snapped
/// grid positions; the text engine never controls column layout.
fn build_row_paint(
    cells: &[terminal::TerminalCell],
    default_fg: u32,
    default_bg: u32,
    _font_size: Pixels,
    _window: &mut Window,
) -> RowPaint {
    let mut bg_spans: Vec<(usize, usize, u32)> = Vec::new();
    let mut col = 0usize;
    while col < cells.len() {
        let color = cells[col].bg.unwrap_or(default_bg);
        if color == default_bg {
            col += 1;
            continue;
        }
        let start = col;
        let mut len = 1usize;
        while start + len < cells.len() && cells[start + len].bg.unwrap_or(default_bg) == color {
            len += 1;
        }
        bg_spans.push((start, len, color));
        col += len;
    }

    let cell_paints: Vec<CellPaint> = cells
        .iter()
        .map(|cell| CellPaint {
            text: cell.text.clone(),
            fg: cell.fg.unwrap_or(default_fg),
            width: cell.width,
            bold: cell.bold,
            italic: cell.italic,
            faint: cell.faint,
            underline: cell.underline,
            strikethrough: cell.strikethrough,
        })
        .collect();

    RowPaint {
        cells: cell_paints,
        bg_spans,
    }
}

fn selection_bounds(sel: TerminalSelection) -> (usize, usize, usize, usize) {
    let (r0, r1) = if sel.anchor_row <= sel.end_row {
        (sel.anchor_row, sel.end_row)
    } else {
        (sel.end_row, sel.anchor_row)
    };
    let (c0, c1) = if sel.anchor_row == sel.end_row && sel.anchor_col > sel.end_col {
        (sel.end_col, sel.anchor_col)
    } else if sel.anchor_row == sel.end_row {
        (sel.anchor_col, sel.end_col)
    } else if sel.anchor_row < sel.end_row {
        (sel.anchor_col, sel.end_col)
    } else {
        (sel.end_col, sel.anchor_col)
    };
    (r0, r1, c0, c1)
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        None
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        self.send_typed_text(text);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        _: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_text_in_range(range, new_text, window, cx);
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let col = self.renderer.cursor_col? as f32;
        let row = self.renderer.cursor_row? as f32;
        Some(Bounds::new(
            point(
                element_bounds.origin.x + px(self.cell_width * col),
                element_bounds.origin.y + px(self.cell_height * row),
            ),
            size(px(self.cell_width), px(self.cell_height)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let entity_wheel = entity.clone();
        let entity_canvas = entity.clone();
        let entity_down = entity.clone();
        let entity_up = entity.clone();
        let entity_move = entity.clone();

        if let Some(text) = self.paste_pending.clone() {
            let confirm_paste = entity.clone();
            let confirm_cancel = entity.clone();
            return div()
                .id("terminal-paste-confirm")
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(Tokens::spacing_2())
                .bg(Tokens::main_bg())
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .text_color(Tokens::text_primary())
                        .child("Paste contains control characters. Paste anyway?"),
                )
                .child(
                    div()
                        .flex()
                        .gap(Tokens::spacing_2())
                        .child(
                            div()
                                .px(Tokens::spacing_2())
                                .rounded(Tokens::radius_xs())
                                .bg(Tokens::accent())
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    confirm_paste.update(cx, |view, cx| view.confirm_paste(cx));
                                })
                                .child("Paste"),
                        )
                        .child(
                            div()
                                .px(Tokens::spacing_2())
                                .rounded(Tokens::radius_xs())
                                .hover(|s| s.bg(Tokens::surface_hover()))
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    confirm_cancel.update(cx, |view, cx| {
                                        view.paste_pending = None;
                                        cx.notify();
                                    });
                                })
                                .child("Cancel"),
                        ),
                )
                .child(
                    div()
                        .text_size(Tokens::text_xs())
                        .text_color(Tokens::text_tertiary())
                        .max_w(px(400.0))
                        .child(text.chars().take(120).collect::<String>()),
                )
                .into_any_element();
        }

        div()
            .id("terminal-view")
            .size_full()
            .min_h(px(0.0))
            .bg(rgb(Tokens::terminal_theme().background))
            .key_context("Terminal")
            .track_focus(&self.focus_handle)
            .on_mouse_down(
                MouseButton::Left,
                move |event: &MouseDownEvent, window, cx| {
                    entity_down.update(cx, |view, cx| {
                        view.focus_handle.focus(window);
                        view.selecting = true;
                        if let Some((col, row)) = view.cell_at_position(Self::cell_at_down(event)) {
                            view.selection = Some(TerminalSelection {
                                anchor_col: col,
                                anchor_row: row,
                                end_col: col,
                                end_row: row,
                            });
                        }
                        cx.notify();
                    });
                },
            )
            .on_mouse_up(MouseButton::Left, move |_: &MouseUpEvent, _, cx| {
                entity_up.update(cx, |view, cx| {
                    view.selecting = false;
                    if let Some(sel) = view.selection {
                        let text = view.selection_text(sel);
                        if !text.is_empty() {
                            cx.write_to_clipboard(text.into());
                        }
                    }
                });
            })
            .on_mouse_move(move |event: &MouseMoveEvent, _, cx| {
                entity_move.update(cx, |view, cx| {
                    if !view.selecting {
                        return;
                    }
                    if let Some((col, row)) = view.cell_at_position(Self::cell_at_move(event)) {
                        if let Some(sel) = view.selection.as_mut() {
                            sel.end_col = col;
                            sel.end_row = row;
                            cx.notify();
                        }
                    }
                });
            })
            .on_key_down(cx.listener(Self::on_key_down))
            .on_key_up(cx.listener(Self::on_key_up))
            .on_scroll_wheel(move |event: &ScrollWheelEvent, _, cx| {
                entity_wheel.update(cx, |view, _| {
                    let ch = view.cell_height;
                    let delta = event.delta.pixel_delta(px(ch));
                    let lines = (delta.y / px(ch)).round() as isize;
                    if lines != 0 {
                        if let Some(session) = &view.session {
                            session.scroll_viewport(-lines);
                            view.scroll_frozen = true;
                        }
                    }
                });
            })
            .child({
                let entity_paint = entity_canvas.clone();
                canvas(
                    {
                        let entity_paint = entity_paint.clone();
                        move |_bounds, _window, cx| entity_paint.read(cx).render_generation
                    },
                    move |bounds, _gen, window, cx| {
                        let entity = entity_paint.clone();
                        let focus_handle = entity.read(cx).focus_handle.clone();
                        window.handle_input(
                            &focus_handle,
                            ElementInputHandler::new(bounds, entity.clone()),
                            cx,
                        );
                        entity.update(cx, |view, cx| {
                            let w = f32::from(bounds.size.width);
                            let h = f32::from(bounds.size.height);
                            if view.ensure_cell_metrics(window, cx) {
                                view.recompute_grid(cx);
                            }
                            if (view.content_width - w).abs() > 0.5
                                || (view.content_height - h).abs() > 0.5
                            {
                                view.set_content_size(w, h, cx);
                            }
                            view.paint_grid(bounds, window, cx);
                        });
                    },
                )
                .size_full()
            })
            .into_any_element()
    }
}

impl TerminalView {
    fn cell_at_position(&self, pos: Point<Pixels>) -> Option<(usize, usize)> {
        let col = (f32::from(pos.x) / self.cell_width).floor() as usize;
        let row = (f32::from(pos.y) / self.cell_height).floor() as usize;
        if col < self.renderer.cols() && row < self.renderer.rows() {
            Some((col, row))
        } else {
            None
        }
    }

    fn cell_at_down(event: &MouseDownEvent) -> Point<Pixels> {
        event.position
    }

    fn cell_at_move(event: &MouseMoveEvent) -> Point<Pixels> {
        event.position
    }

    fn selection_text(&self, sel: TerminalSelection) -> String {
        let (r0, r1, c0, c1) = selection_bounds(sel);
        let mut out = String::new();
        for row in r0..=r1 {
            let cells = self.renderer.row_cells(row);
            let start = if row == r0 { c0 } else { 0 };
            let end = if row == r1 {
                c1.min(cells.len().saturating_sub(1))
            } else {
                cells.len().saturating_sub(1)
            };
            for cell in &cells[start..=end.min(cells.len().saturating_sub(1))] {
                out.push_str(if cell.text.is_empty() || cell.width.is_spacer() {
                    " "
                } else {
                    &cell.text
                });
            }
            if row < r1 {
                out.push('\n');
            }
        }
        out
    }
}
