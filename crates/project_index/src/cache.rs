use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::file_scanner::{ScanStats, ScannedFile};
use crate::repo_index::{IndexPhase, IndexSnapshot, RepoNode, RepoNodeKind};
use crate::summarizer::SUMMARIZER_PROMPT_VERSION;
use crate::symbol_index::{ExtractedImport, ExtractedSymbol, symbol_id};

/// A symbol row as stored in the SQLite cache.
#[derive(Clone, Debug)]
pub struct CachedSymbol {
    pub id: String,
    pub path: String,
    pub name: String,
    pub kind: String,
    pub start_line: u32,
    pub end_line: u32,
    pub signature: Option<String>,
    pub summary: Option<String>,
}

/// An import row as stored in the SQLite cache.
#[derive(Clone, Debug)]
pub struct CachedImport {
    pub path: String,
    pub target: String,
    pub kind: Option<String>,
}

/// A context node row loaded from SQLite.
#[derive(Clone, Debug)]
pub struct CachedContextNode {
    pub id: String,
    pub kind: String,
    pub path: String,
    pub name: String,
    pub language: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub parent_id: Option<String>,
    pub content_hash: Option<String>,
    pub summary: Option<String>,
}

/// A file row as stored in the SQLite cache.
#[derive(Clone, Debug)]
pub struct CachedFile {
    pub path: String,
    pub language: Option<String>,
    pub size: u64,
    pub mtime: i64,
    pub content_hash: String,
}

/// SQLite-backed, content-hash-keyed cache for the repo index.
///
/// The database is a rebuildable sidecar (separate from the event-sourced `vortex.db`). Tables for
/// `symbols`, `imports`, and `summaries` are created up-front so later phases can populate them;
/// Phase 1 only writes `files` and `context_nodes`.
pub struct IndexCache {
    conn: Connection,
}

