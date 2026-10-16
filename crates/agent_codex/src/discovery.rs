//! Find CLI installations even when launched by Finder with a minimal PATH.
use std::path::{Path, PathBuf};

pub fn discover_binary() -> PathBuf {
    // An explicit override must fail visibly rather than silently using another CLI.
    if let Some(path) = std::env::var_os("VORTEX_CODEX_BIN") {
        return path.into();
    }
    let mut directories: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        directories.extend([".local/bin", ".volta/bin", ".npm-global/bin"].map(|p| home.join(p)));
        if let Ok(versions) = std::fs::read_dir(home.join(".nvm/versions/node")) {
            let mut versions = versions.flatten().map(|e| e.path()).collect::<Vec<_>>();
            versions.sort_by_key(|path| {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .trim_start_matches('v')
                    .split('.')
                    .filter_map(|part| part.parse::<u32>().ok())
                    .collect::<Vec<_>>()
            });
            directories.extend(versions.into_iter().rev().map(|p| p.join("bin")));
        }
    }
    directories.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    directories
        .into_iter()
        .map(|p| p.join("codex"))
        .find(|p| is_executable(p))
        .unwrap_or_else(|| "codex".into())
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directories_and_non_executable_files_are_not_cli_candidates() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_executable(dir.path()));
        let file = dir.path().join("codex");
        std::fs::write(&file, "test").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
            assert!(!is_executable(&file));
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        assert!(is_executable(&file));
    }
}
