use regex::Regex;
use tree_sitter::{Node, Parser, TreeCursor};

/// A symbol extracted from source (before persistence).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractedSymbol {
    pub name: String,
    pub kind: String,
    pub start_line: u32,
    pub end_line: u32,
    pub signature: Option<String>,
}

/// An import/use statement extracted from source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractedImport {
    pub target: String,
    pub kind: Option<String>,
}

/// Stable symbol id for SQLite storage.
pub fn symbol_id(path: &str, kind: &str, name: &str, start_line: u32) -> String {
    format!("{path}#{kind}#{name}#{start_line}")
}

/// Extract top-level symbols and imports for `language` from `source`.
pub fn extract_symbols_and_imports(
    language: Option<&str>,
    source: &str,
) -> (Vec<ExtractedSymbol>, Vec<ExtractedImport>) {
    match language {
        Some("rust") => extract_rust(source),
        Some("typescript") | Some("tsx") => extract_typescript(source, language == Some("tsx")),
        Some("javascript") | Some("jsx") => extract_javascript(source),
        Some("python") => extract_python(source),
        _ => extract_regex_fallback(source, language),
    }
}

fn extract_rust(source: &str) -> (Vec<ExtractedSymbol>, Vec<ExtractedImport>) {
    let mut symbols = Vec::new();
    let mut imports = Vec::new();
    if let Some(tree) = parse(source, tree_sitter_rust::LANGUAGE.into()) {
        walk_rust(tree.root_node(), source, &mut symbols, &mut imports, 0);
    }
    if symbols.is_empty() {
        symbols = regex_symbols(source, language_patterns("rust"));
    }
    if imports.is_empty() {
        imports = regex_imports(source, "rust");
    }
    (symbols, imports)
}

fn extract_typescript(source: &str, tsx: bool) -> (Vec<ExtractedSymbol>, Vec<ExtractedImport>) {
    let mut symbols = Vec::new();
    let mut imports = Vec::new();
    let lang = if tsx {
        tree_sitter_typescript::LANGUAGE_TSX
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT
    };
    if let Some(tree) = parse(source, lang.into()) {
        walk_ts(tree.root_node(), source, &mut symbols, &mut imports);
    }
    if symbols.is_empty() {
        symbols = regex_symbols(source, language_patterns("typescript"));
    }
    if imports.is_empty() {
        imports = regex_imports(source, "typescript");
    }
    (symbols, imports)
}

fn extract_javascript(source: &str) -> (Vec<ExtractedSymbol>, Vec<ExtractedImport>) {
    let mut symbols = Vec::new();
    let mut imports = Vec::new();
    if let Some(tree) = parse(source, tree_sitter_javascript::LANGUAGE.into()) {
        walk_ts(tree.root_node(), source, &mut symbols, &mut imports);
    }
    if symbols.is_empty() {
        symbols = regex_symbols(source, language_patterns("javascript"));
    }
    if imports.is_empty() {
        imports = regex_imports(source, "javascript");
    }
    (symbols, imports)
}

fn extract_python(source: &str) -> (Vec<ExtractedSymbol>, Vec<ExtractedImport>) {
    let mut symbols = Vec::new();
    let mut imports = Vec::new();
    if let Some(tree) = parse(source, tree_sitter_python::LANGUAGE.into()) {
        walk_python(tree.root_node(), source, &mut symbols, &mut imports);
    }
    if symbols.is_empty() {
        symbols = regex_symbols(source, language_patterns("python"));
    }
    if imports.is_empty() {
        imports = regex_imports(source, "python");
    }
    (symbols, imports)
}

fn extract_regex_fallback(
    source: &str,
    language: Option<&str>,
) -> (Vec<ExtractedSymbol>, Vec<ExtractedImport>) {
    let lang = language.unwrap_or("unknown");
    (
        regex_symbols(source, language_patterns(lang)),
        regex_imports(source, lang),
    )
}

fn parse(source: &str, language: tree_sitter::Language) -> Option<tree_sitter::Tree> {
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    parser.parse(source, None)
}

