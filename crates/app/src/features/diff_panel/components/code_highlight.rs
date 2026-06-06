//! Cached tree-sitter syntax highlighting for markdown code blocks.
//!
//! Highlighting is expensive (query compilation + parse per block). Results are
//! memoized by theme + language + content hash, and skipped while streaming.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Mutex, OnceLock};

use gpui::HighlightStyle;
use gpui_component::Rope;
use gpui_component::highlighter::SyntaxHighlighter;

use crate::tokens::theme::active_highlight_theme;

const MAX_CACHE_ENTRIES: usize = 128;

static HIGHLIGHT_CACHE: OnceLock<Mutex<HashMap<String, Vec<(Range<usize>, HighlightStyle)>>>> =
    OnceLock::new();

fn cache_key(theme: &str, lang: &str, body: &str) -> String {
    let hash = blake3::hash(body.as_bytes()).to_hex();
    format!("{theme}:{lang}:{hash}")
}

/// Returns syntax highlight spans for a code block, using a process-wide cache.
///
/// Pass `enabled: false` while content is still streaming to avoid re-parsing
/// on every frame.
pub fn highlight_code(
    lang: Option<&str>,
    body: &str,
    enabled: bool,
) -> Vec<(Range<usize>, HighlightStyle)> {
    if !enabled || body.is_empty() {
        return Vec::new();
    }

    // Skip expensive tree-sitter work for very large blocks; plain monospace is enough.
    if body.len() > 12_000 {
        return Vec::new();
    }

    let theme = active_highlight_theme();
    let lang = lang.unwrap_or("text");
    let key = cache_key(&theme.name, lang, body);

    let cache = HIGHLIGHT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(hit) = cache.lock().unwrap().get(&key) {
        return hit.clone();
    }

    let mut highlighter = SyntaxHighlighter::new(lang);
    highlighter.update(None, &Rope::from_str(body));
    let styles = highlighter.styles(&(0..body.len()), &theme);

    let mut guard = cache.lock().unwrap();
    if guard.len() >= MAX_CACHE_ENTRIES {
        guard.clear();
    }
    guard.insert(key, styles.clone());
    styles
}
