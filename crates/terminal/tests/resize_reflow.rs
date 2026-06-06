use std::time::Duration;

use terminal::{TerminalRenderer, TerminalSession, TerminalTheme};

#[test]
fn resize_updates_grid_dimensions() {
    let cwd = std::env::temp_dir();
    let session = TerminalSession::spawn(&cwd, 80, 24, TerminalTheme::default()).expect("spawn");
    session.resize(100, 30, 8, 18);

    let rx = session.frame_notifications();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut saw = false;
    while std::time::Instant::now() < deadline {
        if let Ok(frame) = rx.recv_timeout(Duration::from_millis(200)) {
            if frame.cols == 100 && frame.rows == 30 {
                saw = true;
                break;
            }
        }
    }
    assert!(saw, "expected resize frame with 100x30");
}

#[test]
fn renderer_resizes_with_frame() {
    let mut renderer = TerminalRenderer::default();
    let frame = terminal::TerminalDamageFrame {
        cols: 10,
        rows: 5,
        cells: vec![terminal::TerminalCell::default(); 50],
        dirty_rows: vec![true; 5],
        full_redraw: true,
        cursor_col: None,
        cursor_row: None,
        cursor_visible: false,
        default_fg: 0xffffff,
        default_bg: 0x000000,
        scrollback_at_bottom: true,
    };
    renderer.apply_damage_frame(&frame);
    assert_eq!(renderer.cols(), 10);
    assert_eq!(renderer.rows(), 5);
}
