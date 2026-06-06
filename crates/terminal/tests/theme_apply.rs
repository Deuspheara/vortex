use terminal::TerminalTheme;

#[test]
fn theme_default_colors_are_non_zero() {
    let theme = TerminalTheme::default();
    assert_ne!(theme.background, 0);
    assert_ne!(theme.foreground, 0);
    assert_ne!(theme.cursor, 0);
    assert_eq!(theme.ansi.len(), 16);
    assert_eq!(theme.bright_ansi.len(), 8);
}

#[test]
fn theme_mapper_values_reach_defaults() {
    let theme = TerminalTheme {
        background: 0x112233,
        foreground: 0xaabbcc,
        cursor: 0xffffff,
        cursor_text: 0x000000,
        ..TerminalTheme::default()
    };
    assert_eq!(theme.default_bg(), 0x112233);
    assert_eq!(theme.default_fg(), 0xaabbcc);
}
