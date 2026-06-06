use std::time::Duration;

#[test]
fn spawn_shell_receives_output() {
    let cwd = std::env::temp_dir();
    let session =
        terminal::TerminalSession::spawn(&cwd, 80, 24, terminal::TerminalTheme::default())
            .expect("spawn");

    let rx = session.frame_notifications();
    let frame = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("timed out waiting for first frame");
    eprintln!(
        "first frame {}x{}, sample cells: {:?}",
        frame.cols,
        frame.rows,
        frame
            .cells
            .iter()
            .filter(|c| !c.text.trim().is_empty())
            .take(5)
            .map(|c| &c.text)
            .collect::<Vec<_>>()
    );

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut saw_text = false;
    let mut inspect = Some(frame);
    while std::time::Instant::now() < deadline {
        if let Some(frame) = inspect.take().or_else(|| rx.try_recv().ok()) {
            for cell in &frame.cells {
                if !cell.text.trim().is_empty() {
                    saw_text = true;
                    break;
                }
            }
        }
        if saw_text {
            break;
        }
        if let Ok(frame) = rx.recv_timeout(Duration::from_millis(100)) {
            inspect = Some(frame);
        }
    }
    assert!(saw_text, "expected shell prompt output within 5s");
}
