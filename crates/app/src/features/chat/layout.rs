//! Shared thread layout constants — gaps and heights for manifest + components.

use crate::shared::components::markdown_preview::LINE_LEADING;
use crate::tokens::Tokens;

#[inline]
fn px(v: gpui::Pixels) -> f32 {
    v.into()
}

/// Space before a new conversational turn — compact but readable.
pub fn turn_gap() -> f32 {
    px(Tokens::spacing_2())
}

/// Space before the first row in an agent activity band.
pub fn activity_band_gap() -> f32 {
    px(Tokens::spacing_1p5())
}

/// Tight stack between consecutive activity headers — `Tokens::spacing_0p5()`.
pub fn activity_inner_gap() -> f32 {
    px(Tokens::spacing_0p5())
}

/// Space between assistant prose and the following tool/reasoning band.
pub fn post_assistant_activity_gap() -> f32 {
    px(Tokens::spacing_1p5())
}

/// Vertical padding inside user message bubble (both sides) — `Tokens::spacing_4()`.
pub fn user_bubble_py() -> f32 {
    px(Tokens::spacing_4())
}

/// Height of truncated-output notice rows — `Tokens::ROW_HEIGHT_XS`.
pub const TRUNCATED_H: f32 = Tokens::ROW_HEIGHT_XS;

/// Vertical padding inside run-error row (both sides) — `Tokens::spacing_4()`.
pub fn run_error_py() -> f32 {
    px(Tokens::spacing_4())
}

/// Title line height in run-error estimate — `LINE_LEADING`.
pub const RUN_ERROR_TITLE_H: f32 = LINE_LEADING;

/// Gap between error title and message — `Tokens::spacing_0p5()`.
pub fn run_error_inner_gap() -> f32 {
    px(Tokens::spacing_0p5())
}

/// Max expanded reasoning body height.
pub const REASONING_BODY_MAX_H: f32 = 480.0;

/// Section header row height — `Tokens::ROW_HEIGHT_SM`.
pub const SECTION_HEADER_H: f32 = Tokens::ROW_HEIGHT_SM;

/// Gap between timeline sections — `Tokens::spacing_2()`.
pub fn section_gap() -> f32 {
    px(Tokens::spacing_2())
}

/// "See more" control height under collapsed user messages.
pub const USER_SEE_MORE_H: f32 = 22.0;

/// Activity header row height — `Tokens::TOOL_ROW_HEIGHT`.
pub const HEADER_H: f32 = Tokens::TOOL_ROW_HEIGHT;

/// Compact plan status row height — `Tokens::TOOL_ROW_HEIGHT`.
pub const PLAN_STATUS_H: f32 = Tokens::TOOL_ROW_HEIGHT;

/// Monospace output line height — `Tokens::DIFF_LINE_HEIGHT`.
pub const LINE_H: f32 = Tokens::DIFF_LINE_HEIGHT;

/// Diff file stat line height — `Tokens::ROW_HEIGHT_SM`.
pub const DIFF_FILE_H: f32 = Tokens::ROW_HEIGHT_SM;

/// Approval row height — `Tokens::ROW_HEIGHT_SM`.
pub const APPROVAL_H: f32 = Tokens::ROW_HEIGHT_SM;

/// Top padding on assistant markdown body — `Tokens::spacing_1()`.
pub fn assistant_body_pt() -> f32 {
    px(Tokens::spacing_1())
}

/// Compact "Result" label height above assistant prose.
pub fn assistant_result_label_h() -> f32 {
    px(Tokens::text_sm_leading_compact())
}

/// Gap between the result label and assistant prose.
pub fn assistant_result_label_gap() -> f32 {
    px(Tokens::spacing_1())
}

/// Extra height for streaming cursor under assistant body — `Tokens::spacing_2()`.
pub fn assistant_streaming_extra() -> f32 {
    px(Tokens::spacing_2())
}