fn walk_rust(
    node: Node,
    source: &str,
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
    depth: u32,
) {
    let kind = node.kind();
    match kind {
        "function_item" | "struct_item" | "enum_item" | "trait_item" | "type_item"
        | "const_item" | "static_item" | "mod_item" | "macro_definition" => {
            if depth <= 1 {
                if let Some(name) = node
                    .child_by_field_name("name")
                    .and_then(|n| node_text(n, source))
                {
                    let sym_kind = rust_symbol_kind(kind);
                    symbols.push(ExtractedSymbol {
                        name,
                        kind: sym_kind.to_string(),
                        start_line: node.start_position().row as u32 + 1,
                        end_line: node.end_position().row as u32 + 1,
                        signature: Some(signature_line(node, source)),
                    });
                }
            }
        }
        "impl_item" => {
            if depth <= 1 {
                let type_name = node
                    .child_by_field_name("type")
                    .and_then(|n| node_text(n, source))
                    .unwrap_or_else(|| "impl".to_string());
                symbols.push(ExtractedSymbol {
                    name: type_name,
                    kind: "impl".into(),
                    start_line: node.start_position().row as u32 + 1,
                    end_line: node.end_position().row as u32 + 1,
                    signature: Some(signature_line(node, source)),
                });
            }
        }
        "use_declaration" | "extern_crate_declaration" => {
            if let Some(text) = node_text(node, source) {
                imports.push(ExtractedImport {
                    target: text.trim().to_string(),
                    kind: Some("use".into()),
                });
            }
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let child_depth = if is_rust_top_level(kind) {
            depth + 1
        } else {
            depth
        };
        walk_rust(child, source, symbols, imports, child_depth);
    }
}

fn is_rust_top_level(kind: &str) -> bool {
    matches!(
        kind,
        "source_file" | "mod_item" | "declaration_list" | "impl_item"
    )
}

fn rust_symbol_kind(node_kind: &str) -> &'static str {
    match node_kind {
        "function_item" => "function",
        "struct_item" => "struct",
        "enum_item" => "enum",
        "trait_item" => "trait",
        "type_item" => "type",
        "const_item" | "static_item" => "const",
        "mod_item" => "module",
        "macro_definition" => "macro",
        _ => "symbol",
    }
}

fn walk_ts(
    node: Node,
    source: &str,
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
) {
    match node.kind() {
        "function_declaration"
        | "class_declaration"
        | "interface_declaration"
        | "type_alias_declaration"
        | "enum_declaration"
        | "method_definition"
        | "lexical_declaration"
        | "variable_declaration" => {
            if node.parent().map(|p| p.kind()) == Some("program")
                || node.parent().map(|p| p.kind()) == Some("statement_block")
                || is_exported(node, source)
            {
                if let Some(name) = ts_symbol_name(node, source) {
                    symbols.push(ExtractedSymbol {
                        name,
                        kind: ts_symbol_kind(node.kind()).to_string(),
                        start_line: node.start_position().row as u32 + 1,
                        end_line: node.end_position().row as u32 + 1,
                        signature: Some(signature_line(node, source)),
                    });
                }
            }
        }
        "import_statement" | "import_declaration" => {
            if let Some(text) = node_text(node, source) {
                imports.push(ExtractedImport {
                    target: text.trim().to_string(),
                    kind: Some("import".into()),
                });
            }
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_ts(child, source, symbols, imports);
    }
}

fn walk_python(
    node: Node,
    source: &str,
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
) {
    match node.kind() {
        "function_definition" | "class_definition" => {
            if is_python_toplevel(node) {
                if let Some(name) = node
                    .child_by_field_name("name")
                    .and_then(|n| node_text(n, source))
                {
                    symbols.push(ExtractedSymbol {
                        name,
                        kind: if node.kind() == "class_definition" {
                            "class".into()
                        } else {
                            "function".into()
                        },
                        start_line: node.start_position().row as u32 + 1,
                        end_line: node.end_position().row as u32 + 1,
                        signature: Some(signature_line(node, source)),
                    });
                }
            }
        }
        "import_statement" | "import_from_statement" => {
            if let Some(text) = node_text(node, source) {
                imports.push(ExtractedImport {
                    target: text.trim().to_string(),
                    kind: Some("import".into()),
                });
            }
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_python(child, source, symbols, imports);
    }
}

fn is_python_toplevel(node: Node) -> bool {
    node.parent()
        .map(|p| matches!(p.kind(), "module" | "block"))
        .unwrap_or(true)
}

fn is_exported(node: Node, source: &str) -> bool {
    node.prev_sibling()
        .map(|s| {
            s.kind() == "export_statement"
                || node_text(s, source).is_some_and(|t| t.contains("export"))
        })
        .unwrap_or(false)
}

fn ts_symbol_name(node: Node, source: &str) -> Option<String> {
    if let Some(name) = node
        .child_by_field_name("name")
        .and_then(|n| node_text(n, source))
    {
        return Some(name);
    }
    // const foo = ...
    let mut cursor: TreeCursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            if let Some(id) = child
                .child_by_field_name("name")
                .and_then(|n| node_text(n, source))
            {
                return Some(id);
            }
        }
    }
    None
}

fn ts_symbol_kind(node_kind: &str) -> &'static str {
    match node_kind {
        "function_declaration" => "function",
        "class_declaration" => "class",
        "interface_declaration" => "interface",
        "type_alias_declaration" => "type",
        "enum_declaration" => "enum",
        "method_definition" => "method",
        _ => "symbol",
    }
}

fn node_text(node: Node, source: &str) -> Option<String> {
    node.utf8_text(source.as_bytes())
        .ok()
        .map(|s| s.to_string())
}

fn signature_line(node: Node, source: &str) -> String {
    let start = node.start_position();
    let end = node.end_position();
    if start.row == end.row {
        return node_text(node, source)
            .unwrap_or_default()
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
    }
    let lines: Vec<&str> = source.lines().collect();
    let row = start.row;
    if row < lines.len() {
        lines[row].trim().to_string()
    } else {
        String::new()
    }
}

struct RegexPattern {
    kind: &'static str,
    re: Regex,
}

