//! Centralised design tokens for the Vortex agentic UI.
//!
//! Colours resolve from the active theme palette (see `theme.rs`).
//! Layout and typography follow the Vortex visual spec (Cursor/Zed-like).

#![allow(dead_code)]

use gpui::{Background, ColorSpace, Hsla, linear_color_stop, linear_gradient, px};

use super::theme::active_palette;

pub struct Tokens;

// ════════════════════════════════════════════════════════════
//  COLOUR PALETTE  (theme-backed)
// ════════════════════════════════════════════════════════════

macro_rules! palette_color {
    ($field:ident) => {
        pub fn $field() -> Hsla {
            active_palette().$field
        }
    };
}

impl Tokens {
    palette_color!(app_bg);
    palette_color!(panel_bg);
    palette_color!(main_bg);
    palette_color!(surface);
    palette_color!(surface_hover);
    palette_color!(surface_active);
    palette_color!(surface_overlay);
    palette_color!(chrome);
    palette_color!(input_bg);
    palette_color!(diff_bg);

    pub fn surface_overlay_alias() -> Hsla {
        Self::surface_overlay()
    }

    pub fn bg() -> Hsla {
        Self::app_bg()
    }
    pub fn sidebar_bg() -> Hsla {
        active_palette().sidebar_bg
    }
    pub fn raised_card() -> Hsla {
        Self::surface()
    }
    pub fn popover() -> Hsla {
        Self::surface_overlay()
    }
    pub fn composer_bg() -> Hsla {
        Self::input_bg().alpha(1.0)
    }

    /// Thread scrim above the composer — fully clear at the top, strong fade near the input.
    pub fn composer_fade_gradient() -> Background {
        let bg = Self::main_bg();
        linear_gradient(
            180.,
            linear_color_stop(bg.alpha(0.0), 0.0),
            linear_color_stop(bg.alpha(0.88), 1.0),
        )
        .color_space(ColorSpace::Oklab)
    }

    /// Footer backdrop behind metadata / pill / chips — gradient, not a flat fill.
    /// Transparent at the top edge, solid main background by the input pill.
    pub fn composer_footer_gradient() -> Background {
        let bg = Self::main_bg();
        linear_gradient(
            180.,
            linear_color_stop(bg.alpha(0.0), 0.0),
            linear_color_stop(bg, 0.55),
        )
        .color_space(ColorSpace::Oklab)
    }
}

impl Tokens {
    palette_color!(border);
    palette_color!(border_subtle);
    palette_color!(divider);
    palette_color!(border_strong);
    palette_color!(border_focus);

    pub fn sidebar_border() -> Hsla {
        active_palette().sidebar_border
    }
}

impl Tokens {
    palette_color!(text_primary);
    palette_color!(text_secondary);
    palette_color!(text_tertiary);
    palette_color!(text_faint);
    palette_color!(text_bright);
    palette_color!(sidebar_text);
    palette_color!(sidebar_text_hover);
    palette_color!(sidebar_text_muted);

    pub fn text() -> Hsla {
        Self::text_primary()
    }
    pub fn text_muted() -> Hsla {
        Self::text_tertiary()
    }
    pub fn text_subtle() -> Hsla {
        Self::text_faint()
    }
}

impl Tokens {
    palette_color!(accent);
    palette_color!(accent_hover);
    palette_color!(accent_dim);
}

impl Tokens {
    palette_color!(success);
    palette_color!(warning);
    palette_color!(danger);
    palette_color!(info);

    pub fn green() -> Hsla {
        Self::success()
    }
    pub fn red() -> Hsla {
        Self::danger()
    }
    pub fn blue() -> Hsla {
        Self::info()
    }
    pub fn code_fg() -> Hsla {
        active_palette().code_fg
    }
    pub fn code_bg() -> Hsla {
        active_palette().code_bg
    }
}

// ── Diff-specific colours (theme-aware) ────────────────────

