//! GPUI keyboard event → PTY bytes (IO-thread encoding; no VT roundtrip for input).

use crate::session::{KeyAction, KeyPress, TerminalMods};

/// Map a GPUI-style key name and modifiers into a `KeyPress`.
pub fn key_press_from_parts(
    key: impl Into<String>,
    shift: bool,
    alt: bool,
    control: bool,
    super_key: bool,
    action: KeyAction,
    text: Option<String>,
) -> KeyPress {
    KeyPress {
        key: key.into(),
        mods: TerminalMods {
            shift,
            alt,
            control,
            super_key,
        },
        action,
        text,
    }
}

/// Encode a key event to PTY stdin bytes on the IO thread (immediate write, no VT hop).
pub fn encode_key_for_pty(key: &KeyPress) -> Vec<u8> {
    if key.action == KeyAction::Release {
        return Vec::new();
    }

    let k = key.key.to_ascii_lowercase();
    match k.as_str() {
        "enter" | "return" => return vec![b'\r'],
        "tab" => return vec![b'\t'],
        "backspace" => return vec![0x7f],
        "escape" => return vec![0x1b],
        "space" => return vec![b' '],
        "up" => return vec![0x1b, b'[', b'A'],
        "down" => return vec![0x1b, b'[', b'B'],
        "right" => return vec![0x1b, b'[', b'C'],
        "left" => return vec![0x1b, b'[', b'D'],
        "home" => return vec![0x1b, b'[', b'H'],
        "end" => return vec![0x1b, b'[', b'F'],
        "pageup" => return vec![0x1b, b'[', b'5', b'~'],
        "pagedown" => return vec![0x1b, b'[', b'6', b'~'],
        "delete" => return vec![0x1b, b'[', b'3', b'~'],
        "f1" => return vec![0x1b, b'O', b'P'],
        "f2" => return vec![0x1b, b'O', b'Q'],
        "f3" => return vec![0x1b, b'O', b'R'],
        "f4" => return vec![0x1b, b'O', b'S'],
        "f5" => return vec![0x1b, b'[', b'1', b'5', b'~'],
        "f6" => return vec![0x1b, b'[', b'1', b'7', b'~'],
        "f7" => return vec![0x1b, b'[', b'1', b'8', b'~'],
        "f8" => return vec![0x1b, b'[', b'1', b'9', b'~'],
        "f9" => return vec![0x1b, b'[', b'2', b'0', b'~'],
        "f10" => return vec![0x1b, b'[', b'2', b'1', b'~'],
        "f11" => return vec![0x1b, b'[', b'2', b'3', b'~'],
        "f12" => return vec![0x1b, b'[', b'2', b'4', b'~'],
        _ => {}
    }

    if key.mods.control {
        if let Some(text) = &key.text {
            if text.len() == 1 {
                let b = text.as_bytes()[0];
                if (b'a'..=b'z').contains(&b) {
                    return vec![b - b'a' + 1];
                }
                if (b'A'..=b'Z').contains(&b) {
                    return vec![b - b'A' + 1];
                }
            }
        }
        let k = key.key.to_ascii_lowercase();
        if k.len() == 1 {
            let b = k.as_bytes()[0];
            if (b'a'..=b'z').contains(&b) {
                return vec![b - b'a' + 1];
            }
        }
    }

    if let Some(text) = &key.text {
        if !text.is_empty() && !key.mods.control && !key.mods.alt && !key.mods.super_key {
            return text.as_bytes().to_vec();
        }
    }

    match k.as_str() {
        _ if k.len() == 1 && !key.mods.alt && !key.mods.super_key => k.as_bytes().to_vec(),
        _ => Vec::new(),
    }
}

/// Normalize paste text: `\r\n` → `\n`, strip other C0 controls except tab/newline.
pub fn normalize_paste(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.replace("\r\n", "\n").chars() {
        match ch {
            '\n' | '\t' => out.push(ch),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Returns true when paste content contains risky control bytes (besides tab/newline).
pub fn paste_needs_confirmation(text: &str) -> bool {
    text.chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
}

/// Bracketed-paste wrapper bytes when the terminal has bracketed paste enabled.
pub fn bracketed_paste_bytes(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 6);
    out.extend_from_slice(b"\x1b[200~");
    out.extend_from_slice(text.as_bytes());
    out.extend_from_slice(b"\x1b[201~");
    out
}
