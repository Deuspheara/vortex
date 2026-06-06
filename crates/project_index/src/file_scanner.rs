use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use globset::{Glob, GlobMatcher};
use sha2::{Digest, Sha256};

const DEFAULT_MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const BINARY_SNIFF_BYTES: usize = 8 * 1024;
const BUILTIN_EXCLUDES: &[&str] = &[
    "target/",
    "node_modules/",
    "dist/",
    "build/",
    ".next/",
    ".nuxt/",
    ".turbo/",
    ".cache/",
    "coverage/",
    ".venv/",
    "venv/",
    ".git/",
    "DerivedData/",
    "Pods/",
    "vendor/bundle/",
];

/// A single file discovered by the scanner, relative to the scan root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannedFile {
    /// Path relative to the scan root, using the platform separator.
    pub path: PathBuf,
    /// Coarse language label derived from the extension, if recognized.
    pub language: Option<String>,
    /// Size in bytes.
    pub size: u64,
    /// Last-modified time as seconds since the Unix epoch (0 if unavailable).
    pub mtime: i64,
    /// Lowercase hex sha256 of the file contents.
    pub content_hash: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexSkipReason {
    Ignore,
    Hidden,
    Binary,
    Large,
    Policy,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanStats {
    pub files_indexed: usize,
    pub skipped_ignore: usize,
    pub skipped_hidden: usize,
    pub skipped_binary: usize,
    pub skipped_large: usize,
    pub skipped_policy: usize,
}

impl ScanStats {
    fn bump(&mut self, reason: IndexSkipReason) {
        match reason {
            IndexSkipReason::Ignore => self.skipped_ignore += 1,
            IndexSkipReason::Hidden => self.skipped_hidden += 1,
            IndexSkipReason::Binary => self.skipped_binary += 1,
            IndexSkipReason::Large => self.skipped_large += 1,
            IndexSkipReason::Policy => self.skipped_policy += 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanOutcome {
    pub files: Vec<ScannedFile>,
    pub stats: ScanStats,
    pub active_ignore_sources: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct IndexPolicy {
    pub max_file_bytes: u64,
    built_in_rules: Vec<CompiledRule>,
    root_rules: Vec<CompiledRule>,
    active_sources: Vec<String>,
}

#[derive(Clone, Debug)]
struct CompiledRule {
    include: bool,
    matchers: Vec<GlobMatcher>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuleDecision {
    Include,
    Exclude(IndexSkipReason),
}

impl IndexPolicy {
    pub fn discover(root: &Path) -> Self {
        let mut active_sources = vec!["built-in".to_string()];
        let built_in_rules = BUILTIN_EXCLUDES
            .iter()
            .filter_map(|pattern| compile_rule(pattern))
            .collect();
        let mut root_rules = Vec::new();
        for name in [".gitignore", ".ignore", ".rgignore", ".vortexignore"] {
            let path = root.join(name);
            if !path.is_file() {
                continue;
            }
            if let Ok(contents) = std::fs::read_to_string(&path) {
                active_sources.push(name.to_string());
                for line in contents.lines() {
                    if let Some(rule) = compile_rule(line) {
                        root_rules.push(rule);
                    }
                }
            }
        }
        Self {
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            built_in_rules,
            root_rules,
            active_sources,
        }
    }

    pub fn active_sources(&self) -> &[String] {
        &self.active_sources
    }

    fn classify_dir(&self, rel: &str) -> Option<IndexSkipReason> {
        if is_hidden_rel(rel) {
            return Some(IndexSkipReason::Hidden);
        }
        match self.rule_decision(rel, true) {
            Some(RuleDecision::Exclude(reason)) => Some(reason),
            _ => None,
        }
    }

    fn classify_file(&self, rel: &str, metadata: &std::fs::Metadata) -> Option<IndexSkipReason> {
        if is_hidden_rel(rel) {
            return Some(IndexSkipReason::Hidden);
        }
        match self.rule_decision(rel, false) {
            Some(RuleDecision::Exclude(reason)) => return Some(reason),
            Some(RuleDecision::Include) | None => {}
        }
        if metadata.len() > self.max_file_bytes {
            return Some(IndexSkipReason::Large);
        }
        None
    }

    fn rule_decision(&self, rel: &str, is_dir: bool) -> Option<RuleDecision> {
        let rel = rel.replace('\\', "/");
        let mut decision = None;
        for rule in &self.built_in_rules {
            if rule.matches(&rel, is_dir) {
                decision = Some(if rule.include {
                    RuleDecision::Include
                } else {
                    RuleDecision::Exclude(IndexSkipReason::Policy)
                });
            }
        }
        for rule in &self.root_rules {
            if rule.matches(&rel, is_dir) {
                decision = Some(if rule.include {
                    RuleDecision::Include
                } else {
                    RuleDecision::Exclude(IndexSkipReason::Ignore)
                });
            }
        }
        decision
    }
}

impl CompiledRule {
    fn matches(&self, rel: &str, is_dir: bool) -> bool {
        if !is_dir && rel.ends_with('/') {
            return false;
        }
        self.matchers.iter().any(|matcher| matcher.is_match(rel))
    }
}

/// Walk `root` with the same ignore rules used by [`crate::ProjectIndex`] and return one
/// [`ScannedFile`] per regular file (path, language, size, mtime, content hash).
///
/// This reuses `ignore::WalkBuilder` exactly like `search.rs` so the index and search see the
/// same file set (respecting `.gitignore` / `.git/info/exclude`).
pub fn scan_files(root: &Path, respect_git_ignore: bool) -> Result<Vec<ScannedFile>, String> {
    Ok(scan_files_with_policy(root, respect_git_ignore)?.files)
}

pub fn scan_files_with_policy(
    root: &Path,
    respect_git_ignore: bool,
) -> Result<ScanOutcome, String> {
    let policy = if respect_git_ignore {
        IndexPolicy::discover(root)
    } else {
        IndexPolicy {
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            built_in_rules: Vec::new(),
            root_rules: Vec::new(),
            active_sources: Vec::new(),
        }
    };
    let mut files = Vec::new();
    let mut stats = ScanStats::default();
    walk_dir(root, root, &policy, &mut files, &mut stats)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    stats.files_indexed = files.len();
    Ok(ScanOutcome {
        files,
        stats,
        active_ignore_sources: policy.active_sources().to_vec(),
    })
}

fn walk_dir(
    root: &Path,
    dir: &Path,
    policy: &IndexPolicy,
    files: &mut Vec<ScannedFile>,
    stats: &mut ScanStats,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    let mut ordered = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        ordered.insert(entry.path());
    }

    for path in ordered {
        let rel_path = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let rel = rel_path.to_string_lossy().replace('\\', "/");
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if let Some(reason) = policy.classify_dir(&ensure_dir_suffix(&rel)) {
                stats.bump(reason);
                continue;
            }
            walk_dir(root, &path, policy, files, stats)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        if let Some(reason) = policy.classify_file(&rel, &metadata) {
            stats.bump(reason);
            continue;
        }
        if is_binary_file(&path)? {
            stats.bump(IndexSkipReason::Binary);
            continue;
        }
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        files.push(ScannedFile {
            path: rel_path,
            language: language_for_path(&path).map(|l| l.to_string()),
            size: metadata.len(),
            mtime: mtime_secs(&metadata),
            content_hash: hash_bytes(&bytes),
        });
    }
    Ok(())
}

fn ensure_dir_suffix(rel: &str) -> String {
    if rel.ends_with('/') {
        rel.to_string()
    } else {
        format!("{rel}/")
    }
}

fn is_hidden_rel(rel: &str) -> bool {
    Path::new(rel)
        .components()
        .any(|component| match component {
            Component::Normal(part) => part.to_string_lossy().starts_with('.'),
            _ => false,
        })
}

fn is_binary_file(path: &Path) -> Result<bool, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf = [0u8; BINARY_SNIFF_BYTES];
    let read = file.read(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf[..read].contains(&0))
}

fn compile_rule(raw: &str) -> Option<CompiledRule> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let (include, pattern) = if let Some(rest) = trimmed.strip_prefix('!') {
        (true, rest.trim())
    } else {
        (false, trimmed)
    };
    if pattern.is_empty() {
        return None;
    }

    let mut variants = Vec::new();
    let normalized = pattern.trim_start_matches('/');
    if normalized.ends_with('/') {
        let prefix = normalized.trim_end_matches('/');
        variants.push(format!("{prefix}/**"));
        variants.push(prefix.to_string());
    } else if normalized.contains('/') {
        variants.push(normalized.to_string());
    } else {
        variants.push(normalized.to_string());
        variants.push(format!("**/{normalized}"));
        variants.push(format!("**/{normalized}/**"));
    }

    let matchers = variants
        .into_iter()
        .filter_map(|variant| Glob::new(&variant).ok().map(|glob| glob.compile_matcher()))
        .collect::<Vec<_>>();
    if matchers.is_empty() {
        None
    } else {
        Some(CompiledRule { include, matchers })
    }
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn mtime_secs(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Coarse language classification by file extension / well-known file name. Returns a stable
/// lowercase label used both in the cache and the compact map.
pub fn language_for_path(path: &Path) -> Option<&'static str> {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        match name {
            "Cargo.toml" | "Cargo.lock" => return Some("toml"),
            "Dockerfile" => return Some("dockerfile"),
            "Makefile" => return Some("makefile"),
            _ => {}
        }
    }
    let ext = path.extension().and_then(|e| e.to_str())?;
    let lang = match ext.to_ascii_lowercase().as_str() {
        "rs" => "rust",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "py" => "python",
        "go" => "go",
        "rb" => "ruby",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "dart" => "dart",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" => "cpp",
        "cs" => "csharp",
        "php" => "php",
        "sh" | "bash" | "zsh" => "shell",
        "toml" => "toml",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" => "markdown",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" | "sass" => "scss",
        "sql" => "sql",
        _ => return None,
    };
    Some(lang)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn write(root: &Path, rel: &str, contents: &[u8]) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut file = fs::File::create(path).unwrap();
        file.write_all(contents).unwrap();
    }

    #[test]
    fn scan_policy_skips_generated_hidden_binary_and_large_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "src/main.rs", b"fn main() {}\n");
        write(root, "target/debug/app", b"compiled");
        write(root, ".hidden/secret.rs", b"pub fn hidden() {}\n");
        write(root, "assets/blob.bin", b"\0\0\0");
        write(
            root,
            "dist/bundle.js",
            &vec![b'a'; (DEFAULT_MAX_FILE_BYTES as usize) + 1],
        );

        let outcome = scan_files_with_policy(root, true).unwrap();
        let scanned: Vec<String> = outcome
            .files
            .iter()
            .map(|file| file.path.to_string_lossy().replace('\\', "/"))
            .collect();

        assert_eq!(scanned, vec!["src/main.rs".to_string()]);
        assert_eq!(outcome.stats.files_indexed, 1);
        assert_eq!(outcome.stats.skipped_policy, 2);
        assert_eq!(outcome.stats.skipped_hidden, 1);
        assert_eq!(outcome.stats.skipped_binary, 1);
    }

    #[test]
    fn vortexignore_can_opt_back_in_after_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, ".gitignore", b"*.rs\n");
        write(root, ".vortexignore", b"!keep.rs\n");
        write(root, "keep.rs", b"pub fn keep() {}\n");
        write(root, "drop.rs", b"pub fn drop() {}\n");

        let outcome = scan_files_with_policy(root, true).unwrap();
        let scanned: Vec<String> = outcome
            .files
            .iter()
            .map(|file| file.path.to_string_lossy().replace('\\', "/"))
            .collect();

        assert_eq!(scanned, vec!["keep.rs".to_string()]);
        assert!(
            outcome
                .active_ignore_sources
                .iter()
                .any(|s| s == ".gitignore")
        );
        assert!(
            outcome
                .active_ignore_sources
                .iter()
                .any(|s| s == ".vortexignore")
        );
        assert_eq!(outcome.stats.skipped_ignore, 1);
    }
}
