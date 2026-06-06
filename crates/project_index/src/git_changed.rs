use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Collect paths changed in the git working tree (modified, staged, untracked).
/// Returns relative paths using forward slashes. Empty when not a git repo or on failure.
pub fn git_changed_files(root: &Path) -> Vec<PathBuf> {
    let mut out = HashSet::new();
    if !is_git_repo(root) {
        return Vec::new();
    }
    collect_git_lines(root, &["diff", "--name-only", "HEAD"], &mut out);
    collect_git_lines(root, &["diff", "--name-only", "--cached"], &mut out);
    collect_status_lines(root, &mut out);
    let mut paths: Vec<PathBuf> = out.into_iter().map(PathBuf::from).collect();
    paths.sort();
    paths
}

fn is_git_repo(root: &Path) -> bool {
    std::process::Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn collect_git_lines(root: &Path, args: &[&str], out: &mut HashSet<String>) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output();
    let Ok(output) = output else {
        return;
    };
    if !output.status.success() {
        return;
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let line = line.trim();
        if !line.is_empty() {
            out.insert(line.replace('\\', "/"));
        }
    }
}

fn collect_status_lines(root: &Path, out: &mut HashSet<String>) {
    let output = std::process::Command::new("git")
        .args(["status", "--short", "--untracked-files=all"])
        .current_dir(root)
        .output();
    let Ok(output) = output else {
        return;
    };
    if !output.status.success() {
        return;
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let line = line.trim();
        if line.len() < 4 {
            continue;
        }
        // Format: "XY path" or "XY path -> path"
        let path_part = line[3..].trim();
        let path = path_part
            .split(" -> ")
            .last()
            .unwrap_or(path_part)
            .trim()
            .replace('\\', "/");
        if !path.is_empty() {
            out.insert(path);
        }
    }
}
