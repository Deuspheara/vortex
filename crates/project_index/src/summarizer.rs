use sha2::{Digest, Sha256};

use crate::symbol_index::ExtractedSymbol;

/// Version string baked into summary cache keys so prompt/heuristic changes invalidate entries.
pub const SUMMARIZER_PROMPT_VERSION: &str = "heuristic-v1";

/// Hook for file and directory summarization. Phase 3 ships a deterministic heuristic impl;
/// `agent_core` can plug an LLM-backed summarizer later.
pub trait Summarizer: Send + Sync {
    fn summarize_file(
        &self,
        path: &str,
        language: Option<&str>,
        source: &str,
        symbols: &[ExtractedSymbol],
    ) -> String;

    fn summarize_directory(&self, path: &str, child_entries: &[(&str, &str)]) -> String;
}

/// Deterministic summarizer: first doc comment + top symbol names + a short purpose line.
#[derive(Clone, Copy, Debug, Default)]
pub struct HeuristicSummarizer;

impl HeuristicSummarizer {
    pub fn new() -> Self {
        Self
    }
}

impl Summarizer for HeuristicSummarizer {
    fn summarize_file(
        &self,
        _path: &str,
        language: Option<&str>,
        source: &str,
        symbols: &[ExtractedSymbol],
    ) -> String {
        let doc = first_doc_comment(source, language);
        let names: Vec<&str> = symbols.iter().take(5).map(|s| s.name.as_str()).collect();
        let purpose = doc
            .as_deref()
            .or_else(|| first_line_comment(source, language))
            .map(|s| s.to_string())
            .unwrap_or_else(|| infer_purpose_from_symbols(&names, language));

        if names.is_empty() {
            purpose
        } else if doc.is_some() {
            format!("{}, {}", names.join(", "), purpose)
        } else {
            format!("{} — {}", names.join(", "), purpose)
        }
    }

    fn summarize_directory(&self, _path: &str, child_entries: &[(&str, &str)]) -> String {
        if child_entries.is_empty() {
            return "empty directory".into();
        }
        let names: Vec<&str> = child_entries.iter().take(6).map(|(n, _)| *n).collect();
        let hints: Vec<&str> = child_entries
            .iter()
            .take(3)
            .filter_map(|(_, s)| if s.is_empty() { None } else { Some(*s) })
            .collect();
        if hints.is_empty() {
            format!("contains {}", names.join(", "))
        } else {
            format!("contains {} ({})", names.join(", "), hints.join("; "))
        }
    }
}

/// Cache key: sha256(path + content_hash + summarizer_prompt_version).
pub fn summary_cache_key(path: &str, content_hash: &str, prompt_version: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    hasher.update(content_hash.as_bytes());
    hasher.update(prompt_version.as_bytes());
    hex_digest(&hasher.finalize())
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn first_doc_comment(source: &str, language: Option<&str>) -> Option<String> {
    match language {
        Some("rust") => extract_rust_doc(source),
        Some("python") => extract_python_doc(source),
        Some("typescript") | Some("tsx") | Some("javascript") | Some("jsx") => {
            extract_js_doc(source)
        }
        _ => extract_rust_doc(source)
            .or_else(|| extract_python_doc(source))
            .or_else(|| extract_js_doc(source)),
    }
}

fn extract_rust_doc(source: &str) -> Option<String> {
    let mut lines = Vec::new();
    for line in source.lines().take(20) {
        let trimmed = line.trim();
        if trimmed.starts_with("//!") {
            lines.push(trimmed.trim_start_matches("//!").trim());
        } else if trimmed.starts_with("///") {
            lines.push(trimmed.trim_start_matches("///").trim());
        } else if !trimmed.is_empty() && !trimmed.starts_with('#') && !lines.is_empty() {
            break;
        }
    }
    join_doc_lines(&lines)
}

fn extract_python_doc(source: &str) -> Option<String> {
    let trimmed = source.trim_start();
    if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
        let quote = if trimmed.starts_with("\"\"\"") {
            "\"\"\""
        } else {
            "'''"
        };
        let rest = &trimmed[3..];
        if let Some(end) = rest.find(quote) {
            let inner = rest[..end].trim();
            return join_doc_lines(&inner.lines().map(|l| l.trim()).collect::<Vec<_>>());
        }
    }
    None
}

fn extract_js_doc(source: &str) -> Option<String> {
    let trimmed = source.trim_start();
    if !trimmed.starts_with("/**") {
        return None;
    }
    let mut lines = Vec::new();
    for line in trimmed.lines().take(15) {
        let t = line.trim();
        if t.starts_with("/**") || t.starts_with("*") || t.starts_with("*/") {
            let content = t
                .trim_start_matches("/**")
                .trim_start_matches("*/")
                .trim_start_matches('*')
                .trim();
            if !content.is_empty() {
                lines.push(content);
            }
        } else {
            break;
        }
    }
    join_doc_lines(&lines)
}

fn first_line_comment<'a>(source: &'a str, language: Option<&str>) -> Option<&'a str> {
    for line in source.lines().take(10) {
        let t = line.trim();
        match language {
            Some("python") if t.starts_with('#') => {
                return Some(t.trim_start_matches('#').trim());
            }
            Some("rust") if t.starts_with("//") && !t.starts_with("///") => {
                return Some(t.trim_start_matches("//").trim());
            }
            _ if t.starts_with("//") => return Some(t.trim_start_matches("//").trim()),
            _ => {}
        }
    }
    None
}

fn join_doc_lines(lines: &[&str]) -> Option<String> {
    let joined: Vec<&str> = lines.iter().copied().filter(|l| !l.is_empty()).collect();
    if joined.is_empty() {
        None
    } else {
        Some(joined.join(" ").chars().take(200).collect())
    }
}

fn infer_purpose_from_symbols(names: &[&str], language: Option<&str>) -> String {
    if names.is_empty() {
        return match language {
            Some(lang) => format!("{lang} source file"),
            None => "source file".into(),
        };
    }
    format!("defines {}", names.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol_index::ExtractedSymbol;

    #[test]
    fn summary_key_is_stable() {
        let k1 = summary_cache_key("a.rs", "abc", SUMMARIZER_PROMPT_VERSION);
        let k2 = summary_cache_key("a.rs", "abc", SUMMARIZER_PROMPT_VERSION);
        assert_eq!(k1, k2);
        assert_ne!(
            k1,
            summary_cache_key("a.rs", "def", SUMMARIZER_PROMPT_VERSION)
        );
    }

    #[test]
    fn heuristic_uses_doc_and_symbols() {
        let src = "/// Builds prompt context.\n\npub struct ContextBuilder;\n";
        let symbols = vec![ExtractedSymbol {
            name: "ContextBuilder".into(),
            kind: "struct".into(),
            start_line: 3,
            end_line: 3,
            signature: None,
        }];
        let s = HeuristicSummarizer.summarize_file("builder.rs", Some("rust"), src, &symbols);
        assert!(s.contains("ContextBuilder"));
        assert!(s.contains("prompt") || s.contains("Builds"));
    }
}
