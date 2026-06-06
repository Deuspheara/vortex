# Vortex terminal crate

PTY-backed interactive shell with [libghostty-vt](https://crates.io/crates/libghostty-vt) for VT emulation.

## Build requirements

`libghostty-vt` compiles Ghostty's VT engine at build time and requires **Zig 0.15.2** on `PATH` (not 0.16).

```bash
# macOS (Homebrew)
brew install zig@0.15

# Verify
zig version   # expect 0.15.2
```

Optional offline builds: set `GHOSTTY_SOURCE_DIR` to a local Ghostty checkout (see `libghostty-vt-sys` docs).

## Architecture

| Module | Role |
|--------|------|
| `pty.rs` | Spawn shell via `portable-pty` (`$SHELL` or `/bin/zsh`), pixel winsize |
| `pty_io.rs` | PTY reader + IO loop; routes keys through `VtCommand::Key` |
| `core.rs` | libghostty-vt thread, damage frames, libghostty key encoder |
| `renderer.rs` | Retained cell buffer + row dirty flags for canvas paint |
| `input.rs` | GPUI → `KeyPress` mapping, paste normalization |
| `theme.rs` | `TerminalTheme` struct (mapped from app `Tokens`) |
| `session.rs` | Public `TerminalSession` handle (Send) |

The UI thread receives `TerminalDamageFrame` snapshots and paints via a single GPUI `canvas()` surface.

## App dependency

```toml
terminal = { path = "../terminal", features = ["ghostty"] }
```

## Manual acceptance checklist

- `yes | head -n 50000` — smooth output, no UI freeze
- `cargo test` — ANSI colors render correctly
- `vim` / `htop` — keyboard and resize work
- Unicode / CJK — graphemes render
- Resize while streaming — reflow without broken wrap
- Scroll up → frozen viewport; “Jump to latest” returns to bottom
- Paste — bracketed paste; confirmation for control characters
- Ctrl+C / Ctrl+D — signal delivery
- Theme switch — colors track workspace tokens
- High DPI — crisp cell grid
- Panel collapse/expand — session persists (no respawn)
