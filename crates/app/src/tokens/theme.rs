//! Vortex theme system — community-extensible, VS Code-style.
//!
//! Built on `gpui-component`'s `ThemeRegistry`. Bundled themes ship in
//! `crates/app/themes/`. Users can drop `.json` theme sets into
//! `~/.vortex/themes/` to add community themes without recompiling.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{OnceLock, RwLock};

use std::sync::Arc;

use gpui::{App, Hsla, SharedString, Window};
use gpui_component::highlighter::HighlightTheme;
use gpui_component::{Theme, ThemeMode, ThemeRegistry, ThemeSet};

/// Name of the default bundled dark theme.
pub const DEFAULT_DARK_THEME: &str = "Vortex Dark";
/// Name of the bundled light theme.
pub const DEFAULT_LIGHT_THEME: &str = "Vortex Light";

/// Derived semantic palette mapped from the active gpui-component theme.
#[derive(Debug, Clone, Copy)]
pub struct VortexPalette {
    pub app_bg: Hsla,
    pub panel_bg: Hsla,
    pub main_bg: Hsla,
    pub surface: Hsla,
    pub surface_hover: Hsla,
    pub surface_active: Hsla,
    pub surface_overlay: Hsla,
    pub chrome: Hsla,
    pub input_bg: Hsla,
    pub diff_bg: Hsla,
    pub sidebar_bg: Hsla,
    pub border: Hsla,
    pub border_subtle: Hsla,
    pub divider: Hsla,
    pub border_strong: Hsla,
    pub border_focus: Hsla,
    pub sidebar_border: Hsla,
    pub text_primary: Hsla,
    pub text_secondary: Hsla,
    pub text_tertiary: Hsla,
    pub text_faint: Hsla,
    pub text_bright: Hsla,
    pub sidebar_text: Hsla,
    pub sidebar_text_hover: Hsla,
    pub sidebar_text_muted: Hsla,
    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub accent_dim: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    pub danger: Hsla,
    pub info: Hsla,
    pub code_fg: Hsla,
    pub code_bg: Hsla,
    pub diff_add_bg: Hsla,
    pub diff_add_text: Hsla,
    pub diff_add_indicator: Hsla,
    pub diff_del_bg: Hsla,
    pub diff_del_text: Hsla,
    pub diff_del_indicator: Hsla,
    pub diff_line_number: Hsla,
}

/// Diff gutter / hunk colours derived from appearance mode.
#[derive(Debug, Clone, Copy)]
struct DiffColors {
    add_bg: Hsla,
    add_text: Hsla,
    add_indicator: Hsla,
    del_bg: Hsla,
    del_text: Hsla,
    del_indicator: Hsla,
    line_number: Hsla,
}

fn diff_colors(dark: bool) -> DiffColors {
    if dark {
        DiffColors {
            add_bg: {
                let c: Hsla = gpui::rgb(0x238636).into();
                c.opacity(0.10)
            },
            add_text: gpui::rgb(0x7ee787).into(),
            add_indicator: gpui::rgb(0x238636).into(),
            del_bg: {
                let c: Hsla = gpui::rgb(0xf85149).into();
                c.opacity(0.10)
            },
            del_text: gpui::rgb(0xff7b72).into(),
            del_indicator: gpui::rgb(0xf85149).into(),
            line_number: gpui::rgb(0x565a61).into(),
        }
    } else {
        DiffColors {
            add_bg: {
                let c: Hsla = gpui::rgb(0x2da44e).into();
                c.opacity(0.18)
            },
            add_text: gpui::rgb(0x116329).into(),
            add_indicator: gpui::rgb(0x1a7f37).into(),
            del_bg: {
                let c: Hsla = gpui::rgb(0xcf222e).into();
                c.opacity(0.14)
            },
            del_text: gpui::rgb(0x82071e).into(),
            del_indicator: gpui::rgb(0xcf222e).into(),
            line_number: gpui::rgb(0x9ba0ae).into(),
        }
    }
}

