use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::cache::IndexCache;
use crate::file_scanner::{ScanStats, ScannedFile, scan_files_with_policy};
use crate::git_changed::git_changed_files;
use crate::ranker::{RankHit, build_import_map, rank_symbols};
use crate::summarizer::{HeuristicSummarizer, Summarizer, summary_cache_key};
use crate::symbol_index::extract_symbols_and_imports;

/// Stable identifier for a node in the repo tree. Phase 1 derives ids from the relative path
/// (the workspace root is `""`), so they are deterministic and rebuildable.
pub type NodeId = String;

/// Kind of a node in the code-native repo tree. Directory/File are populated in Phase 1; the
/// symbol-level kinds are reserved for the tree-sitter pass in Phase 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepoNodeKind {
    Workspace,
    Package,
    Directory,
    File,
    Module,
    Class,
    Struct,
    Enum,
    Function,
    Method,
    Test,
    Config,
    Documentation,
}

/// A reference to a code symbol. Populated by the Phase 2 tree-sitter pass; the type exists now so
/// the data model and storage are stable.
#[derive(Clone, Debug)]
pub struct SymbolRef {
    pub id: String,
    pub path: String,
    pub name: String,
    pub kind: String,
    pub start_line: u32,
    pub end_line: u32,
    pub signature: Option<String>,
    pub summary: Option<String>,
}

/// A node in the repo tree (workspace, directory, file, or — later — a symbol).
#[derive(Clone, Debug)]
pub struct RepoNode {
    pub id: NodeId,
    pub kind: RepoNodeKind,
    pub path: String,
    pub name: String,
    pub language: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub summary: Option<String>,
    /// Parent node id; `None` for the workspace root. Used for cache persistence.
    pub parent_id: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub symbols: Vec<SymbolRef>,
    pub content_hash: Option<String>,
}

/// Budget controlling how much of the tree [`RepoIndex::compact_map`] emits.
#[derive(Clone, Debug)]
pub struct MapBudget {
    /// Maximum directory nesting depth rendered (root counts as depth 0).
    pub max_depth: usize,
    /// Maximum number of rendered entries (directories + files).
    pub max_entries: usize,
    /// Optional relative path prefix to restrict the rendered subtree.
    pub focus: Option<String>,
}

impl MapBudget {
    /// Default compact budget targeting ~500-1500 tokens (per the context contract).
    pub fn compact() -> Self {
        Self {
            max_depth: 4,
            max_entries: 400,
            focus: None,
        }
    }

    pub fn with_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    pub fn with_focus(mut self, focus: Option<String>) -> Self {
        self.focus = focus.filter(|f| !f.is_empty());
        self
    }
}

impl Default for MapBudget {
    fn default() -> Self {
        Self::compact()
    }
}

/// Summary of what a [`RepoIndex::refresh`] changed in the cache.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RefreshStats {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum IndexPhase {
    #[default]
    Unindexed,
    Queued,
    Scanning,
    Parsing,
    Summarizing,
    Ready,
    Stale,
    Failed,
}

