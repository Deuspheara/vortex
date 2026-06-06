use std::path::PathBuf;

use agent_protocol::ToolContext;
use project_index::{RepoIndex, project_db_path};

/// Base directory for rebuildable index caches: `~/.config/vortex` (falls back to `.vortex`).
pub fn index_cache_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".config").join("vortex"))
        .unwrap_or_else(|| PathBuf::from(".vortex"))
}

/// Build (or refresh) the repo index for the tool context.
pub fn open_repo_index(ctx: &ToolContext) -> Result<RepoIndex, String> {
    let db_path = project_db_path(&index_cache_dir(), &ctx.project_id.0);
    RepoIndex::build(&ctx.project_root, &db_path)
}
