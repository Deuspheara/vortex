use terminal::{KeyAction, KeyPress, TerminalMods, encode_key_for_pty};

#[test]
fn printable_key_encodes_to_bytes() {
    let bytes = encode_key_for_pty(&KeyPress {
        key: "a".into(),
        mods: TerminalMods::default(),
        action: KeyAction::Press,
        text: Some("a".into()),
    });
    assert_eq!(bytes, b"a");
}

#[test]
fn arrow_key_encodes_escape_sequence() {
    let bytes = encode_key_for_pty(&KeyPress {
        key: "up".into(),
        mods: TerminalMods::default(),
        action: KeyAction::Press,
        text: None,
    });
    assert_eq!(bytes, vec![0x1b, b'[', b'A']);
}

#[test]
fn enter_key_encodes_carriage_return() {
    for key in ["enter", "return"] {
        let bytes = encode_key_for_pty(&KeyPress {
            key: key.into(),
            mods: TerminalMods::default(),
            action: KeyAction::Press,
            text: Some("\n".into()),
        });
        assert_eq!(bytes, b"\r", "key={key}");
    }
}

#[test]
fn ctrl_c_encodes_control_byte() {
    let bytes = encode_key_for_pty(&KeyPress {
        key: "c".into(),
        mods: TerminalMods {
            control: true,
            ..Default::default()
        },
        action: KeyAction::Press,
        text: Some("c".into()),
    });
    assert_eq!(bytes, vec![3]);
}