impl Tokens {
    pub fn diff_code_normal() -> Hsla {
        active_palette().code_fg
    }
    pub fn diff_code_muted() -> Hsla {
        Self::text_tertiary()
    }
    pub fn diff_line_number() -> Hsla {
        active_palette().diff_line_number
    }
    pub fn diff_line_number_active() -> Hsla {
        Self::text_secondary()
    }
    pub fn diff_path_text() -> Hsla {
        Self::text_secondary()
    }
    pub fn diff_tab_active_bg() -> Hsla {
        Self::surface_active()
    }
    pub fn diff_tab_inactive_text() -> Hsla {
        Self::text_tertiary()
    }
    pub fn diff_tab_active_text() -> Hsla {
        Self::text_primary()
    }
    pub fn diff_panel_border() -> Hsla {
        Self::border_subtle()
    }
    pub fn diff_header_border() -> Hsla {
        Self::border_subtle()
    }

    pub fn diff_add_bg() -> Hsla {
        active_palette().diff_add_bg
    }
    pub fn diff_add_text() -> Hsla {
        active_palette().diff_add_text
    }
    pub fn diff_add_indicator() -> Hsla {
        active_palette().diff_add_indicator
    }

    pub fn diff_del_bg() -> Hsla {
        active_palette().diff_del_bg
    }
    pub fn diff_del_text() -> Hsla {
        active_palette().diff_del_text
    }
    pub fn diff_del_indicator() -> Hsla {
        active_palette().diff_del_indicator
    }

    pub fn diff_hunk_bg() -> Hsla {
        Self::surface_hover()
    }
    pub fn diff_hunk_text() -> Hsla {
        Self::text_faint()
    }

    pub fn tool_path_text() -> Hsla {
        Self::text_tertiary()
    }
    pub fn tool_name_text() -> Hsla {
        Self::text_secondary()
    }
    pub fn timeline_border() -> Hsla {
        Self::divider()
    }
    pub fn sidebar_hover_bg() -> Hsla {
        Self::surface_hover().blend(Self::accent().opacity(0.025))
    }
    pub fn sidebar_selected_bg() -> Hsla {
        Self::surface_active().blend(Self::accent().opacity(0.045))
    }
    pub fn sidebar_time_fade_gradient(bg: Hsla) -> Background {
        linear_gradient(
            90.,
            linear_color_stop(bg.alpha(0.0), 0.0),
            linear_color_stop(bg.alpha(0.92), 1.0),
        )
        .color_space(ColorSpace::Oklab)
    }
    pub fn search_bg() -> Hsla {
        active_palette().input_bg
    }
    pub fn search_border() -> Hsla {
        Self::border()
    }
    pub fn composer_border() -> Hsla {
        Self::border()
    }
    pub fn topbar_title_active() -> Hsla {
        Self::text_primary()
    }
    pub fn topbar_title_muted() -> Hsla {
        Self::text_tertiary()
    }
    pub fn approval_border() -> Hsla {
        Self::border_strong()
    }
    pub fn activity_detail_border() -> Hsla {
        Self::border_strong().opacity(0.82)
    }
    pub fn activity_detail_text() -> Hsla {
        Self::text_secondary()
    }
    pub fn activity_meta_text() -> Hsla {
        Self::text_tertiary()
    }
}

// ════════════════════════════════════════════════════════════
//  SPACING
// ════════════════════════════════════════════════════════════

impl Tokens {
    pub fn spacing_0p5() -> gpui::Pixels {
        px(2.0)
    }
    pub fn spacing_1() -> gpui::Pixels {
        px(4.0)
    }
    pub fn spacing_1p5() -> gpui::Pixels {
        px(6.0)
    }
    pub fn spacing_2() -> gpui::Pixels {
        px(8.0)
    }
    pub fn spacing_2p5() -> gpui::Pixels {
        px(10.0)
    }
    pub fn spacing_3() -> gpui::Pixels {
        px(12.0)
    }
    pub fn spacing_3p5() -> gpui::Pixels {
        px(14.0)
    }
    pub fn spacing_4() -> gpui::Pixels {
        px(16.0)
    }
    pub fn spacing_5() -> gpui::Pixels {
        px(20.0)
    }
    pub fn spacing_6() -> gpui::Pixels {
        px(24.0)
    }
    pub fn spacing_7() -> gpui::Pixels {
        px(28.0)
    }
    pub fn spacing_8() -> gpui::Pixels {
        px(32.0)
    }
    pub fn spacing_10() -> gpui::Pixels {
        px(40.0)
    }
    pub fn spacing_12() -> gpui::Pixels {
        px(48.0)
    }