impl IndexCache {
    /// Open (creating if needed) the cache database at `db_path`, ensuring parent directories and
    /// the schema exist.
    pub fn open(db_path: &Path) -> Result<Self, String> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
        let cache = Self { conn };
        cache.create_tables()?;
        Ok(cache)
    }

    /// Open an in-memory cache (used for tests and ephemeral builds).
    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
        let cache = Self { conn };
        cache.create_tables()?;
        Ok(cache)
    }

    fn create_tables(&self) -> Result<(), String> {
        self.conn.execute_batch(SCHEMA).map_err(|e| e.to_string())
    }

    /// Current `files` rows keyed by path.
    pub fn load_files(&self) -> Result<HashMap<String, CachedFile>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, language, size, mtime, content_hash FROM files")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(CachedFile {
                    path: row.get(0)?,
                    language: row.get(1)?,
                    size: row.get::<_, i64>(2)? as u64,
                    mtime: row.get(3)?,
                    content_hash: row.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut out = HashMap::new();
        for row in rows {
            let row = row.map_err(|e| e.to_string())?;
            out.insert(row.path.clone(), row);
        }
        Ok(out)
    }

    /// Insert or update a single file row (keyed by path).
    pub fn upsert_file(&self, path: &str, file: &ScannedFile) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO files (path, language, size, mtime, content_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(path) DO UPDATE SET
                   language = excluded.language,
                   size = excluded.size,
                   mtime = excluded.mtime,
                   content_hash = excluded.content_hash",
                rusqlite::params![
                    path,
                    file.language,
                    file.size as i64,
                    file.mtime,
                    file.content_hash,
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Delete a file row (and any symbols/imports/summaries attached to it).
    pub fn delete_file(&self, path: &str) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM files WHERE path = ?1", [path])
            .map_err(|e| e.to_string())?;
        self.conn
            .execute("DELETE FROM symbols WHERE path = ?1", [path])
            .map_err(|e| e.to_string())?;
        self.conn
            .execute("DELETE FROM imports WHERE path = ?1", [path])
            .map_err(|e| e.to_string())?;
        self.conn
            .execute("DELETE FROM summaries WHERE path = ?1", [path])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Replace symbols for a single file (deletes prior rows for that path first).
    pub fn replace_symbols_for_file(
        &self,
        path: &str,
        symbols: &[ExtractedSymbol],
    ) -> Result<Vec<CachedSymbol>, String> {
        self.conn
            .execute("DELETE FROM symbols WHERE path = ?1", [path])
            .map_err(|e| e.to_string())?;
        let mut cached = Vec::with_capacity(symbols.len());
        for sym in symbols {
            let id = symbol_id(path, &sym.kind, &sym.name, sym.start_line);
            self.conn
                .execute(
                    "INSERT INTO symbols (id, path, name, kind, start_line, end_line, signature, summary)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    rusqlite::params![
                        id,
                        path,
                        sym.name,
                        sym.kind,
                        sym.start_line as i64,
                        sym.end_line as i64,
                        sym.signature,
                        None::<String>,
                    ],
                )
                .map_err(|e| e.to_string())?;
            cached.push(CachedSymbol {
                id,
                path: path.to_string(),
                name: sym.name.clone(),
                kind: sym.kind.clone(),
                start_line: sym.start_line,
                end_line: sym.end_line,
                signature: sym.signature.clone(),
                summary: None,
            });
        }
        Ok(cached)
    }

    /// Replace imports for a single file.
    pub fn replace_imports_for_file(
        &self,
        path: &str,
        imports: &[ExtractedImport],
    ) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM imports WHERE path = ?1", [path])
            .map_err(|e| e.to_string())?;
        for imp in imports {
            self.conn
                .execute(
                    "INSERT INTO imports (path, target, kind) VALUES (?1, ?2, ?3)",
                    rusqlite::params![path, imp.target, imp.kind],
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Load all symbols (for ranking and search).
    pub fn load_all_symbols(&self) -> Result<Vec<CachedSymbol>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, path, name, kind, start_line, end_line, signature, summary FROM symbols",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(CachedSymbol {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    name: row.get(2)?,
                    kind: row.get(3)?,
                    start_line: row.get::<_, i64>(4)? as u32,
                    end_line: row.get::<_, i64>(5)? as u32,
                    signature: row.get(6)?,
                    summary: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    /// Load a symbol by id.
    pub fn load_symbol(&self, id: &str) -> Result<Option<CachedSymbol>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, path, name, kind, start_line, end_line, signature, summary
                 FROM symbols WHERE id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query([id]).map_err(|e| e.to_string())?;
        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            Ok(Some(CachedSymbol {
                id: row.get(0).map_err(|e| e.to_string())?,
                path: row.get(1).map_err(|e| e.to_string())?,
                name: row.get(2).map_err(|e| e.to_string())?,
                kind: row.get(3).map_err(|e| e.to_string())?,
                start_line: row.get::<_, i64>(4).map_err(|e| e.to_string())? as u32,
                end_line: row.get::<_, i64>(5).map_err(|e| e.to_string())? as u32,
                signature: row.get(6).map_err(|e| e.to_string())?,
                summary: row.get(7).map_err(|e| e.to_string())?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Load all imports as (path, target) pairs.
    pub fn load_all_imports(&self) -> Result<Vec<(String, String)>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, target FROM imports")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    /// Imports for a single file.
    pub fn load_imports_for_path(&self, path: &str) -> Result<Vec<CachedImport>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, target, kind FROM imports WHERE path = ?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([path], |row| {
                Ok(CachedImport {
                    path: row.get(0)?,
                    target: row.get(1)?,
                    kind: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    /// Files that share import targets with `path` or import each other (heuristic related set).
    pub fn related_paths(&self, path: &str) -> Result<Vec<String>, String> {
        let imports = self.load_imports_for_path(path)?;
        let all = self.load_all_imports()?;
        let mut related = std::collections::HashSet::new();
        for imp in &imports {
            for (other_path, target) in &all {
                if other_path != path
                    && (target.contains(&imp.target) || imp.target.contains(target))
                {
                    related.insert(other_path.clone());
                }
            }
        }
        for (other_path, target) in &all {
            if other_path != path && target.contains(path) {
                related.insert(other_path.clone());
            }
        }
        let mut out: Vec<String> = related.into_iter().collect();
        out.sort();
        Ok(out)
    }

    /// Upsert a cached summary row.
    pub fn upsert_summary(&self, key: &str, path: &str, summary: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO summaries (key, path, summary, summarizer_prompt_version)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(key) DO UPDATE SET
                   summary = excluded.summary,
                   path = excluded.path,
                   summarizer_prompt_version = excluded.summarizer_prompt_version",
                rusqlite::params![key, path, summary, SUMMARIZER_PROMPT_VERSION],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Load summaries keyed by path (latest row per path).
    pub fn load_summaries_by_paths(
        &self,
        paths: &[String],
    ) -> Result<HashMap<String, String>, String> {
        let mut out = HashMap::new();
        for path in paths {
            let mut stmt = self
                .conn
                .prepare(
                    "SELECT summary FROM summaries WHERE path = ?1
                     ORDER BY rowid DESC LIMIT 1",
                )
                .map_err(|e| e.to_string())?;
            let mut rows = stmt.query([path]).map_err(|e| e.to_string())?;
            if let Some(row) = rows.next().map_err(|e| e.to_string())? {
                let summary: String = row.get(0).map_err(|e| e.to_string())?;
                out.insert(path.clone(), summary);
            }
        }
        Ok(out)
    }

    /// Load a context node by id (file path or directory path).
    pub fn load_context_node(&self, id: &str) -> Result<Option<CachedContextNode>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, kind, path, name, language, start_line, end_line, parent_id, content_hash, summary
                 FROM context_nodes WHERE id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query([id]).map_err(|e| e.to_string())?;
        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            Ok(Some(CachedContextNode {
                id: row.get(0).map_err(|e| e.to_string())?,
                kind: row.get(1).map_err(|e| e.to_string())?,
                path: row.get(2).map_err(|e| e.to_string())?,
                name: row.get(3).map_err(|e| e.to_string())?,
                language: row.get(4).map_err(|e| e.to_string())?,
                start_line: row.get::<_, i64>(5).map_err(|e| e.to_string())? as u32,
                end_line: row.get::<_, i64>(6).map_err(|e| e.to_string())? as u32,
                parent_id: row.get(7).map_err(|e| e.to_string())?,
                content_hash: row.get(8).map_err(|e| e.to_string())?,
                summary: row.get(9).map_err(|e| e.to_string())?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Update summary on a context node row.
    pub fn update_context_node_summary(&self, id: &str, summary: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE context_nodes SET summary = ?2 WHERE id = ?1",
                rusqlite::params![id, summary],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Replace the entire `context_nodes` table with `nodes`. The node tree is cheap to rebuild,
    /// so we regenerate it on each refresh rather than diffing.
    pub fn replace_context_nodes(&self, nodes: &[RepoNode]) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM context_nodes", [])
            .map_err(|e| e.to_string())?;
        let tx_sql =
            "INSERT INTO context_nodes (id, kind, path, name, language, start_line, end_line, parent_id, content_hash, summary)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)";
        for node in nodes {
            self.conn
                .execute(
                    tx_sql,
                    rusqlite::params![
                        node.id,
                        node.kind.as_str(),
                        node.path,
                        node.name,
                        node.language,
                        node.start_line as i64,
                        node.end_line as i64,
                        node.parent_id,
                        node.content_hash,
                        node.summary,
                    ],
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn update_index_phase(&self, phase: IndexPhase) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO index_metadata (
                    key, phase, stale, active_ignore_sources,
                    files_indexed, skipped_ignore, skipped_hidden, skipped_binary, skipped_large, skipped_policy,
                    symbols_indexed, summaries_cached
                 ) VALUES ('singleton', ?1, 0, '', 0, 0, 0, 0, 0, 0, 0, 0)
                 ON CONFLICT(key) DO UPDATE SET phase = excluded.phase",
                [phase.as_str()],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn update_index_snapshot_fields(
        &self,
        active_ignore_sources: Vec<String>,
        stats: ScanStats,
        last_error: Option<String>,
        stale: bool,
    ) -> Result<(), String> {
        let active_ignore_sources = active_ignore_sources.join("\n");
        self.conn
            .execute(
                "INSERT INTO index_metadata (
                    key, phase, stale, active_ignore_sources, last_error,
                    files_indexed, skipped_ignore, skipped_hidden, skipped_binary, skipped_large, skipped_policy,
                    symbols_indexed, summaries_cached
                ) VALUES ('singleton', 'scanning', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, 0)
                ON CONFLICT(key) DO UPDATE SET
                    stale = excluded.stale,
                    active_ignore_sources = excluded.active_ignore_sources,
                    last_error = excluded.last_error,
                    files_indexed = excluded.files_indexed,
                    skipped_ignore = excluded.skipped_ignore,
                    skipped_hidden = excluded.skipped_hidden,
                    skipped_binary = excluded.skipped_binary,
                    skipped_large = excluded.skipped_large,
                    skipped_policy = excluded.skipped_policy",
                rusqlite::params![
                    stale as i64,
                    active_ignore_sources,
                    last_error,
                    stats.files_indexed as i64,
                    stats.skipped_ignore as i64,
                    stats.skipped_hidden as i64,
                    stats.skipped_binary as i64,
                    stats.skipped_large as i64,
                    stats.skipped_policy as i64,
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn mark_index_ready(&self) -> Result<(), String> {
        let symbols = self.count_rows("symbols")?;
        let summaries = self.count_rows("summaries")?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.conn
            .execute(
                "INSERT INTO index_metadata (
                    key, phase, stale, last_indexed_unix_secs, symbols_indexed, summaries_cached
                ) VALUES ('singleton', 'ready', 0, ?1, ?2, ?3)
                ON CONFLICT(key) DO UPDATE SET
                    phase = 'ready',
                    stale = 0,
                    last_indexed_unix_secs = excluded.last_indexed_unix_secs,
                    last_error = NULL,
                    symbols_indexed = excluded.symbols_indexed,
                    summaries_cached = excluded.summaries_cached",
                rusqlite::params![now, symbols as i64, summaries as i64],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn mark_index_stale(&self) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO index_metadata (key, phase, stale) VALUES ('singleton', 'stale', 1)
                 ON CONFLICT(key) DO UPDATE SET phase = 'stale', stale = 1",
                [],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn mark_index_failed(&self, error: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO index_metadata (key, phase, stale, last_error) VALUES ('singleton', 'failed', 1, ?1)
                 ON CONFLICT(key) DO UPDATE SET phase = 'failed', stale = 1, last_error = excluded.last_error",
                [error],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn load_index_snapshot(&self) -> Result<IndexSnapshot, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT phase, last_indexed_unix_secs, last_error, stale, active_ignore_sources,
                        files_indexed, skipped_ignore, skipped_hidden, skipped_binary, skipped_large, skipped_policy,
                        symbols_indexed, summaries_cached
                 FROM index_metadata WHERE key = 'singleton'",
            )
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
        let Some(row) = rows.next().map_err(|e| e.to_string())? else {
            return Ok(IndexSnapshot::default());
        };
        let active_sources: String = row.get(4).map_err(|e| e.to_string())?;
        Ok(IndexSnapshot {
            phase: IndexPhase::from_str(&row.get::<_, String>(0).map_err(|e| e.to_string())?),
            last_indexed_unix_secs: row.get(1).map_err(|e| e.to_string())?,
            last_error: row.get(2).map_err(|e| e.to_string())?,
            stale: row.get::<_, i64>(3).map_err(|e| e.to_string())? != 0,
            active_ignore_sources: active_sources
                .lines()
                .map(|line| line.to_string())
                .filter(|line| !line.is_empty())
                .collect(),
            stats: ScanStats {
                files_indexed: row.get::<_, i64>(5).map_err(|e| e.to_string())? as usize,
                skipped_ignore: row.get::<_, i64>(6).map_err(|e| e.to_string())? as usize,
                skipped_hidden: row.get::<_, i64>(7).map_err(|e| e.to_string())? as usize,
                skipped_binary: row.get::<_, i64>(8).map_err(|e| e.to_string())? as usize,
                skipped_large: row.get::<_, i64>(9).map_err(|e| e.to_string())? as usize,
                skipped_policy: row.get::<_, i64>(10).map_err(|e| e.to_string())? as usize,
            },
            symbols_indexed: row.get::<_, i64>(11).map_err(|e| e.to_string())? as usize,
            summaries_cached: row.get::<_, i64>(12).map_err(|e| e.to_string())? as usize,
        })
    }

    fn count_rows(&self, table: &str) -> Result<usize, String> {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let count: i64 = self
            .conn
            .query_row(&sql, [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        Ok(count as usize)
    }
}

impl RepoNodeKind {
    /// Stable lowercase label used for the `context_nodes.kind` column and the model-facing map.
    pub fn as_str(&self) -> &'static str {
        match self {
            RepoNodeKind::Workspace => "workspace",
            RepoNodeKind::Package => "package",
            RepoNodeKind::Directory => "directory",
            RepoNodeKind::File => "file",
            RepoNodeKind::Module => "module",
            RepoNodeKind::Class => "class",
            RepoNodeKind::Struct => "struct",
            RepoNodeKind::Enum => "enum",
            RepoNodeKind::Function => "function",
            RepoNodeKind::Method => "method",
            RepoNodeKind::Test => "test",
            RepoNodeKind::Config => "config",
            RepoNodeKind::Documentation => "documentation",
        }
    }
}

/// Compute the default rebuildable cache path for a project id under a base directory, e.g.
/// `<base_dir>/index/<sanitized_project_id>.db`.
pub fn project_db_path(base_dir: &Path, project_id: &str) -> PathBuf {
    base_dir
        .join("index")
        .join(format!("{}.db", sanitize_id(project_id)))
}

fn sanitize_id(id: &str) -> String {
    let cleaned: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "default".to_string()
    } else {
        cleaned
    }
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS files (
    path          TEXT PRIMARY KEY,
    language      TEXT,
    size          INTEGER NOT NULL,
    mtime         INTEGER NOT NULL,
    content_hash  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS symbols (
    id          TEXT PRIMARY KEY,
    path        TEXT NOT NULL,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,
    start_line  INTEGER NOT NULL,
    end_line    INTEGER NOT NULL,
    signature   TEXT,
    summary     TEXT
);
CREATE INDEX IF NOT EXISTS idx_symbols_path ON symbols(path);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);

CREATE TABLE IF NOT EXISTS imports (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    path    TEXT NOT NULL,
    target  TEXT NOT NULL,
    kind    TEXT
);
CREATE INDEX IF NOT EXISTS idx_imports_path ON imports(path);

CREATE TABLE IF NOT EXISTS context_nodes (
    id            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL,
    path          TEXT NOT NULL,
    name          TEXT NOT NULL,
    language      TEXT,
    start_line    INTEGER NOT NULL,
    end_line      INTEGER NOT NULL,
    parent_id     TEXT,
    content_hash  TEXT,
    summary       TEXT
);
CREATE INDEX IF NOT EXISTS idx_context_nodes_parent ON context_nodes(parent_id);

CREATE TABLE IF NOT EXISTS summaries (
    key                          TEXT PRIMARY KEY,
    path                         TEXT NOT NULL,
    summary                      TEXT NOT NULL,
    summarizer_prompt_version    TEXT
);

CREATE TABLE IF NOT EXISTS index_metadata (
    key                      TEXT PRIMARY KEY,
    phase                    TEXT NOT NULL DEFAULT 'unindexed',
    last_indexed_unix_secs   INTEGER,
    last_error               TEXT,
    stale                    INTEGER NOT NULL DEFAULT 0,
    active_ignore_sources    TEXT NOT NULL DEFAULT '',
    files_indexed            INTEGER NOT NULL DEFAULT 0,
    skipped_ignore           INTEGER NOT NULL DEFAULT 0,
    skipped_hidden           INTEGER NOT NULL DEFAULT 0,
    skipped_binary           INTEGER NOT NULL DEFAULT 0,
    skipped_large            INTEGER NOT NULL DEFAULT 0,
    skipped_policy           INTEGER NOT NULL DEFAULT 0,
    symbols_indexed          INTEGER NOT NULL DEFAULT 0,
    summaries_cached         INTEGER NOT NULL DEFAULT 0
);
"#;
