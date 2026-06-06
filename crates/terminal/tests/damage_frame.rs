use terminal::{TerminalCell, TerminalDamageFrame, TerminalRenderer};

#[test]
fn renderer_marks_only_dirty_rows() {
    let mut renderer = TerminalRenderer::default();
    let seed = TerminalDamageFrame {
        cols: 4,
        rows: 2,
        cells: vec![TerminalCell::default(); 8],
        dirty_rows: vec![true, true],
        full_redraw: true,
        cursor_col: None,
        cursor_row: None,
        cursor_visible: false,
        default_fg: 0xffffff,
        default_bg: 0x000000,
        scrollback_at_bottom: true,
    };
    renderer.apply_damage_frame(&seed);
    renderer.clear_dirty();

    let frame = TerminalDamageFrame {
        cols: 4,
        rows: 2,
        cells: vec![TerminalCell::default(); 8],
        dirty_rows: vec![false, true],
        full_redraw: false,
        cursor_col: None,
        cursor_row: None,
        cursor_visible: false,
        default_fg: 0xffffff,
        default_bg: 0x000000,
        scrollback_at_bottom: true,
    };
    renderer.apply_damage_frame(&frame);
    assert!(!renderer.row_dirty(0));
    assert!(renderer.row_dirty(1));
}

#[test]
fn full_redraw_marks_all_rows_dirty() {
    let mut renderer = TerminalRenderer::default();
    let frame = TerminalDamageFrame {
        cols: 3,
        rows: 3,
        cells: vec![TerminalCell::default(); 9],
        dirty_rows: vec![true; 3],
        full_redraw: true,
        cursor_col: None,
        cursor_row: None,
        cursor_visible: false,
        default_fg: 0xffffff,
        default_bg: 0x000000,
        scrollback_at_bottom: true,
    };
    renderer.apply_damage_frame(&frame);
    assert!(renderer.row_dirty(0));
    assert!(renderer.row_dirty(2));
}