fn language_patterns(lang: &str) -> Vec<RegexPattern> {
    let patterns: &[(&str, &str)] = match lang {
        "rust" => &[
            (
                "function",
                r"(?m)^(?:pub\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+(\w+)",
            ),
            ("struct", r"(?m)^(?:pub\s+)?struct\s+(\w+)"),
            ("enum", r"(?m)^(?:pub\s+)?enum\s+(\w+)"),
            ("trait", r"(?m)^(?:pub\s+)?trait\s+(\w+)"),
            ("impl", r"(?m)^impl(?:<[^>]+>)?\s+(?:\w+\s+for\s+)?(\w+)"),
            ("type", r"(?m)^(?:pub\s+)?type\s+(\w+)"),
            ("module", r"(?m)^(?:pub\s+)?mod\s+(\w+)"),
        ],
        "typescript" | "tsx" | "javascript" | "jsx" => &[
            (
                "function",
                r"(?m)^(?:export\s+)?(?:async\s+)?function\s+(\w+)",
            ),
            ("class", r"(?m)^(?:export\s+)?(?:abstract\s+)?class\s+(\w+)"),
            ("interface", r"(?m)^(?:export\s+)?interface\s+(\w+)"),
            ("type", r"(?m)^(?:export\s+)?type\s+(\w+)"),
            ("const", r"(?m)^(?:export\s+)?const\s+(\w+)\s*="),
        ],
        "python" => &[
            ("function", r"(?m)^(?:async\s+)?def\s+(\w+)"),
            ("class", r"(?m)^class\s+(\w+)"),
        ],
        _ => &[
            ("function", r"(?m)^(?:pub\s+)?(?:async\s+)?fn\s+(\w+)"),
            ("class", r"(?m)^(?:export\s+)?class\s+(\w+)"),
            ("function", r"(?m)^def\s+(\w+)"),
        ],
    };
    patterns
        .iter()
        .filter_map(|(kind, pat)| Regex::new(pat).ok().map(|re| RegexPattern { kind, re }))
        .collect()
}

fn regex_symbols(source: &str, patterns: Vec<RegexPattern>) -> Vec<ExtractedSymbol> {
    let mut out = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    for pat in patterns {
        for cap in pat.re.captures_iter(source) {
            let Some(name) = cap.get(1).map(|m| m.as_str().to_string()) else {
                continue;
            };
            let line_idx = source[..cap.get(0).unwrap().start()]
                .chars()
                .filter(|c| *c == '\n')
                .count();
            let start_line = line_idx as u32 + 1;
            let sig = lines.get(line_idx).map(|l| l.trim().to_string());
            if out
                .iter()
                .any(|s: &ExtractedSymbol| s.name == name && s.start_line == start_line)
            {
                continue;
            }
            out.push(ExtractedSymbol {
                name,
                kind: pat.kind.to_string(),
                start_line,
                end_line: start_line,
                signature: sig,
            });
        }
    }
    out.sort_by_key(|s| s.start_line);
    out
}

fn regex_imports(source: &str, lang: &str) -> Vec<ExtractedImport> {
    let patterns: &[&str] = match lang {
        "rust" => &[r"(?m)^use\s+[^;]+;"],
        "python" => &[r"(?m)^(?:from\s+\S+\s+)?import\s+.+"],
        _ => &[
            r"(?m)^import\s+.+",
            r#"(?m)^import\s+\{[^}]+\}\s+from\s+['"][^'"]+['"]"#,
        ],
    };
    let mut out = Vec::new();
    for pat in patterns {
        let Ok(re) = Regex::new(pat) else {
            continue;
        };
        for cap in re.find_iter(source) {
            out.push(ExtractedImport {
                target: cap.as_str().trim().to_string(),
                kind: Some("import".into()),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_symbols_have_line_ranges() {
        let src = "fn alpha() {}\n\npub struct Beta;\n\nenum Gamma { A, B }\n";
        let (symbols, _) = extract_symbols_and_imports(Some("rust"), src);
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "alpha" && s.kind == "function")
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Beta" && s.kind == "struct")
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Gamma" && s.kind == "enum")
        );
        let alpha = symbols.iter().find(|s| s.name == "alpha").unwrap();
        assert_eq!(alpha.start_line, 1);
    }

    #[test]
    fn typescript_extracts_class() {
        let src = "export class DeltaCoalescer {\n  batch() {}\n}\n";
        let (symbols, _) = extract_symbols_and_imports(Some("typescript"), src);
        assert!(symbols.iter().any(|s| s.name == "DeltaCoalescer"));
    }

    #[test]
    fn python_extracts_def() {
        let src = "def helper():\n    pass\n\nclass Worker:\n    pass\n";
        let (symbols, _) = extract_symbols_and_imports(Some("python"), src);
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "helper" && s.kind == "function")
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "Worker" && s.kind == "class")
        );
    }

    #[test]
    fn regex_fallback_for_unknown_language() {
        let src = "fn mystery() {}\n";
        let (symbols, _) = extract_symbols_and_imports(Some("zig"), src);
        assert!(symbols.iter().any(|s| s.name == "mystery"));
    }
}