impl VortexPalette {
    /// Hard-coded fallback used before theme init completes.
    pub fn fallback() -> Self {
        let diff = diff_colors(true);
        Self {
            app_bg: gpui::rgb(0x070808).into(),
            panel_bg: gpui::rgb(0x0d0e0f).into(),
            main_bg: gpui::rgb(0x070808).into(),
            surface: gpui::rgb(0x121314).into(),
            surface_hover: gpui::rgb(0x191a1c).into(),
            surface_active: gpui::rgb(0x1a1d21).into(),
            surface_overlay: gpui::rgb(0x121314).into(),
            chrome: gpui::rgb(0x0b0c0d).into(),
            input_bg: gpui::rgb(0x141516).into(),
            diff_bg: gpui::rgb(0x080909).into(),
            sidebar_bg: gpui::rgb(0x0b0c0d).into(),
            border: gpui::rgb(0x2a2d31).into(),
            border_subtle: gpui::rgb(0x1f2124).into(),
            divider: {
                let c: Hsla = gpui::rgb(0x2a2d31).into();
                c.opacity(0.22)
            },
            border_strong: gpui::rgb(0x3a3d42).into(),
            border_focus: gpui::rgb(0x7aa2ff).into(),
            sidebar_border: gpui::rgb(0x202225).into(),
            text_primary: gpui::rgb(0xe6e6e6).into(),
            text_secondary: gpui::rgb(0xa2a4a8).into(),
            text_tertiary: gpui::rgb(0x71747a).into(),
            text_faint: gpui::rgb(0x52555a).into(),
            text_bright: gpui::rgb(0xffffff).into(),
            sidebar_text: gpui::rgb(0x8d9096).into(),
            sidebar_text_hover: gpui::rgb(0xd6d7d9).into(),
            sidebar_text_muted: gpui::rgb(0x5f6268).into(),
            accent: gpui::rgb(0x7aa2ff).into(),
            accent_hover: gpui::rgb(0x92b4ff).into(),
            accent_dim: gpui::rgb(0x5a7fd9).into(),
            success: gpui::rgb(0x57c785).into(),
            warning: gpui::rgb(0xd6a04f).into(),
            danger: gpui::rgb(0xe06c75).into(),
            info: gpui::rgb(0x7aa2ff).into(),
            code_fg: gpui::rgb(0xc9cbd0).into(),
            code_bg: gpui::rgb(0x080909).into(),
            diff_add_bg: diff.add_bg,
            diff_add_text: diff.add_text,
            diff_add_indicator: diff.add_indicator,
            diff_del_bg: diff.del_bg,
            diff_del_text: diff.del_text,
            diff_del_indicator: diff.del_indicator,
            diff_line_number: diff.line_number,
        }
    }

