use std::io::Read;

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

    let mut cmd = CommandBuilder::new("/bin/sh");
    cmd.arg("-lc");
    cmd.arg("printf vortex-pty-ready");
    cmd.cwd(std::env::temp_dir());
    cmd.env("TERM", "xterm-256color");

    let mut child = pair.slave.spawn_command(cmd).expect("spawn shell");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().expect("reader");
    let mut output = Vec::new();
    reader.read_to_end(&mut output).expect("read pty output");
    child.wait().expect("wait for shell");

    let output = String::from_utf8_lossy(&output);
    assert!(
        output.contains("vortex-pty-ready"),
        "expected bytes from shell, got {output:?}"
    );
}