impl IndexPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unindexed => "unindexed",
            Self::Queued => "queued",
            Self::Scanning => "scanning",
            Self::Parsing => "parsing",
            Self::Summarizing => "summarizing",
            Self::Ready => "ready",
            Self::Stale => "stale",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(raw: &str) -> Self {
        match raw {
            "queued" => Self::Queued,
            "scanning" => Self::Scanning,
            "parsing" => Self::Parsing,
            "summarizing" => Self::Summarizing,
            "ready" => Self::Ready,
            "stale" => Self::Stale,
            "failed" => Self::Failed,
            _ => Self::Unindexed,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexSnapshot {
    pub phase: IndexPhase,
    pub last_indexed_unix_secs: Option<i64>,
    pub last_error: Option<String>,
    pub stale: bool,
    pub active_ignore_sources: Vec<String>,
    pub stats: ScanStats,
    pub symbols_indexed: usize,
    pub summaries_cached: usize,
}

/// Code-native repo index backed by a content-hash-keyed SQLite cache.
pub struct RepoIndex {
    root: PathBuf,
    cache: IndexCache,
    files: Vec<ScannedFile>,
    respect_git_ignore: bool,
    changed_files: HashSet<String>,
    summarizer: HeuristicSummarizer,
}

impl RepoIndex {
    /// Build (or refresh) the index for `root`, persisting to the SQLite cache at `db_path`.
    pub fn build(root: impl Into<PathBuf>, db_path: impl AsRef<Path>) -> Result<Self, String> {
        let cache = IndexCache::open(db_path.as_ref())?;
        Self::with_cache(root, cache)
    }

    /// Build the index using an in-memory cache (no sidecar file). Useful for one-shot map
    /// generation and tests.
    pub fn build_in_memory(root: impl Into<PathBuf>) -> Result<Self, String> {
        let cache = IndexCache::open_in_memory()?;
        Self::with_cache(root, cache)
    }

    fn with_cache(root: impl Into<PathBuf>, cache: IndexCache) -> Result<Self, String> {
        let root = root.into();
        let changed: HashSet<String> = git_changed_files(&root)
            .into_iter()
            .map(|p| path_key(&p))
            .collect();
        let mut index = Self {
            root,
            cache,
            files: Vec::new(),
            respect_git_ignore: true,
            changed_files: changed,
            summarizer: HeuristicSummarizer::new(),
        };
        index.refresh()?;
        Ok(index)
    }

    /// Override the set of recently changed files used for ranking (e.g. from an external git helper).
    pub fn with_changed_files(mut self, files: impl IntoIterator<Item = PathBuf>) -> Self {
        self.changed_files = files.into_iter().map(|p| path_key(&p)).collect();
        self
    }

    /// Rescan the workspace and update the cache incrementally by content hash. Returns stats on
    /// what changed.
    pub fn refresh(&mut self) -> Result<RefreshStats, String> {
        self.cache.update_index_phase(IndexPhase::Scanning)?;
        let outcome = scan_files_with_policy(&self.root, self.respect_git_ignore)?;
        self.cache.update_index_snapshot_fields(
            outcome.active_ignore_sources.clone(),
            outcome.stats.clone(),
            None,
            false,
        )?;
        let scanned = outcome.files;
        let cached = self.cache.load_files()?;
        let mut stats = RefreshStats::default();
        let mut symbol_pass_paths: Vec<String> = Vec::new();

        let mut seen = std::collections::HashSet::new();
        for file in &scanned {
            let key = path_key(&file.path);
            seen.insert(key.clone());
            match cached.get(&key) {
                Some(prev) if prev.content_hash == file.content_hash => {
                    stats.unchanged += 1;
                }
                Some(_) => {
                    self.cache.upsert_file(&key, file)?;
                    stats.updated += 1;
                    symbol_pass_paths.push(key.clone());
                }
                None => {
                    self.cache.upsert_file(&key, file)?;
                    stats.added += 1;
                    symbol_pass_paths.push(key.clone());
                }
            }
        }
        for path in cached.keys() {
            if !seen.contains(path) {
                self.cache.delete_file(path)?;
                stats.removed += 1;
            }
        }

        if self.changed_files.is_empty() {
            self.changed_files = git_changed_files(&self.root)
                .into_iter()
                .map(|p| path_key(&p))
                .collect();
        }

        self.files = scanned.clone();
        let nodes = self.build_nodes();
        self.cache.replace_context_nodes(&nodes)?;
        self.cache.update_index_phase(IndexPhase::Parsing)?;
        for rel in &symbol_pass_paths {
            if let Some(file) = scanned.iter().find(|f| path_key(&f.path) == *rel) {
                self.index_file_symbols(rel, file)?;
            }
        }

        self.cache.update_index_phase(IndexPhase::Summarizing)?;
        self.files = scanned;
        let mut nodes = self.build_nodes();
        self.refresh_directory_summaries(&mut nodes, &symbol_pass_paths)?;
        self.cache.replace_context_nodes(&nodes)?;
        self.cache.update_index_snapshot_fields(
            outcome.active_ignore_sources,
            outcome.stats,
            None,
            false,
        )?;
        self.cache.mark_index_ready()?;
        Ok(stats)
    }

    fn index_file_symbols(&self, rel: &str, file: &ScannedFile) -> Result<(), String> {
        let abs = self.root.join(rel);
        let source = std::fs::read_to_string(&abs).unwrap_or_default();
        let language = file.language.as_deref();
        let (symbols, imports) = extract_symbols_and_imports(language, &source);
        self.cache.replace_symbols_for_file(rel, &symbols)?;
        self.cache.replace_imports_for_file(rel, &imports)?;

        let summary = self
            .summarizer
            .summarize_file(rel, language, &source, &symbols);
        let key = summary_cache_key(
            rel,
            &file.content_hash,
            crate::summarizer::SUMMARIZER_PROMPT_VERSION,
        );
        self.cache.upsert_summary(&key, rel, &summary)?;
        Ok(())
    }

    fn refresh_directory_summaries(
        &self,
        nodes: &mut [RepoNode],
        changed_paths: &[String],
    ) -> Result<(), String> {
        let mut dirs_to_update: HashSet<String> = HashSet::new();
        for path in changed_paths {
            let mut current = parent_dir(path).map(|s| s.to_string());
            while let Some(dir) = current {
                dirs_to_update.insert(dir.clone());
                current = parent_dir(&dir).map(|s| s.to_string());
            }
        }
        let file_summaries = self.file_summaries_map()?;
        let node_snapshot: Vec<RepoNode> = nodes.to_vec();
        for node in nodes.iter_mut() {
            if node.kind != RepoNodeKind::File {
                if dirs_to_update.contains(&node.path) || node.path.is_empty() {
                    let child_entries: Vec<(&str, &str)> = node
                        .children
                        .iter()
                        .filter_map(|child_id| {
                            node_snapshot.iter().find(|n| &n.id == child_id).map(|n| {
                                let summary = file_summaries
                                    .get(&n.path)
                                    .map(String::as_str)
                                    .or(n.summary.as_deref())
                                    .unwrap_or("");
                                (n.name.as_str(), summary)
                            })
                        })
                        .collect();
                    let summary = self
                        .summarizer
                        .summarize_directory(&node.path, &child_entries);
                    node.summary = Some(summary);
                }
                continue;
            }
            if let Some(summary) = file_summaries.get(&node.path) {
                node.summary = Some(summary.clone());
            }
        }
        Ok(())
    }

    fn file_summaries_map(&self) -> Result<BTreeMap<String, String>, String> {
        let paths: Vec<String> = self.files.iter().map(|f| path_key(&f.path)).collect();
        Ok(self
            .cache
            .load_summaries_by_paths(&paths)?
            .into_iter()
            .collect())
    }

    /// The workspace root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Number of files currently indexed.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn index_snapshot(&self) -> Result<IndexSnapshot, String> {
        self.cache.load_index_snapshot()
    }

    pub fn mark_stale(&self) -> Result<(), String> {
        self.cache.mark_index_stale()
    }

    /// Render the compact, model-facing `<repo_index>` text block bounded by `budget`.
    pub fn compact_map(&self, budget: MapBudget) -> String {
        let root_name = self
            .root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace");

        let tree = self.build_dir_tree();
        let mut out = String::new();
        out.push_str(&format!(
            "<repo_index root=\"{root_name}\" budget=\"compact\">\n"
        ));

        let focus = budget.focus.as_deref().map(normalize_rel);
        let summaries = self.file_summaries_map().unwrap_or_default();
        let mut emitted = 0usize;
        let mut truncated = false;
        render_dir(
            &tree,
            0,
            "",
            focus.as_deref(),
            &budget,
            &summaries,
            &mut emitted,
            &mut truncated,
            &mut out,
        );
        if emitted == 0 {
            out.push_str("(no files)\n");
        }
        if truncated {
            out.push_str(&format!(
                "… (truncated at {} entries; call repo_map with a focus path for more)\n",
                budget.max_entries
            ));
        }
        out.push_str("</repo_index>\n");
        out
    }

    /// Build the flat list of `context_nodes` (workspace + directories + files) from the current
    /// scanned file set.
    fn build_nodes(&self) -> Vec<RepoNode> {
        let mut nodes: BTreeMap<String, RepoNode> = BTreeMap::new();
        // Workspace root.
        let root_name = self
            .root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace")
            .to_string();
        nodes.insert(
            String::new(),
            RepoNode {
                id: String::new(),
                kind: RepoNodeKind::Workspace,
                path: String::new(),
                name: root_name,
                language: None,
                start_line: 0,
                end_line: 0,
                summary: None,
                parent_id: None,
                children: Vec::new(),
                symbols: Vec::new(),
                content_hash: None,
            },
        );

        for file in &self.files {
            let rel = path_key(&file.path);
            // Ensure all ancestor directories exist as nodes.
            let mut parent = String::new();
            if let Some(dir) = parent_dir(&rel) {
                let mut accum = String::new();
                for component in dir.split('/').filter(|c| !c.is_empty()) {
                    let parent_id = accum.clone();
                    if accum.is_empty() {
                        accum = component.to_string();
                    } else {
                        accum = format!("{accum}/{component}");
                    }
                    nodes.entry(accum.clone()).or_insert_with(|| RepoNode {
                        id: accum.clone(),
                        kind: RepoNodeKind::Directory,
                        path: accum.clone(),
                        name: component.to_string(),
                        language: None,
                        start_line: 0,
                        end_line: 0,
                        summary: None,
                        parent_id: Some(parent_id.clone()),
                        children: Vec::new(),
                        symbols: Vec::new(),
                        content_hash: None,
                    });
                }
                parent = dir.to_string();
            }
            let name = file_name(&rel).to_string();
            nodes.insert(
                rel.clone(),
                RepoNode {
                    id: rel.clone(),
                    kind: RepoNodeKind::File,
                    path: rel.clone(),
                    name,
                    language: file.language.clone(),
                    start_line: 0,
                    end_line: 0,
                    summary: None,
                    parent_id: Some(parent),
                    children: Vec::new(),
                    symbols: Vec::new(),
                    content_hash: Some(file.content_hash.clone()),
                },
            );
        }

        // Link children to parents.
        let ids: Vec<String> = nodes.keys().cloned().collect();
        for id in ids {
            let parent_id = nodes.get(&id).and_then(|n| n.parent_id.clone());
            if let Some(parent_id) = parent_id {
                if let Some(parent) = nodes.get_mut(&parent_id) {
                    parent.children.push(id.clone());
                }
            }
        }

        nodes.into_values().collect()
    }

    /// Start a recursive filesystem watcher on the workspace root. The returned watcher must be
    /// kept alive; raw events are delivered on the channel so a caller can debounce and call
    /// [`RepoIndex::refresh`]. (Wiring this into the runtime is a later phase.)
    pub fn watch(
        &self,
    ) -> Result<
        (
            notify::RecommendedWatcher,
            std::sync::mpsc::Receiver<notify::Result<notify::Event>>,
        ),
        String,
    > {
        use notify::{RecursiveMode, Watcher};
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })
        .map_err(|e| e.to_string())?;
        watcher
            .watch(&self.root, RecursiveMode::Recursive)
            .map_err(|e| e.to_string())?;
        Ok((watcher, rx))
    }

    /// Rank symbols and files for `query`, returning the top `limit` hits.
    pub fn rank(&self, query: &str, limit: usize) -> Vec<RankHit> {
        let symbols = self.cache.load_all_symbols().unwrap_or_default();
        let imports = self.cache.load_all_imports().unwrap_or_default();
        let import_map = build_import_map(&imports);
        rank_symbols(
            query,
            &symbols,
            &self.files,
            &import_map,
            &self.changed_files,
            &self.changed_files,
            limit,
        )
    }

    /// Find symbols matching `name`, optionally filtered by `kind`. Returns compact `<symbol_result>` text.
    pub fn find_symbol(&self, name: &str, kind: Option<&str>, limit: usize) -> String {
        let mut hits = self.rank(name, limit.max(10));
        if let Some(k) = kind {
            let k = k.to_lowercase();
            hits.retain(|h| h.kind.to_lowercase() == k);
        }
        hits.truncate(limit);
        format_symbol_result(name, kind, &hits)
    }

    /// Open a node by id (symbol id or file/directory path) and return a `<file_slice>` block.
    pub fn open_node(&self, node_id: &str) -> Result<String, String> {
        if let Some(sym) = self.cache.load_symbol(node_id)? {
            return read_file_slice(&self.root, &sym.path, sym.start_line, sym.end_line);
        }
        if let Some(node) = self.cache.load_context_node(node_id)? {
            if node.kind == "file" {
                let end = if node.end_line > 0 {
                    node.end_line
                } else {
                    line_count(&self.root.join(&node.path))
                };
                return read_file_slice(&self.root, &node.path, 1, end);
            }
            return Err(format!(
                "node {node_id} is a {} — use find_symbol or repo_map for directories",
                node.kind
            ));
        }
        // Allow opening by bare relative path.
        let path = node_id.replace('\\', "/");
        if self.root.join(&path).is_file() {
            let end = line_count(&self.root.join(&path));
            return read_file_slice(&self.root, &path, 1, end);
        }
        Err(format!("unknown node id: {node_id}"))
    }

    /// Return a short summary for a symbol id, file path, or context node id (cached when available).
    pub fn summarize_node(&self, node_id: &str) -> Result<String, String> {
        if let Some(sym) = self.cache.load_symbol(node_id)? {
            if let Some(s) = sym.summary.as_ref().filter(|s| !s.is_empty()) {
                return Ok(s.clone());
            }
            if let Some(sig) = sym.signature.as_ref().filter(|s| !s.is_empty()) {
                return Ok(format!("{} {} — {sig}", sym.kind, sym.name));
            }
            return Ok(format!("{} {} in {}", sym.kind, sym.name, sym.path));
        }
        if let Some(node) = self.cache.load_context_node(node_id)? {
            if let Some(s) = node.summary.as_ref().filter(|s| !s.is_empty()) {
                return Ok(s.clone());
            }
            let paths = vec![node.path.clone()];
            if let Ok(map) = self.cache.load_summaries_by_paths(&paths) {
                if let Some(s) = map.get(&node.path) {
                    return Ok(s.clone());
                }
            }
            return Ok(format!("{} {}", node.kind, node.path));
        }
        let path = node_id.replace('\\', "/");
        if self.root.join(&path).is_file() {
            let paths = vec![path.clone()];
            if let Ok(map) = self.cache.load_summaries_by_paths(&paths) {
                if let Some(s) = map.get(&path) {
                    return Ok(s.clone());
                }
            }
            return Ok(format!("file {path}"));
        }
        Err(format!("unknown node id: {node_id}"))
    }

    /// List files related to `path` via imports (compact text block).
    pub fn related_files(&self, path: &str) -> String {
        let rel = normalize_rel(path);
        let related = self.cache.related_paths(&rel).unwrap_or_default();
        let mut out = String::new();
        out.push_str(&format!("<related_files path=\"{rel}\">\n"));
        if related.is_empty() {
            out.push_str("(no related files found via imports)\n");
        } else {
            for p in related {
                out.push_str(&format!("{p}\n"));
            }
        }
        out.push_str("</related_files>\n");
        out
    }

    /// Build an in-memory directory tree (for rendering) from the scanned file list.
    fn build_dir_tree(&self) -> DirTree {
        let mut root = DirTree::default();
        for file in &self.files {
            let rel = path_key(&file.path);
            let components: Vec<&str> = rel.split('/').filter(|c| !c.is_empty()).collect();
            if components.is_empty() {
                continue;
            }
            let mut node = &mut root;
            for component in &components[..components.len() - 1] {
                node = node.dirs.entry((*component).to_string()).or_default();
            }
            let file_name = components[components.len() - 1].to_string();
            node.files.push((file_name, file.language.clone()));
        }
        root
    }
}

