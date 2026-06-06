use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use regex::{Regex, RegexBuilder};

#[derive(Clone, Debug)]
pub struct SearchContextLine {
    pub line_number: usize,
    pub line: String,
}

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub path: PathBuf,
    pub line_number: usize,
    pub line: String,
    pub before: Vec<SearchContextLine>,
    pub after: Vec<SearchContextLine>,
}

#[derive(Clone, Debug, Default)]
pub struct ListFilesOptions {
    pub base: Option<PathBuf>,
    pub pattern: Option<String>,
    pub exclude: Vec<String>,
    pub max_files: usize,
    pub respect_git_ignore: bool,
    pub sort_by: FileSort,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum FileSort {
    ModifiedDesc,
    #[default]
    PathAsc,
}

#[derive(Clone, Debug)]
pub struct SearchOptions {
    pub query: String,
    pub base: Option<PathBuf>,
    pub include: Option<String>,
    pub exclude: Option<String>,
    pub regex: bool,
    pub case_sensitive: bool,
    pub context_before: usize,
    pub context_after: usize,
    pub max_hits: usize,
    pub max_matches_per_file: usize,
    pub names_only: bool,
    pub respect_git_ignore: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            query: String::new(),
            base: None,
            include: None,
            exclude: None,
            regex: false,
            case_sensitive: false,
            context_before: 0,
            context_after: 0,
            max_hits: 50,
            max_matches_per_file: 10,
            names_only: false,
            respect_git_ignore: true,
        }
    }
}

pub struct ProjectIndex {
    root: PathBuf,
}

impl ProjectIndex {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn list_files(&self, max_files: usize) -> Result<Vec<PathBuf>, String> {
        self.list_files_with_options(&ListFilesOptions {
            max_files,
            ..ListFilesOptions::default()
        })
    }

    pub fn list_files_with_options(
        &self,
        options: &ListFilesOptions,
    ) -> Result<Vec<PathBuf>, String> {
        let base = self.resolve_base(options.base.as_deref())?;
        let include = compile_optional_glob(options.pattern.as_deref())?;
        let exclude = compile_globs(&options.exclude)?;
        let mut files = Vec::new();

        for entry in self.walk(&base, options.respect_git_ignore) {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let abs = entry.path();
            let rel_to_base = abs.strip_prefix(&base).unwrap_or(abs);
            let rel_to_root = abs.strip_prefix(&self.root).unwrap_or(abs).to_path_buf();
            if include
                .as_ref()
                .is_some_and(|set| !set.is_match(rel_to_base))
            {
                continue;
            }
            if exclude.is_match(rel_to_base) || exclude.is_match(&rel_to_root) {
                continue;
            }
            files.push(rel_to_root);
        }

        match options.sort_by {
            FileSort::PathAsc => files.sort(),
            FileSort::ModifiedDesc => {
                files.sort_by_key(|path| std::cmp::Reverse(modified_at(&self.root.join(path))))
            }
        }

        let max = normalize_limit(options.max_files, 200);
        files.truncate(max);
        Ok(files)
    }

    pub fn search(&self, query: &str, max_hits: usize) -> Result<Vec<SearchHit>, String> {
        self.search_with_options(&SearchOptions {
            query: query.to_string(),
            max_hits,
            ..SearchOptions::default()
        })
    }

