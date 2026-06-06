use std::io::Read;
use std::time::Duration;

#[test]
fn pty_reads_shell_bytes() {
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty");

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let mut cmd = CommandBuilder::new(&shell);
    cmd.arg("-i");
    cmd.cwd(std::env::temp_dir());
    cmd.env("TERM", "xterm-256color");

    let _child = pair.slave.spawn_command(cmd).expect("spawn shell");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().expect("reader");
    let mut buf = [0u8; 4096];
    let mut total = 0usize;
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                eprintln!("pty bytes: {:?}", String::from_utf8_lossy(&buf[..n]));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("read error: {e}"),
        }
    }
    assert!(
        total > 0,
        "expected bytes from interactive shell, got {total}"
    );
}