pub fn load_index_snapshot(db_path: impl AsRef<Path>) -> Result<IndexSnapshot, String> {
    let cache = IndexCache::open(db_path.as_ref())?;
    cache.load_index_snapshot()
}

pub fn mark_index_phase(db_path: impl AsRef<Path>, phase: IndexPhase) -> Result<(), String> {
    let cache = IndexCache::open(db_path.as_ref())?;
    cache.update_index_phase(phase)
}

pub fn mark_index_stale(db_path: impl AsRef<Path>) -> Result<(), String> {
    let cache = IndexCache::open(db_path.as_ref())?;
    cache.mark_index_stale()
}

pub fn mark_index_failed(db_path: impl AsRef<Path>, error: &str) -> Result<(), String> {
    let cache = IndexCache::open(db_path.as_ref())?;
    cache.mark_index_failed(error)
}

/// Nested directory representation used only for rendering the compact map.
#[derive(Default)]
struct DirTree {
    dirs: BTreeMap<String, DirTree>,
    files: Vec<(String, Option<String>)>,
}

#[allow(clippy::too_many_arguments)]
fn render_dir(
    node: &DirTree,
    depth: usize,
    prefix: &str,
    focus: Option<&str>,
    budget: &MapBudget,
    summaries: &BTreeMap<String, String>,
    emitted: &mut usize,
    truncated: &mut bool,
    out: &mut String,
) {
    if *truncated {
        return;
    }
    let indent = "  ".repeat(depth);

    for (name, child) in &node.dirs {
        let child_path = join_rel(prefix, name);
        if !focus_allows(focus, &child_path) {
            continue;
        }
        if *emitted >= budget.max_entries {
            *truncated = true;
            return;
        }
        out.push_str(&format!("{indent}{name}/\n"));
        *emitted += 1;
        if depth + 1 <= budget.max_depth {
            render_dir(
                child,
                depth + 1,
                &child_path,
                focus,
                budget,
                summaries,
                emitted,
                truncated,
                out,
            );
            if *truncated {
                return;
            }
        }
    }

    let file_indent = "  ".repeat(depth + 1);
    let mut files = node.files.clone();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, language) in files {
        let file_path = join_rel(prefix, &name);
        if !focus_allows(focus, &file_path) {
            continue;
        }
        if *emitted >= budget.max_entries {
            *truncated = true;
            return;
        }
        if let Some(summary) = summaries.get(&file_path) {
            out.push_str(&format!("{file_indent}{name}  - {summary}\n"));
        } else {
            match language {
                Some(lang) => out.push_str(&format!("{file_indent}{name}  - {lang}\n")),
                None => out.push_str(&format!("{file_indent}{name}\n")),
            }
        }
        *emitted += 1;
    }
}

