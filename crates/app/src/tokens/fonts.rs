//! Bundled font registration for GPUI text rendering.

use std::borrow::Cow;

use gpui::App;

/// Register terminal Nerd Font files embedded at build time.
pub fn register_bundled_fonts(cx: &mut App) {
    if let Err(err) = cx.text_system().add_fonts(vec![
        Cow::Borrowed(include_bytes!(
            "../../assets/fonts/JetBrainsMonoNerdFontMono-Regular.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../../assets/fonts/JetBrainsMonoNerdFontMono-Bold.ttf"
        )),
    ]) {
        tracing::warn!("failed to register bundled terminal fonts: {err}");
    }
}