    pub fn sidebar_padding() -> gpui::Pixels {
        Self::spacing_2()
    }
    pub fn thread_padding_x() -> gpui::Pixels {
        Self::spacing_7()
    }
    pub fn thread_padding_top() -> gpui::Pixels {
        Self::spacing_6()
    }
    pub fn composer_thread_padding_bottom() -> gpui::Pixels {
        px(Self::COMPOSER_THREAD_INSET)
    }
    pub fn thread_end_scroll_padding() -> gpui::Pixels {
        px(Self::THREAD_END_SCROLL_PADDING)
    }
    pub fn thread_end_scroll_padding_for(
        overlay_bar_height: f32,
        input_expanded: bool,
    ) -> gpui::Pixels {
        px(Self::composer_stack_total(
            overlay_bar_height,
            input_expanded,
            false,
            false,
        ))
    }
    pub fn composer_thread_inset_for(
        overlay_bar_height: f32,
        input_expanded: bool,
    ) -> gpui::Pixels {
        px(Self::composer_thread_inset_px(
            overlay_bar_height,
            input_expanded,
            false,
            false,
        ))
    }
    pub fn tool_group_pl() -> gpui::Pixels {
        px(16.0)
    }
    pub fn ordered_list_marker_width() -> gpui::Pixels {
        Self::spacing_6()
    }
}

// ════════════════════════════════════════════════════════════
//  RADII
// ════════════════════════════════════════════════════════════

impl Tokens {
    pub fn radius_xs() -> gpui::Pixels {
        px(5.0)
    }
    pub fn radius_sm() -> gpui::Pixels {
        px(7.0)
    }
    pub fn radius_md() -> gpui::Pixels {
        px(10.0)
    }
    pub fn radius_lg() -> gpui::Pixels {
        px(14.0)
    }
    pub fn radius_xl() -> gpui::Pixels {
        px(20.0)
    }
    pub fn radius_full() -> gpui::Pixels {
        px(9999.0)
    }

    pub fn radius_card() -> gpui::Pixels {
        px(10.0)
    }
    pub fn radius_composer() -> gpui::Pixels {
        px(24.0)
    }
    /// Horizontal inset for rows above/below the composer pill (half the corner radius).
    pub fn composer_rail_inset_x() -> gpui::Pixels {
        px(12.0)
    }
    pub fn radius_search() -> gpui::Pixels {
        px(10.0)
    }
}

// ════════════════════════════════════════════════════════════
//  SIZING — layout constants
// ════════════════════════════════════════════════════════════

impl Tokens {
    pub const SIDEBAR_WIDTH: f32 = 280.0;
    pub const SIDEBAR_MIN_WIDTH: f32 = 220.0;
    pub const SIDEBAR_MAX_WIDTH: f32 = 360.0;
    pub const SIDEBAR_COLLAPSED: f32 = 40.0;
    pub const DRAWER_WIDTH: f32 = 360.0;
    pub const INSPECTOR_WIDTH_COMPACT: f32 = 400.0;
    pub const INSPECTOR_WIDTH_REVIEW: f32 = 560.0;
    pub const DIFF_PANEL_WIDTH: f32 = Self::INSPECTOR_WIDTH_REVIEW;
    pub const DIFF_PANEL_MIN_WIDTH: f32 = 440.0;
    pub const DIFF_PANEL_MAX_WIDTH: f32 = 760.0;
    pub const CENTER_MIN_WIDTH: f32 = 520.0;
    pub const TOP_BAR_HEIGHT: f32 = 40.0;
    /// Left inset clearing macOS traffic-light window controls.
    pub const TOP_BAR_TRAFFIC_LIGHT_INSET: f32 = 78.0;
    pub const BOTTOM_PANEL_HEIGHT: f32 = 220.0;
    pub const BOTTOM_PANEL_MIN_HEIGHT: f32 = 140.0;
    pub const BOTTOM_PANEL_MAX_HEIGHT: f32 = 420.0;

