//! Terminal color theme — mapped from Vortex design tokens in the app layer.

/// Full terminal color palette for libghostty defaults and GPUI canvas painting.
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalTheme {
    pub background: u32,
    pub foreground: u32,
    pub cursor: u32,
    pub cursor_text: u32,
    pub selection_background: u32,
    pub selection_foreground: Option<u32>,
    pub ansi: [u32; 16],
    pub bright_ansi: [u32; 8],
    pub dim_opacity: f32,
    pub bold_is_bright: bool,
}

impl Default for TerminalTheme {
    fn default() -> Self {
        Self {
            background: 0x070808,
            foreground: 0xe6e6e6,
            cursor: 0xe6e6e6,
            cursor_text: 0x070808,
            selection_background: 0x3d4f5f,
            selection_foreground: None,
            ansi: [
                0x070808, 0xcc6666, 0x86b375, 0xd7ba7d, 0x6b9bd1, 0xb294bb, 0x4ec9b0, 0xe6e6e6,
                0x808080, 0xf44747, 0x89d185, 0xffcc66, 0x569cd6, 0xc586c0, 0x4ec9b0, 0xffffff,
            ],
            bright_ansi: [
                0x808080, 0xf44747, 0x89d185, 0xffcc66, 0x569cd6, 0xc586c0, 0x4ec9b0, 0xffffff,
            ],
            dim_opacity: 0.5,
            bold_is_bright: true,
        }
    }
}

impl TerminalTheme {
    pub fn default_fg(&self) -> u32 {
        self.foreground
    }

    pub fn default_bg(&self) -> u32 {
        self.background
    }
}

/// Legacy alias kept for a transition period in tests.
pub type TerminalPalette = TerminalTheme;