    /// Build semantic colors from the active gpui-component theme.
    pub fn from_theme(theme: &Theme) -> Self {
        let dark = theme.is_dark();
        let workspace = theme.background;
        let border_subtle = theme.border.opacity(if dark { 0.4 } else { 0.55 });
        let divider = workspace.blend(theme.border.opacity(if dark { 0.2 } else { 0.32 }));
        let input_bg = theme.secondary.opacity(if dark { 0.92 } else { 1.0 });
        let highlight = &theme.highlight_theme.style;
        let diff_bg = highlight.editor_background.unwrap_or(theme.background);
        let code_fg = highlight.editor_foreground.unwrap_or(theme.foreground);
        let code_bg = highlight.editor_background.unwrap_or(if dark {
            theme.background
        } else {
            theme.list_hover
        });
        let diff = diff_colors(dark);
        Self {
            app_bg: workspace,
            panel_bg: theme.sidebar,
            main_bg: workspace,
            surface: theme.secondary,
            surface_hover: theme.list_hover,
            surface_active: theme.list_active,
            surface_overlay: theme.popover,
            chrome: theme.title_bar,
            input_bg,
            diff_bg,
            sidebar_bg: theme.sidebar,
            border: theme.border,
            border_subtle,
            divider,
            border_strong: theme.input,
            border_focus: theme.ring,
            sidebar_border: theme.sidebar_border,
            text_primary: theme.foreground,
            text_secondary: theme.muted_foreground,
            text_tertiary: caption_color(theme.muted_foreground, theme.background),
            text_faint: caption_color(theme.muted_foreground, theme.background).opacity(0.85),
            text_bright: theme.primary_foreground,
            sidebar_text: theme.sidebar_foreground,
            sidebar_text_hover: theme.foreground,
            sidebar_text_muted: caption_color(theme.sidebar_foreground, theme.sidebar),
            accent: theme.primary,
            accent_hover: theme.primary_hover,
            accent_dim: theme.primary_active,
            success: theme.success,
            warning: theme.warning,
            danger: theme.danger,
            info: theme.info,
            code_fg,
            code_bg,
            diff_add_bg: diff.add_bg,
            diff_add_text: diff.add_text,
            diff_add_indicator: diff.add_indicator,
            diff_del_bg: diff.del_bg,
            diff_del_text: diff.del_text,
            diff_del_indicator: diff.del_indicator,
            diff_line_number: diff.line_number,
        }
    }
}

fn caption_color(muted: Hsla, bg: Hsla) -> Hsla {
    bg.blend(muted.opacity(0.72))
}

static ACTIVE_PALETTE: OnceLock<RwLock<VortexPalette>> = OnceLock::new();
static ACTIVE_HIGHLIGHT: OnceLock<RwLock<Arc<HighlightTheme>>> = OnceLock::new();

fn palette_lock() -> &'static RwLock<VortexPalette> {
    ACTIVE_PALETTE.get_or_init(|| RwLock::new(VortexPalette::fallback()))
}

fn highlight_lock() -> &'static RwLock<Arc<HighlightTheme>> {
    ACTIVE_HIGHLIGHT.get_or_init(|| RwLock::new(HighlightTheme::default_dark()))
}

/// Returns the currently active Vortex semantic palette.
pub fn active_palette() -> VortexPalette {
    palette_lock()
        .read()
        .map(|p| *p)
        .unwrap_or_else(|_| VortexPalette::fallback())
}

/// Returns the active tree-sitter highlight theme (for code blocks).
#[allow(dead_code)]
pub fn active_highlight_theme() -> Arc<HighlightTheme> {
    highlight_lock()
        .read()
        .map(|h| h.clone())
        .unwrap_or_else(|_| HighlightTheme::default_dark())
}

/// Sync the cached palette from the active gpui-component theme.
pub fn sync_palette(cx: &App) {
    let theme = Theme::global(cx);
    let palette = VortexPalette::from_theme(theme);
    if let Ok(mut active) = palette_lock().write() {
        *active = palette;
    }
    if let Ok(mut highlight) = highlight_lock().write() {
        *highlight = theme.highlight_theme.clone();
    }
}

/// Bundled themes directory (shipped with the app).
pub fn bundled_themes_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("themes")
}

/// User themes directory for community / custom themes.
pub fn user_themes_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".vortex")
        .join("themes")
}

/// Ensure user themes dir exists and contains bundled defaults.
fn seed_user_themes() {
    let user = user_themes_dir();
    if std::fs::create_dir_all(&user).is_err() {
        return;
    }

    let bundled = bundled_themes_dir();
    if !bundled.exists() {
        return;
    }

    if let Ok(entries) = std::fs::read_dir(&bundled) {
        for entry in entries.flatten() {
            let src = entry.path();
            if src.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let dest = user.join(src.file_name().unwrap_or_default());
            let overwrite = src.file_name().is_some_and(|n| n == "vortex.json");
            if overwrite || !dest.exists() {
                let _ = std::fs::copy(&src, &dest);
            }
        }
    }
}