    pub fn search_with_options(&self, options: &SearchOptions) -> Result<Vec<SearchHit>, String> {
        if options.query.is_empty() {
            return Ok(Vec::new());
        }

        let base = self.resolve_base(options.base.as_deref())?;
        let include = compile_optional_glob(options.include.as_deref())?;
        let exclude = compile_optional_glob(options.exclude.as_deref())?;
        let pattern =
            compile_search_pattern(&options.query, options.regex, options.case_sensitive)?;
        let mut hits = Vec::new();

        'files: for entry in self.walk(&base, options.respect_git_ignore) {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let abs = entry.path();
            let rel_to_base = abs.strip_prefix(&base).unwrap_or(abs);
            if include
                .as_ref()
                .is_some_and(|set| !set.is_match(rel_to_base))
            {
                continue;
            }
            if exclude
                .as_ref()
                .is_some_and(|set| set.is_match(rel_to_base))
            {
                continue;
            }
            if is_binary_extension(abs) {
                continue;
            }
            let content = match fs::read_to_string(abs) {
                Ok(content) => content,
                Err(_) => continue,
            };
            let lines: Vec<&str> = content.lines().collect();
            let mut matches_in_file = 0usize;

            for (idx, line) in lines.iter().enumerate() {
                if !pattern.is_match(line) {
                    continue;
                }

                let path = abs.strip_prefix(&self.root).unwrap_or(abs).to_path_buf();
                if options.names_only {
                    hits.push(SearchHit {
                        path,
                        line_number: idx + 1,
                        line: String::new(),
                        before: Vec::new(),
                        after: Vec::new(),
                    });
                    if hits.len() >= normalize_limit(options.max_hits, 50) {
                        break 'files;
                    }
                    continue 'files;
                }

                hits.push(SearchHit {
                    path,
                    line_number: idx + 1,
                    line: (*line).to_string(),
                    before: collect_context(&lines, idx, options.context_before, true),
                    after: collect_context(&lines, idx, options.context_after, false),
                });
                matches_in_file += 1;

                if hits.len() >= normalize_limit(options.max_hits, 50) {
                    break 'files;
                }
                if matches_in_file >= normalize_limit(options.max_matches_per_file, 10) {
                    continue 'files;
                }
            }
        }

        Ok(hits)
    }

    fn resolve_base(&self, base: Option<&Path>) -> Result<PathBuf, String> {
        match base {
            Some(base) => {
                let joined = self.root.join(base);
                let canonical = joined
                    .canonicalize()
                    .map_err(|e| format!("invalid base path `{}`: {e}", base.display()))?;
                if !canonical.starts_with(&self.root) {
                    return Err(format!(
                        "base path `{}` is outside the project root",
                        base.display()
                    ));
                }
                Ok(canonical)
            }
            None => Ok(self.root.clone()),
        }
    }

    fn walk(&self, base: &Path, respect_git_ignore: bool) -> ignore::Walk {
        let mut builder = WalkBuilder::new(base);
        builder
            .hidden(false)
            .git_ignore(respect_git_ignore)
            .git_global(false)
            .git_exclude(respect_git_ignore);
        builder.build()
    }
}

fn compile_search_pattern(
    query: &str,
    is_regex: bool,
    case_sensitive: bool,
) -> Result<Regex, String> {
    let pattern = if is_regex {
        query.to_string()
    } else {
        regex::escape(query)
    };
    RegexBuilder::new(&pattern)
        .case_insensitive(!case_sensitive)
        .build()
        .map_err(|e| e.to_string())
}

fn compile_optional_glob(pattern: Option<&str>) -> Result<Option<GlobSet>, String> {
    pattern
        .map(|pattern| compile_globs(&[pattern.to_string()]))
        .transpose()
}

fn compile_globs(patterns: &[String]) -> Result<GlobSet, String> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern).map_err(|e| e.to_string())?);
    }
    builder.build().map_err(|e| e.to_string())
}

fn collect_context(
    lines: &[&str],
    match_index: usize,
    amount: usize,
    before: bool,
) -> Vec<SearchContextLine> {
    if amount == 0 {
        return Vec::new();
    }

    if before {
        let start = match_index.saturating_sub(amount);
        (start..match_index)
            .map(|idx| SearchContextLine {
                line_number: idx + 1,
                line: lines[idx].to_string(),
            })
            .collect()
    } else {
        let end = (match_index + 1 + amount).min(lines.len());
        ((match_index + 1)..end)
            .map(|idx| SearchContextLine {
                line_number: idx + 1,
                line: lines[idx].to_string(),
            })
            .collect()
    }
}

fn modified_at(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn normalize_limit(requested: usize, default: usize) -> usize {
    let limit = if requested == 0 { default } else { requested };
    limit.min(10_000)
}

fn is_binary_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some(
            "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "webp"
                | "ico"
                | "pdf"
                | "zip"
                | "gz"
                | "wasm"
                | "o"
                | "so"
                | "dylib"
                | "exe"
                | "lock"
        )
    )
}