    /// Monospace terminal cell metrics (bottom panel) — initial fallback until font metrics are measured.
    pub const TERMINAL_CELL_WIDTH: f32 = 8.0;
    pub const TERMINAL_CELL_HEIGHT: f32 = 18.0;

    /// Tab bar row height including vertical padding (`spacing_1` top and bottom).
    pub fn tab_bar_height() -> f32 {
        Self::ROW_HEIGHT_SM + f32::from(Self::spacing_1()) * 2.0
    }

    /// Total fixed chrome above the terminal canvas (header only).
    pub fn terminal_chrome_height() -> f32 {
        Self::tab_bar_height()
    }

    /// Monospace font stack for the embedded terminal canvas.
    pub fn terminal_font_family() -> &'static str {
        "JetBrainsMono Nerd Font Mono"
    }

    /// Proportional reading face for chat and activity surfaces.
    pub fn ui_font_family() -> &'static str {
        #[cfg(target_os = "macos")]
        {
            "SF Pro Text"
        }
        #[cfg(not(target_os = "macos"))]
        {
            "sans-serif"
        }
    }

    /// Symbol-oriented fallback family for terminal graphics cells.
    pub fn terminal_symbol_font_family() -> &'static str {
        "JetBrainsMono Nerd Font Mono"
    }

    /// Font size for terminal cells — cell width/height are measured from this at runtime.
    pub fn terminal_font_size() -> gpui::Pixels {
        px(14.0)
    }

    pub fn attachment_preview_size() -> gpui::Pixels {
        px(Self::ATTACHMENT_PREVIEW_SIZE)
    }

    /// Full terminal theme mapped from the active Vortex palette.
    pub fn terminal_theme() -> terminal::TerminalTheme {
        terminal::TerminalTheme {
            background: hsla_to_rgb(Self::main_bg()),
            foreground: hsla_to_rgb(Self::text_primary()),
            cursor: hsla_to_rgb(Self::text_bright()),
            cursor_text: hsla_to_rgb(Self::main_bg()),
            selection_background: hsla_to_rgb(Self::surface_active()),
            selection_foreground: Some(hsla_to_rgb(Self::text_primary())),
            ansi: [
                hsla_to_rgb(Self::main_bg()),
                hsla_to_rgb(Self::danger()),
                hsla_to_rgb(Self::success()),
                hsla_to_rgb(Self::warning()),
                hsla_to_rgb(Self::info()),
                hsla_to_rgb(Self::accent()),
                hsla_to_rgb(Self::accent_dim()),
                hsla_to_rgb(Self::text_primary()),
                hsla_to_rgb(Self::text_tertiary()),
                hsla_to_rgb(Self::danger()),
                hsla_to_rgb(Self::success()),
                hsla_to_rgb(Self::warning()),
                hsla_to_rgb(Self::info()),
                hsla_to_rgb(Self::accent()),
                hsla_to_rgb(Self::accent_hover()),
                hsla_to_rgb(Self::text_bright()),
            ],
            bright_ansi: [
                hsla_to_rgb(Self::text_tertiary()),
                hsla_to_rgb(Self::danger()),
                hsla_to_rgb(Self::success()),
                hsla_to_rgb(Self::warning()),
                hsla_to_rgb(Self::info()),
                hsla_to_rgb(Self::accent()),
                hsla_to_rgb(Self::accent_hover()),
                hsla_to_rgb(Self::text_bright()),
            ],
            dim_opacity: 0.68,
            bold_is_bright: true,
        }
    }
    pub const STATUS_BAR_HEIGHT: f32 = 24.0;
    pub const THREAD_MAX_WIDTH: f32 = 792.0;
    pub const THREAD_EMPTY_COPY_WIDTH: f32 = 456.0;
    pub const COMPOSER_MAX_WIDTH: f32 = 760.0;
    pub const ATTACHMENT_PREVIEW_SIZE: f32 = 56.0;
    pub const ATTACHMENT_PREVIEW_GAP: f32 = 4.0;
    pub const COMPOSER_ERROR_ROW_HEIGHT: f32 = Self::ROW_HEIGHT_XS;
    /// Compact input pill height (empty state — grows with content).
    pub const COMPOSER_INPUT_MIN_HEIGHT: f32 = 44.0;
    /// Compact mode/strip row above the pill (legacy alias — mode chips sit below the pill now).
    pub const COMPOSER_RUN_STRIP_HEIGHT: f32 = Self::ROW_HEIGHT_SM;
    /// Mode chip row height (`mode_chips.rs`).
    pub const COMPOSER_MODE_CHIP_HEIGHT: f32 = 28.0;
    /// Gap between input pill and mode chips.
    pub const COMPOSER_MODE_ROW_PT: f32 = 8.0;
    /// `composer-container` top padding — `spacing_2`.
    pub const COMPOSER_CONTAINER_PT: f32 = 8.0;
    /// `composer-container` bottom padding.
    pub const COMPOSER_CONTAINER_PB: f32 = 20.0;
    /// Gap under metadata row before the pill.
    pub const COMPOSER_METADATA_PB: f32 = 6.0;
    /// Extra scroll clearance so the last thread row clears the composer footer.
    pub const COMPOSER_THREAD_CLEARANCE: f32 = 12.0;
    pub const COMPOSER_MIN_HEIGHT: f32 =
        Self::COMPOSER_INPUT_MIN_HEIGHT + Self::COMPOSER_RUN_STRIP_HEIGHT;
    /// Single-line input padding (auto-grow expands beyond this).
    pub const COMPOSER_INPUT_AREA_HEIGHT: f32 = 40.0;
    /// Visible composer lines before the input scrolls internally.
    pub const COMPOSER_INPUT_MIN_ROWS: usize = 1;
    pub const COMPOSER_INPUT_MAX_ROWS: usize = 4;
    /// Line height for composer auto-grow (14 px text + leading).
    pub const COMPOSER_INPUT_LINE_HEIGHT: f32 = 20.0;
    /// Toolbar row height inside the stacked composer pill (+ / model / send).
    pub const COMPOSER_PILL_TOOLBAR_HEIGHT: f32 = Self::ROW_HEIGHT_MD + 4.0;
    /// Chars that fit in the inline (single-row toolbar) composer input before stacking.
    pub const COMPOSER_INLINE_CHARS_PER_LINE: usize = 78;

    /// Max scrollable input area height (rows × line height).
    pub fn composer_input_max_height() -> f32 {
        Self::COMPOSER_INPUT_MAX_ROWS as f32 * Self::COMPOSER_INPUT_LINE_HEIGHT
    }

    /// Max pill height when the toolbar sits below the input (stacked layout).
    pub fn composer_attachment_extra_height(has_attachments: bool, has_error: bool) -> f32 {
        let attachments_h = if has_attachments {
            Self::ATTACHMENT_PREVIEW_SIZE + Self::ATTACHMENT_PREVIEW_GAP
        } else {
            0.0
        };
        let error_h = if has_error {
            Self::COMPOSER_ERROR_ROW_HEIGHT
        } else {
            0.0
        };
        attachments_h + error_h
    }

    pub fn composer_pill_stacked_max_height(has_attachments: bool, has_error: bool) -> f32 {
        Self::composer_input_max_height()
            + Self::COMPOSER_PILL_TOOLBAR_HEIGHT
            + 12.0
            + Self::composer_attachment_extra_height(has_attachments, has_error)
    }

    /// Max pill height for the inline (single-row) layout.
    pub fn composer_pill_inline_max_height(has_attachments: bool, has_error: bool) -> f32 {
        Self::COMPOSER_INPUT_MIN_HEIGHT
            + 8.0
            + Self::composer_attachment_extra_height(has_attachments, has_error)
    }
    /// Height of the fade scrim above the composer (thread content shows through at top).
    /// Must be tall enough to avoid a hard horizontal band.
    pub const COMPOSER_FADE_HEIGHT: f32 = 192.0;

    /// Composer footer content height (metadata + pill + mode chips + container padding).
    pub fn composer_footer_height(
        input_expanded: bool,
        has_attachments: bool,
        has_error: bool,
    ) -> f32 {
        let pill_h = if input_expanded {
            Self::composer_pill_stacked_max_height(has_attachments, has_error)
        } else {
            Self::composer_pill_inline_max_height(has_attachments, has_error)
        };
        Self::COMPOSER_CONTAINER_PT
            + Self::ROW_HEIGHT_SM
            + Self::COMPOSER_METADATA_PB
            + pill_h
            + Self::COMPOSER_MODE_ROW_PT
            + Self::COMPOSER_MODE_CHIP_HEIGHT
            + Self::COMPOSER_CONTAINER_PB
    }

    /// Default stack height (inline pill) — used for fade inset baseline.
    pub const COMPOSER_STACK_HEIGHT: f32 = Self::COMPOSER_CONTAINER_PT
        + Self::ROW_HEIGHT_SM
        + Self::COMPOSER_METADATA_PB
        + (Self::COMPOSER_INPUT_MIN_HEIGHT + 8.0)
        + Self::COMPOSER_MODE_ROW_PT
        + Self::COMPOSER_MODE_CHIP_HEIGHT
        + Self::COMPOSER_CONTAINER_PB;
    /// Bottom scroll inset so thread content can pass under the composer fade.
    pub const COMPOSER_THREAD_INSET: f32 = Self::COMPOSER_STACK_HEIGHT - Self::COMPOSER_FADE_HEIGHT;
    /// Scroll padding after the last row (minimum; live value uses [`Self::composer_stack_total`]).
    pub const THREAD_END_SCROLL_PADDING: f32 =
        Self::COMPOSER_STACK_HEIGHT + Self::COMPOSER_THREAD_CLEARANCE;
    /// Bottom gap below sticky approval / pending-action blocks above the composer card.
    pub const COMPOSER_OVERLAY_BAR_GAP: f32 = 8.0;
    /// Sticky approval card above the composer (card body + action row + bottom gap).
    pub const COMPOSER_APPROVAL_BAR_HEIGHT: f32 =
        Self::COMPOSER_OVERLAY_BAR_GAP + 24.0 + 29.0 + 12.0 + Self::ROW_HEIGHT_MD;
    /// One compact pending-action row (`ROW_HEIGHT_LG` + vertical padding).
    pub const COMPOSER_PENDING_ACTION_ROW_HEIGHT: f32 = Self::ROW_HEIGHT_LG + 16.0;

    pub const ROW_HEIGHT_XS: f32 = 24.0;
    pub const ROW_HEIGHT_SM: f32 = 28.0;
    pub const ROW_HEIGHT_MD: f32 = 30.0;
    pub const ROW_HEIGHT_LG: f32 = 32.0;
    pub const ROW_HEIGHT_XL: f32 = 44.0;

    pub const SEARCH_HEIGHT: f32 = 32.0;
    pub const TOOL_ROW_HEIGHT: f32 = 28.0;
    pub const DIFF_HEADER_HEIGHT: f32 = 44.0;
    pub const DIFF_TAB_HEIGHT: f32 = 32.0;
    pub const DIFF_PATH_HEIGHT: f32 = 32.0;
    pub const DIFF_LINE_HEIGHT: f32 = 20.0;
    pub const DIFF_GUTTER_WIDTH: f32 = 38.0;
    pub const DIFF_SIGN_WIDTH: f32 = 18.0;

    pub const TREE_INDENT: f32 = 12.0;

    pub fn tree_indent(level: u32) -> gpui::Pixels {
        px(Self::TREE_INDENT * level as f32)
    }

    /// Pending-action bar height for the given number of visible rows (0–2).
    pub fn composer_pending_action_bar_height(row_count: u8) -> f32 {
        match row_count {
            0 => 0.0,
            1 => Self::COMPOSER_OVERLAY_BAR_GAP + Self::COMPOSER_PENDING_ACTION_ROW_HEIGHT,
            n => {
                let n = n as f32;
                Self::COMPOSER_OVERLAY_BAR_GAP
                    + n * Self::COMPOSER_PENDING_ACTION_ROW_HEIGHT
                    + (n - 1.0) * 4.0
            }
        }
    }

    /// Total bottom stack: composer footer + clearance + optional overlay bar above it.
    pub fn composer_stack_total(
        overlay_bar_height: f32,
        input_expanded: bool,
        has_attachments: bool,
        has_error: bool,
    ) -> f32 {
        Self::composer_footer_height(input_expanded, has_attachments, has_error)
            + Self::COMPOSER_THREAD_CLEARANCE
            + overlay_bar_height
    }

    /// Fade scrim bottom offset for a given overlay bar height.
    pub fn composer_thread_inset_px(
        overlay_bar_height: f32,
        input_expanded: bool,
        has_attachments: bool,
        has_error: bool,
    ) -> f32 {
        Self::composer_stack_total(
            overlay_bar_height,
            input_expanded,
            has_attachments,
            has_error,
        ) - Self::COMPOSER_FADE_HEIGHT
    }
}