fn format_symbol_result(query: &str, kind: Option<&str>, hits: &[RankHit]) -> String {
    let kind_attr = kind.map(|k| format!(" kind=\"{k}\"")).unwrap_or_default();
    let mut out = String::new();
    out.push_str(&format!("<symbol_result query=\"{query}\"{kind_attr}>\n"));
    if hits.is_empty() {
        out.push_str("(no matches)\n");
    } else {
        for (i, hit) in hits.iter().enumerate() {
            let sig = hit
                .signature
                .as_deref()
                .unwrap_or(&hit.kind)
                .chars()
                .take(80)
                .collect::<String>();
            out.push_str(&format!(
                "{}. {}:{}-{}  {}  {}  id={}\n",
                i + 1,
                hit.path,
                hit.start_line,
                hit.end_line,
                hit.kind,
                sig,
                hit.symbol_id,
            ));
        }
    }
    out.push_str("</symbol_result>\n");
    out
}

fn read_file_slice(
    root: &Path,
    rel: &str,
    start_line: u32,
    end_line: u32,
) -> Result<String, String> {
    let abs = root.join(rel);
    let content = std::fs::read_to_string(&abs).map_err(|e| e.to_string())?;
    let lines: Vec<&str> = content.lines().collect();
    let start = start_line.max(1) as usize;
    let end = end_line.max(start_line) as usize;
    let slice = if start > lines.len() {
        String::new()
    } else {
        lines[(start - 1)..lines.len().min(end)].join("\n")
    };
    Ok(format!(
        "<file_slice path=\"{rel}\" lines=\"{start_line}-{end_line}\">\n{slice}\n</file_slice>\n"
    ))
}