/// Load bundled + user themes and apply the default Vortex dark theme.
pub fn init(cx: &mut App) {
    let _ = palette_lock();
    seed_user_themes();
    load_bundled_themes(cx);

    let watch_dir = user_themes_dir();
    let _ = std::fs::create_dir_all(&watch_dir);

    let _ = ThemeRegistry::watch_dir(watch_dir, cx, |cx| {
        sync_palette(cx);
    });

    apply_theme(DEFAULT_DARK_THEME, None, cx);
    sync_palette(cx);

    cx.observe_global::<ThemeRegistry>(|cx| {
        sync_palette(cx);
    })
    .detach();
}

fn load_bundled_themes(cx: &mut App) {
    let path = bundled_themes_dir().join("vortex.json");
    let Ok(content) = std::fs::read_to_string(&path) else {
        tracing::warn!("Bundled theme missing at {:?}", path);
        return;
    };

    let Ok(set) = serde_json::from_str::<ThemeSet>(&content) else {
        tracing::warn!("Failed to parse bundled theme at {:?}", path);
        return;
    };

    let mut dark_default = None;
    let mut light_default = None;

    for theme in set.themes {
        let config = Rc::new(theme);
        let name = config.name.as_ref();

        if config.mode.is_dark() {
            if config.is_default || name == DEFAULT_DARK_THEME {
                dark_default = Some(config.clone());
            }
        } else if config.is_default || name == DEFAULT_LIGHT_THEME {
            light_default = Some(config.clone());
        }
    }

    let theme_state = Theme::global_mut(cx);
    if let Some(config) = dark_default {
        theme_state.dark_theme = config;
    }
    if let Some(config) = light_default {
        theme_state.light_theme = config;
    }
}

/// Switch to a named theme (must exist in the registry).
pub fn apply_theme(name: &str, window: Option<&mut Window>, cx: &mut App) {
    let name = SharedString::from(name.to_string());
    let theme_state = Theme::global(cx);

    let config = ThemeRegistry::global(cx)
        .themes()
        .get(&name)
        .cloned()
        .or_else(|| {
            if theme_state.dark_theme.name == name {
                Some(theme_state.dark_theme.clone())
            } else if theme_state.light_theme.name == name {
                Some(theme_state.light_theme.clone())
            } else {
                None
            }
        });

    let Some(config) = config else {
        tracing::warn!("Theme {:?} not found, keeping current theme", name);
        return;
    };

    let theme = Theme::global_mut(cx);
    if config.mode.is_dark() {
        theme.dark_theme = config;
        Theme::change(ThemeMode::Dark, window, cx);
    } else {
        theme.light_theme = config;
        Theme::change(ThemeMode::Light, window, cx);
    }

    sync_palette(cx);
}

/// List all registered theme names (bundled + community).
#[allow(dead_code)]
pub fn available_themes(cx: &App) -> Vec<SharedString> {
    ThemeRegistry::global(cx)
        .sorted_themes()
        .into_iter()
        .map(|t| t.name.clone())
        .collect()
}

/// Themes matching the given appearance mode.
pub fn themes_for_mode(cx: &App, dark: bool) -> Vec<SharedString> {
    ThemeRegistry::global(cx)
        .sorted_themes()
        .into_iter()
        .filter(|t| t.mode.is_dark() == dark)
        .map(|t| t.name.clone())
        .collect()
}

/// Active appearance mode.
pub fn current_mode(cx: &App) -> ThemeMode {
    Theme::global(cx).mode
}

/// Name of the theme config for the active mode.
pub fn current_theme_name(cx: &App) -> SharedString {
    Theme::global(cx).theme_name().clone()
}

/// Switch light/dark without changing the per-mode theme selection.
pub fn set_appearance_mode(mode: ThemeMode, window: Option<&mut Window>, cx: &mut App) {
    Theme::change(mode, window, cx);
    sync_palette(cx);
}