// ════════════════════════════════════════════════════════════
//  TYPOGRAPHY
// ════════════════════════════════════════════════════════════

impl Tokens {
    /// 11 px — status bar, section labels
    pub fn text_xs() -> gpui::Pixels {
        px(11.0)
    }
    /// 11.5 px — status bar
    pub fn text_status() -> gpui::Pixels {
        px(11.5)
    }
    /// 12 px — role labels, file paths in diff header
    pub fn text_label() -> gpui::Pixels {
        px(12.0)
    }
    /// 12.5 px — diff code, file paths, tab labels
    pub fn text_code() -> gpui::Pixels {
        px(12.5)
    }
    /// 13 px — sidebar, tool rows, top bar
    pub fn text_sm() -> gpui::Pixels {
        px(13.0)
    }
    /// 14 px — composer input
    pub fn text_base() -> gpui::Pixels {
        px(14.0)
    }
    /// 15 px — thread body
    pub fn text_md() -> gpui::Pixels {
        px(15.0)
    }
    /// 15 px thread body line height (~1.55)
    pub fn text_md_leading() -> gpui::Pixels {
        px(23.0)
    }
    /// 13 px tool row line height
    pub fn text_sm_leading() -> gpui::Pixels {
        px(20.0)
    }
    /// 13 px compact line height for secondary text inside fixed-height rows.
    pub fn text_sm_leading_compact() -> gpui::Pixels {
        px(18.0)
    }
    /// 21 px — section headings
    pub fn text_lg() -> gpui::Pixels {
        px(21.0)
    }
    /// 26 px — page titles
    pub fn text_xl() -> gpui::Pixels {
        px(26.0)
    }
    /// 26 px page title line height.
    pub fn text_xl_leading() -> gpui::Pixels {
        px(32.0)
    }
}

// ════════════════════════════════════════════════════════════
//  SHADOWS
// ════════════════════════════════════════════════════════════

impl Tokens {
    pub const ELEVATION_CARD: &str = "sm";
    pub const ELEVATION_FLOATING: &str = "md";
    pub const ELEVATION_MODAL: &str = "lg";
}

fn hsla_to_rgb(color: Hsla) -> u32 {
    let c = color.to_rgb();
    let r = (c.r.clamp(0.0, 1.0) * 255.0).round() as u32;
    let g = (c.g.clamp(0.0, 1.0) * 255.0).round() as u32;
    let b = (c.b.clamp(0.0, 1.0) * 255.0).round() as u32;
    (r << 16) | (g << 8) | b
}