fn line_count(path: &Path) -> u32 {
    std::fs::read_to_string(path)
        .map(|s| s.lines().count().max(1) as u32)
        .unwrap_or(1)
}

/// A path passes the focus filter when either no focus is set, or the focus is a prefix of the
/// path, or the path is a prefix of the focus (so we still render the parent chain down to it).
fn focus_allows(focus: Option<&str>, path: &str) -> bool {
    match focus {
        None => true,
        Some(focus) => {
            path == focus
                || path.starts_with(&format!("{focus}/"))
                || focus.starts_with(&format!("{path}/"))
        }
    }
}

fn join_rel(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}/{name}")
    }
}

/// Normalize a relative path to forward slashes with no leading/trailing slash.
fn normalize_rel(path: &str) -> String {
    path.replace('\\', "/").trim_matches('/').to_string()
}

/// Stable, platform-independent key for a relative path (forward slashes).
fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn parent_dir(rel: &str) -> Option<&str> {
    rel.rfind('/').map(|idx| &rel[..idx])
}

fn file_name(rel: &str) -> &str {
    rel.rfind('/').map(|idx| &rel[idx + 1..]).unwrap_or(rel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(root: &Path, rel: &str, contents: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn build_scans_and_maps_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "src/main.rs", "fn main() {}\n");
        write(root, "src/util/helper.rs", "pub fn h() {}\n");
        write(root, "README.md", "# hi\n");

        let index = RepoIndex::build_in_memory(root).unwrap();
        assert_eq!(index.file_count(), 3);

        let map = index.compact_map(MapBudget::compact());
        assert!(map.starts_with("<repo_index"));
        assert!(map.contains("src/"));
        assert!(
            map.contains("main.rs  - rust") || map.contains("main.rs  - main"),
            "expected summary or language tag for main.rs: {map}"
        );
        assert!(map.contains("README.md  - markdown"));
        assert!(map.ends_with("</repo_index>\n"));
    }

    #[test]
    fn refresh_is_incremental_by_hash() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "a.rs", "fn a() {}\n");
        let mut index = RepoIndex::build_in_memory(root).unwrap();

        // No changes -> everything unchanged.
        let stats = index.refresh().unwrap();
        assert_eq!(stats.added, 0);
        assert_eq!(stats.updated, 0);
        assert_eq!(stats.unchanged, 1);

        // Modify -> updated.
        write(root, "a.rs", "fn a() { let x = 1; }\n");
        let stats = index.refresh().unwrap();
        assert_eq!(stats.updated, 1);

        // Add a new file -> added.
        write(root, "b.rs", "fn b() {}\n");
        let stats = index.refresh().unwrap();
        assert_eq!(stats.added, 1);

        // Remove -> removed.
        fs::remove_file(root.join("b.rs")).unwrap();
        let stats = index.refresh().unwrap();
        assert_eq!(stats.removed, 1);
    }

    #[test]
    fn compact_map_respects_entry_budget() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for i in 0..50 {
            write(root, &format!("f{i}.rs"), "fn x() {}\n");
        }
        let index = RepoIndex::build_in_memory(root).unwrap();
        let budget = MapBudget {
            max_depth: 2,
            max_entries: 10,
            focus: None,
        };
        let map = index.compact_map(budget);
        assert!(map.contains("truncated at 10 entries"));
    }

    #[test]
    fn focus_restricts_subtree() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "src/a.rs", "fn a() {}\n");
        write(root, "docs/b.md", "doc\n");
        let index = RepoIndex::build_in_memory(root).unwrap();
        let map = index.compact_map(MapBudget::compact().with_focus(Some("src".to_string())));
        assert!(map.contains("a.rs"));
        assert!(!map.contains("b.md"));
    }

    #[test]
    fn find_symbol_and_open_node_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "lib.rs",
            "pub struct DeltaCoalescer;\n\nimpl DeltaCoalescer {\n    pub fn batch() {}\n}\n",
        );
        let index = RepoIndex::build_in_memory(root).unwrap();
        let result = index.find_symbol("DeltaCoalescer", None, 5);
        assert!(result.contains("<symbol_result"));
        assert!(result.contains("DeltaCoalescer"));
        assert!(result.contains("id="));
        let hits = index.rank("DeltaCoalescer", 1);
        assert!(!hits.is_empty());
        let slice = index.open_node(&hits[0].symbol_id).unwrap();
        assert!(slice.contains("<file_slice"));
        assert!(slice.contains("DeltaCoalescer"));
    }

    #[test]
    fn build_persists_index_snapshot_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let db_dir = tempfile::tempdir().unwrap();
        let db_path = db_dir.path().join("index.db");

        write(root, ".gitignore", "ignored.rs\n");
        write(root, "src/main.rs", "fn main() {}\n");
        write(root, "ignored.rs", "fn ignored() {}\n");

        let index = RepoIndex::build(root, &db_path).unwrap();
        let snapshot = index.index_snapshot().unwrap();

        assert_eq!(snapshot.phase, IndexPhase::Ready);
        assert!(!snapshot.stale);
        assert_eq!(snapshot.stats.files_indexed, 1);
        assert_eq!(snapshot.stats.skipped_ignore, 1);
        assert!(
            snapshot
                .active_ignore_sources
                .iter()
                .any(|s| s == ".gitignore")
        );
    }

    #[test]
    fn stale_marker_updates_snapshot_without_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let db_dir = tempfile::tempdir().unwrap();
        let db_path = db_dir.path().join("index.db");

        write(root, "src/main.rs", "fn main() {}\n");
        let _index = RepoIndex::build(root, &db_path).unwrap();
        mark_index_stale(&db_path).unwrap();

        let snapshot = load_index_snapshot(&db_path).unwrap();
        assert_eq!(snapshot.phase, IndexPhase::Stale);
        assert!(snapshot.stale);
        assert_eq!(snapshot.stats.files_indexed, 1);
    }
}
