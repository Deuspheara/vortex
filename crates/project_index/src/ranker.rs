use std::collections::{HashMap, HashSet};

use crate::cache::CachedSymbol;
use crate::file_scanner::ScannedFile;

/// A ranked search hit (scores are internal; callers format compact model-facing output).
#[derive(Clone, Debug)]
pub struct RankHit {
    pub path: String,
    pub symbol_id: String,
    pub name: String,
    pub kind: String,
    pub start_line: u32,
    pub end_line: u32,
    pub signature: Option<String>,
    pub score: f64,
}

const W_LEXICAL: f64 = 0.35;
const W_SYMBOL: f64 = 0.30;
const W_PATH: f64 = 0.15;
const W_RECENT: f64 = 0.10;
const W_DEP: f64 = 0.10;

/// Hybrid ranker: lexical + symbol + path + recent_change + dependency_proximity.
pub fn rank_symbols(
    query: &str,
    symbols: &[CachedSymbol],
    files: &[ScannedFile],
    imports_by_path: &HashMap<String, Vec<String>>,
    changed_files: &HashSet<String>,
    anchor_paths: &HashSet<String>,
    limit: usize,
) -> Vec<RankHit> {
    let q = query.trim().to_lowercase();
    if q.is_empty() || limit == 0 {
        return Vec::new();
    }
    let tokens: Vec<&str> = q.split_whitespace().collect();
    let file_mtime: HashMap<String, i64> = files
        .iter()
        .map(|f| (f.path.to_string_lossy().replace('\\', "/"), f.mtime))
        .collect();
    let max_mtime = file_mtime.values().copied().max().unwrap_or(0);

    let mut hits: Vec<RankHit> = symbols
        .iter()
        .filter_map(|sym| {
            let name_l = sym.name.to_lowercase();
            let path_l = sym.path.to_lowercase();
            let sig_l = sym.signature.as_deref().unwrap_or("").to_lowercase();

            let lexical = token_match_score(&tokens, &name_l)
                .max(token_match_score(&tokens, &path_l) * 0.7)
                .max(token_match_score(&tokens, &sig_l) * 0.5);

            let symbol = if name_l == q {
                1.0
            } else if name_l.contains(&q) {
                0.85
            } else if tokens.iter().all(|t| name_l.contains(t)) {
                0.7
            } else {
                0.0
            };

            let path = if path_l.contains(&q) {
                1.0
            } else {
                tokens
                    .iter()
                    .map(|t| if path_l.contains(t) { 1.0 } else { 0.0 })
                    .fold(0.0, f64::max)
            };

            let recent = if changed_files.contains(&sym.path) {
                1.0
            } else {
                let mtime = file_mtime.get(&sym.path).copied().unwrap_or(0);
                if max_mtime > 0 {
                    (mtime as f64 / max_mtime as f64).clamp(0.0, 1.0) * 0.5
                } else {
                    0.0
                }
            };

            let dep = dependency_score(&sym.path, imports_by_path, anchor_paths);

            let score = lexical * W_LEXICAL
                + symbol * W_SYMBOL
                + path * W_PATH
                + recent * W_RECENT
                + dep * W_DEP;

            if score < 0.05 {
                return None;
            }

            Some(RankHit {
                path: sym.path.clone(),
                symbol_id: sym.id.clone(),
                name: sym.name.clone(),
                kind: sym.kind.clone(),
                start_line: sym.start_line,
                end_line: sym.end_line,
                signature: sym.signature.clone(),
                score,
            })
        })
        .collect();

    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.start_line.cmp(&b.start_line))
    });
    hits.truncate(limit);
    hits
}

fn token_match_score(tokens: &[&str], haystack: &str) -> f64 {
    if tokens.is_empty() {
        return 0.0;
    }
    let matched = tokens.iter().filter(|t| haystack.contains(**t)).count();
    matched as f64 / tokens.len() as f64
}

fn dependency_score(
    path: &str,
    imports_by_path: &HashMap<String, Vec<String>>,
    anchors: &HashSet<String>,
) -> f64 {
    if anchors.is_empty() {
        return 0.0;
    }
    if anchors.contains(path) {
        return 1.0;
    }
    for anchor in anchors {
        if imports_by_path
            .get(anchor)
            .is_some_and(|imps| imps.iter().any(|t| path_contains(path, t)))
        {
            return 0.9;
        }
        if imports_by_path
            .get(path)
            .is_some_and(|imps| imps.iter().any(|t| path_contains(anchor, t)))
        {
            return 0.85;
        }
    }
    0.0
}

fn path_contains(path: &str, import_target: &str) -> bool {
    let path_l = path.to_lowercase();
    let target = import_target.to_lowercase();
    path_l.contains(&target) || target.contains(&path_l)
}

/// Build import adjacency from raw import target strings.
pub fn build_import_map(imports: &[(String, String)]) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for (path, target) in imports {
        map.entry(path.clone()).or_default().push(target.clone());
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::CachedSymbol;

    fn sym(path: &str, name: &str, kind: &str) -> CachedSymbol {
        CachedSymbol {
            id: format!("{path}#{kind}#{name}#1"),
            path: path.into(),
            name: name.into(),
            kind: kind.into(),
            start_line: 1,
            end_line: 10,
            signature: None,
            summary: None,
        }
    }

    #[test]
    fn exact_name_ranks_first() {
        let symbols = vec![
            sym("a.rs", "Alpha", "struct"),
            sym("b.rs", "DeltaCoalescer", "struct"),
            sym("c.rs", "Other", "fn"),
        ];
        let mut changed = HashSet::new();
        changed.insert("c.rs".into());
        let hits = rank_symbols(
            "DeltaCoalescer",
            &symbols,
            &[],
            &HashMap::new(),
            &changed,
            &HashSet::new(),
            5,
        );
        assert!(!hits.is_empty());
        assert_eq!(hits[0].name, "DeltaCoalescer");
    }

    #[test]
    fn changed_file_gets_boost() {
        let symbols = vec![sym("a.rs", "Foo", "fn"), sym("b.rs", "Foo", "fn")];
        let mut changed = HashSet::new();
        changed.insert("b.rs".into());
        let hits = rank_symbols(
            "Foo",
            &symbols,
            &[],
            &HashMap::new(),
            &changed,
            &HashSet::new(),
            5,
        );
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].path, "b.rs");
    }
}
