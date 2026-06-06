use std::collections::HashMap;
use std::path::{Path, PathBuf};

use agent_protocol::{OutputStreamKind, PatchFile};
use regex::Regex;
use serde_json::Value;

use super::git::is_git_repo;

/// Best-effort unified diff from incomplete `propose_patch` JSON while the model streams args.
pub fn patch_diff_from_streaming_json(json: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<Value>(json) {
        return patch_diff_from_args(&value).ok();
    }
    for key in ["unified_diff", "diff", "patch", "content"] {
        let marker = format!("\"{key}\":");
        let Some(start) = json.find(&marker) else {
            continue;
        };
        let rest = &json[start + marker.len()..];
        let rest = rest.trim_start();
        if !rest.starts_with('"') {
            continue;
        }
        let mut out = String::new();
        let mut chars = rest[1..].chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => return Some(out),
                '\\' => {
                    let esc = chars.next()?;
                    match esc {
                        'n' => out.push('\n'),
                        't' => out.push('\t'),
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        other => {
                            out.push('\\');
                            out.push(other);
                        }
                    }
                }
                other => out.push(other),
            }
        }
        if !out.is_empty() {
            return Some(out);
        }
    }
    if looks_like_unified_diff(json) {
        return Some(json.to_string());
    }
    None
}

/// Extract unified diff text from tool arguments (tolerates common model key variants).
pub fn patch_diff_from_args(args: &Value) -> Result<String, String> {
    if let Some(s) = args.as_str() {
        if looks_like_unified_diff(s) {
            return Ok(s.to_string());
        }
        if let Ok(parsed) = serde_json::from_str::<Value>(s) {
            return patch_diff_from_args(&parsed);
        }
        return Err(
            "tool arguments must be a JSON object with a unified_diff field, or a raw diff string"
                .into(),
        );
    }

    let obj = args
        .as_object()
        .ok_or_else(|| patch_args_error(args, "expected JSON object"))?;

    for key in ["unified_diff", "diff", "patch", "content", "changes"] {
        if let Some(diff) = obj.get(key).and_then(|v| v.as_str()) {
            if !diff.trim().is_empty() {
                return Ok(diff.to_string());
            }
        }
    }

    Err(patch_args_error(
        args,
        "missing non-empty unified_diff (or diff/patch/content)",
    ))
}

fn patch_args_error(args: &Value, detail: &str) -> String {
    let keys: Vec<_> = args
        .as_object()
        .map(|o| o.keys().map(|k| k.as_str()).collect())
        .unwrap_or_default();
    if keys.is_empty() {
        format!("{detail}")
    } else {
        format!("{detail}; got keys: {}", keys.join(", "))
    }
}

fn looks_like_unified_diff(s: &str) -> bool {
    let t = s.trim();
    t.contains("@@")
        || t.starts_with("--- ")
        || t.starts_with("+++ ")
        || t.starts_with("diff --git")
}

pub fn parse_patch_files(diff: &str, root: &Path) -> Result<Vec<PatchFile>, String> {
    let plus_re = Regex::new(r"^\+\+\+ [ab]/(.+)$").map_err(|e| e.to_string())?;
    let minus_re = Regex::new(r"^--- [ab]/(.+)$").map_err(|e| e.to_string())?;
    let mut map: HashMap<String, PatchFile> = HashMap::new();
    let mut last_minus: Option<String> = None;
    for line in diff.lines() {
        if let Some(caps) = minus_re.captures(line) {
            last_minus = Some(caps.get(1).unwrap().as_str().to_string());
            continue;
        }
        if let Some(caps) = plus_re.captures(line) {
            let path = PathBuf::from(caps.get(1).unwrap().as_str());
            map.entry(path.display().to_string()).or_insert(PatchFile {
                path: root.join(&path),
                additions: 0,
                deletions: 0,
            });
        } else if line.starts_with("+++ /dev/null") {
            // File deletion: target path comes from the preceding `--- a/<path>` line.
            if let Some(path) = last_minus.clone() {
                let path = PathBuf::from(path);
                map.entry(path.display().to_string()).or_insert(PatchFile {
                    path: root.join(&path),
                    additions: 0,
                    deletions: 0,
                });
            }
        } else if line.starts_with('+') && !line.starts_with("+++") {
            if let Some(file) = map.values_mut().last() {
                file.additions += 1;
            }
        } else if line.starts_with('-') && !line.starts_with("---") {
            if let Some(file) = map.values_mut().last() {
                file.deletions += 1;
            }
        }
    }
    Ok(map.into_values().collect())
}

/// Generate a minimal unified diff transforming `old` into `new` for `rel` (a project-relative
/// path). Used by `edit_file` / `write_file` so the model only sends the intent (old/new strings
/// or full content) while we still flow through the propose → preview → apply + checkpoint
/// pipeline. A new file is represented with an empty `old`.
pub fn generate_unified_diff(rel: &str, old: &str, new: &str) -> String {
    if old == new {
        return String::new();
    }
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let mut prefix = 0usize;
    while prefix < old_lines.len()
        && prefix < new_lines.len()
        && old_lines[prefix] == new_lines[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0usize;
    while suffix < old_lines.len() - prefix
        && suffix < new_lines.len() - prefix
        && old_lines[old_lines.len() - 1 - suffix] == new_lines[new_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }

    const CTX: usize = 3;
    let ctx_start = prefix.saturating_sub(CTX);
    let old_change_end = old_lines.len() - suffix;
    let new_change_end = new_lines.len() - suffix;
    let ctx_end = (old_change_end + CTX).min(old_lines.len());

    let mut body: Vec<String> = Vec::new();
    for line in &old_lines[ctx_start..prefix] {
        body.push(format!(" {line}"));
    }
    for line in &old_lines[prefix..old_change_end] {
        body.push(format!("-{line}"));
    }
    for line in &new_lines[prefix..new_change_end] {
        body.push(format!("+{line}"));
    }
    for line in &old_lines[old_change_end..ctx_end] {
        body.push(format!(" {line}"));
    }

    let old_count = ctx_end - ctx_start;
    let new_count = (prefix - ctx_start) + (new_change_end - prefix) + (ctx_end - old_change_end);
    let old_start = if old_lines.is_empty() {
        0
    } else {
        ctx_start + 1
    };
    let new_start = if new_lines.is_empty() {
        0
    } else {
        ctx_start + 1
    };

    let mut out = format!("--- a/{rel}\n+++ b/{rel}\n");
    out.push_str(&format!(
        "@@ -{old_start},{old_count} +{new_start},{new_count} @@\n"
    ));
    out.push_str(&body.join("\n"));
    out.push('\n');
    out
}

/// Generate a unified diff that deletes `rel` (whose current content is `old`).
pub fn generate_deletion_diff(rel: &str, old: &str) -> String {
    let lines: Vec<&str> = old.lines().collect();
    let mut out = format!(
        "diff --git a/{rel} b/{rel}\ndeleted file mode 100644\n--- a/{rel}\n+++ /dev/null\n"
    );
    out.push_str(&format!("@@ -1,{} +0,0 @@\n", lines.len()));
    for line in &lines {
        out.push_str(&format!("-{line}\n"));
    }
    out
}

/// Build a validated [`PatchProposal`] from a unified diff, reusing the same secret scan, path
/// policy, and structure validation as `propose_patch` (single source of truth).
pub fn make_patch_proposal(
    diff: &str,
    summary: &str,
    ctx: &agent_protocol::ToolContext,
) -> Result<agent_protocol::PatchProposal, String> {
    use agent_sandbox::{PathPolicy, SecretScanner};
    let scanner = SecretScanner::default();
    if scanner.scan(diff) == agent_sandbox::SecretAction::Critical {
        return Err("patch contains suspected secrets".into());
    }
    let normalized = validate_patch_structure(diff)?;
    let files = parse_patch_files(&normalized, &ctx.project_root)?;
    let policy = PathPolicy::new(&ctx.project_root);
    for file in &files {
        policy.validate_write(&file.path)?;
    }
    Ok(agent_protocol::PatchProposal {
        id: agent_protocol::PatchId::new(uuid_simple()),
        run_id: ctx.run_id.clone(),
        base_git_sha: current_git_head(&ctx.project_root),
        files,
        unified_diff: normalized,
        summary: summary.to_string(),
        risk: agent_protocol::RiskLevel::Low,
    })
}

/// Whether the per-file portion of `full_diff` for `rel` represents a deletion.
fn is_deletion_for(full_diff: &str, rel: &Path) -> bool {
    let rel_str = rel.to_string_lossy();
    let mut saw_minus = false;
    for line in full_diff.lines() {
        if line.starts_with("--- ") {
            saw_minus = line.contains(&format!("a/{rel_str}")) || line.ends_with(&*rel_str);
        } else if saw_minus && line.starts_with("+++ /dev/null") {
            return true;
        } else if line.starts_with("+++ ") {
            saw_minus = false;
        }
    }
    false
}

/// Short human-readable preview for patch tool args (file paths + line counts).
pub fn patch_diff_preview(diff: &str) -> String {
    let re = match Regex::new(r"^\+\+\+ [ab]/(.+)$") {
        Ok(r) => r,
        Err(_) => return "patch".into(),
    };
    let mut paths = Vec::new();
    let mut adds = 0usize;
    let mut dels = 0usize;
    for line in diff.lines() {
        if let Some(caps) = re.captures(line) {
            paths.push(caps.get(1).unwrap().as_str().to_string());
        } else if line.starts_with('+') && !line.starts_with("+++") {
            adds += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            dels += 1;
        }
    }
    if paths.len() == 1 {
        format!("{} (+{adds} −{dels})", paths[0])
    } else if paths.is_empty() {
        format!("patch (+{adds} −{dels})")
    } else {
        format!("{} files (+{adds} −{dels})", paths.len())
    }
}

/// Validate patch structure only (for propose — preview without requiring exact file match).
pub fn validate_patch_structure(diff: &str) -> Result<String, String> {
    let normalized = normalize_unified_diff(diff);
    if normalized.trim().is_empty() {
        let preview: String = diff.chars().take(120).collect();
        return Err(format!(
            "empty or unrecognizable patch (input len={}, preview={preview:?})",
            diff.len()
        ));
    }
    if !normalized.lines().any(|l| l.starts_with("@@")) {
        return Err("patch has no hunks (expected @@ lines in unified diff format)".into());
    }
    Ok(normalized)
}

/// Validate that a patch can be applied to the current files on disk.
pub fn validate_patch_applicable(root: &Path, diff: &str) -> Result<(), String> {
    let normalized = normalize_unified_diff(diff);
    if normalized.trim().is_empty() {
        return Err("empty or unrecognizable patch".into());
    }
    // File deletions are handled deterministically in-memory (git-apply deletion formatting is
    // fragile); everything else can use the faster `git apply --check` in a repo.
    let has_deletion = normalized.contains("+++ /dev/null");
    if is_git_repo(root) && !has_deletion {
        match git_apply(root, &normalized, true, root) {
            Ok(()) => Ok(()),
            Err(git_err) if is_git_format_error(&git_err) => {
                apply_unified_diff_in_memory(root, &normalized, true, None)
                    .map_err(|e| format!("patch validation failed: {e}"))
            }
            Err(_) => apply_unified_diff_in_memory(root, &normalized, true, None),
        }
    } else {
        apply_unified_diff_in_memory(root, &normalized, true, None)
    }
}

/// Back-compat alias.
pub fn validate_patch(root: &Path, diff: &str) -> Result<(), String> {
    validate_patch_applicable(root, diff)
}

/// Normalize model-generated unified diffs into a git-apply-compatible patch.
pub fn normalize_unified_diff(input: &str) -> String {
    let trimmed = strip_markdown_fence(input.trim());
    let lines: Vec<&str> = trimmed.lines().collect();
    let start = lines
        .iter()
        .position(|l| {
            l.starts_with("diff --git")
                || l.starts_with("--- ")
                || l.starts_with("+++ ")
                || l.starts_with("@@")
        })
        .unwrap_or(0);

    let mut out = Vec::new();
    let mut in_hunk = false;

    for line in &lines[start..] {
        if line.starts_with("diff --git") || line.starts_with("--- ") || line.starts_with("+++ ") {
            in_hunk = false;
            out.push((*line).to_string());
            continue;
        }
        if !in_hunk
            && (line.starts_with('-') || line.starts_with('+'))
            && !line.starts_with("---")
            && !line.starts_with("+++")
        {
            in_hunk = true;
            out.push(format!("@@ -1,1 +1,1 @@"));
            out.push((*line).to_string());
            continue;
        }
        if line.starts_with("@@") {
            in_hunk = true;
            out.push((*line).to_string());
            continue;
        }
        if line.starts_with("index ") {
            in_hunk = false;
            out.push((*line).to_string());
            continue;
        }
        if in_hunk {
            if line.starts_with('\\') {
                out.push((*line).to_string());
                continue;
            }
            if line.is_empty() {
                out.push(" ".to_string());
                continue;
            }
            if line.starts_with(' ') || line.starts_with('+') || line.starts_with('-') {
                out.push((*line).to_string());
            } else {
                out.push(format!(" {line}"));
            }
        }
    }

    let fixed = fix_hunk_headers(out);
    let mut result = fixed.join("\n");
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

fn strip_markdown_fence(s: &str) -> String {
    let s = s.trim();
    if !s.starts_with("```") {
        return s.to_string();
    }
    let Some(body_start) = s.find('\n') else {
        return String::new();
    };
    let body = &s[body_start + 1..];
    if let Some(end) = body.rfind("```") {
        body[..end].trim_end().to_string()
    } else {
        body.to_string()
    }
}

fn fix_hunk_headers(lines: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        if !line.starts_with("@@") {
            result.push(line.clone());
            i += 1;
            continue;
        }
        let (old_start, new_start) = parse_hunk_range_starts(line);
        let mut hunk_body = Vec::new();
        i += 1;
        while i < lines.len() {
            let l = &lines[i];
            if l.starts_with("@@")
                || l.starts_with("diff --git")
                || l.starts_with("--- ")
                || l.starts_with("+++ ")
            {
                break;
            }
            hunk_body.push(lines[i].clone());
            i += 1;
        }
        let (old_count, new_count) = count_hunk_lines(&hunk_body);
        result.push(format!(
            "@@ -{old_start},{old_count} +{new_start},{new_count} @@"
        ));
        result.extend(hunk_body);
    }
    result
}

fn parse_hunk_range_starts(header: &str) -> (usize, usize) {
    let re = match Regex::new(r"^@@ -(\d+)(?:,\d+)? \+(\d+)") {
        Ok(r) => r,
        Err(_) => return (1, 1),
    };
    if let Some(caps) = re.captures(header) {
        let old = caps
            .get(1)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(1);
        let new = caps
            .get(2)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(1);
        (old, new)
    } else {
        (1, 1)
    }
}

fn count_hunk_lines(body: &[String]) -> (usize, usize) {
    let mut old = 0usize;
    let mut new = 0usize;
    for line in body {
        if line.starts_with('\\') {
            continue;
        }
        if line.starts_with('+') {
            new += 1;
        } else if line.starts_with('-') {
            old += 1;
        } else if line.starts_with(' ') {
            old += 1;
            new += 1;
        }
    }
    (old, new)
}

fn is_git_format_error(msg: &str) -> bool {
    msg.contains("corrupt patch") || msg.contains("unrecognized input")
}

pub fn apply_unified_diff(
    root: &Path,
    diff: &str,
    checkpoint_dir: &Path,
    sink: Option<&agent_protocol::ToolOutputSink>,
) -> Result<(), String> {
    let normalized = normalize_unified_diff(diff);
    let has_deletion = normalized.contains("+++ /dev/null");
    if is_git_repo(root) && !has_deletion {
        match git_apply(root, &normalized, false, checkpoint_dir) {
            Ok(()) => {
                emit_applied_files(root, &normalized, sink);
                Ok(())
            }
            Err(git_err) if is_git_format_error(&git_err) => {
                apply_unified_diff_in_memory(root, &normalized, false, sink)
            }
            Err(_) => apply_unified_diff_in_memory(root, &normalized, false, sink),
        }
    } else {
        apply_unified_diff_in_memory(root, &normalized, false, sink)
    }
}

fn emit_applied_files(root: &Path, diff: &str, sink: Option<&agent_protocol::ToolOutputSink>) {
    let Some(sink) = sink else {
        return;
    };
    if let Ok(files) = parse_patch_files(diff, root) {
        for file in files {
            let rel = file
                .path
                .strip_prefix(root)
                .unwrap_or(&file.path)
                .display()
                .to_string();
            (sink.emit)(OutputStreamKind::Stdout, format!("✓ {rel}\n"));
        }
    }
}

fn apply_unified_diff_in_memory(
    root: &Path,
    diff: &str,
    dry_run: bool,
    sink: Option<&agent_protocol::ToolOutputSink>,
) -> Result<(), String> {
    let files = parse_patch_files(diff, root)?;
    for file in files {
        let rel = file
            .path
            .strip_prefix(root)
            .unwrap_or(&file.path)
            .to_path_buf();
        if is_deletion_for(diff, &rel) {
            if dry_run {
                if !file.path.exists() {
                    return Err(format!("cannot delete `{}`: not found", rel.display()));
                }
            } else if file.path.exists() {
                std::fs::remove_file(&file.path).map_err(|e| e.to_string())?;
                if let Some(sink) = sink {
                    (sink.emit)(OutputStreamKind::Stdout, format!("✗ {}\n", rel.display()));
                }
            }
            continue;
        }
        let existing = if file.path.exists() {
            std::fs::read_to_string(&file.path).map_err(|e| e.to_string())?
        } else {
            String::new()
        };
        let file_diff = extract_file_diff(diff, &rel);
        if file_diff.is_empty() {
            return Err(format!("could not extract diff for `{}`", rel.display()));
        }
        let updated = apply_patch_to_string(&existing, &file_diff)?;
        if !dry_run {
            if let Some(parent) = file.path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&file.path, updated).map_err(|e| e.to_string())?;
            if let Some(sink) = sink {
                (sink.emit)(OutputStreamKind::Stdout, format!("✓ {}\n", rel.display()));
            }
        }
    }
    Ok(())
}

fn extract_file_diff(full_diff: &str, rel: &Path) -> String {
    let rel_str = rel.to_string_lossy();
    let marker = format!("+++ b/{rel_str}");
    let mut out = String::new();
    let mut in_file = false;
    let mut started = false;

    for line in full_diff.lines() {
        if line.starts_with("diff --git") {
            if in_file && started {
                break;
            }
            continue;
        }
        if line.starts_with("+++ ") {
            in_file = line == marker
                || line.ends_with(&*rel_str)
                || line.contains(&format!(" b/{rel_str}"));
            if in_file {
                started = true;
            }
        }
        if in_file {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

pub fn apply_patch_to_string(base: &str, diff: &str) -> Result<String, String> {
    let normalized = normalize_unified_diff(diff);
    let diff = if normalized.trim().is_empty() {
        diff
    } else {
        normalized.as_str()
    };
    let had_trailing_newline = base.ends_with('\n');
    let mut lines: Vec<String> = base.lines().map(|l| l.to_string()).collect();

    for hunk in parse_hunks(diff) {
        let anchor = hunk_anchor_lines(&hunk.lines);
        let hint = hunk.old_start.saturating_sub(1);
        let idx = if anchor.is_empty() {
            hint.min(lines.len())
        } else {
            find_anchor_position(&lines, &anchor, hint).ok_or_else(|| {
                let preview = anchor
                    .first()
                    .map(|l| format!("expected `{l}`"))
                    .unwrap_or_else(|| "hunk context not found".into());
                format!("patch does not apply; {preview}")
            })?
        };
        apply_hunk_at(&mut lines, idx, &hunk.lines);
    }

    let mut result = lines.join("\n");
    if had_trailing_newline && (!result.is_empty() || base.is_empty()) {
        result.push('\n');
    }
    Ok(result)
}

#[derive(Clone, Debug)]
enum HunkLineKind {
    Context,
    Remove,
    Add,
}

#[derive(Clone, Debug)]
struct HunkLine {
    kind: HunkLineKind,
    text: String,
}

#[derive(Clone, Debug)]
struct PatchHunk {
    old_start: usize,
    lines: Vec<HunkLine>,
}

fn parse_hunks(diff: &str) -> Vec<PatchHunk> {
    let mut hunks = Vec::new();
    let mut current: Option<PatchHunk> = None;

    for line in diff.lines() {
        if line.starts_with("@@") {
            if let Some(hunk) = current.take() {
                hunks.push(hunk);
            }
            current = Some(PatchHunk {
                old_start: parse_hunk_old_start(line).unwrap_or(1),
                lines: Vec::new(),
            });
            continue;
        }
        if line.starts_with("+++ ") || line.starts_with("--- ") || line.starts_with("diff --git") {
            continue;
        }
        let Some(hunk) = current.as_mut() else {
            continue;
        };
        if line.starts_with('+') {
            hunk.lines.push(HunkLine {
                kind: HunkLineKind::Add,
                text: line[1..].to_string(),
            });
        } else if line.starts_with('-') {
            hunk.lines.push(HunkLine {
                kind: HunkLineKind::Remove,
                text: line[1..].to_string(),
            });
        } else if line.starts_with(' ') {
            hunk.lines.push(HunkLine {
                kind: HunkLineKind::Context,
                text: line[1..].to_string(),
            });
        }
    }
    if let Some(hunk) = current {
        hunks.push(hunk);
    }
    hunks
}

fn hunk_anchor_lines(lines: &[HunkLine]) -> Vec<String> {
    lines
        .iter()
        .filter(|l| !matches!(l.kind, HunkLineKind::Add))
        .map(|l| l.text.clone())
        .collect()
}

fn find_anchor_position(lines: &[String], anchor: &[String], hint: usize) -> Option<usize> {
    if anchor.is_empty() {
        return Some(hint.min(lines.len()));
    }
    if matches_anchor(lines, hint, anchor) {
        return Some(hint);
    }
    let max_start = lines.len().saturating_sub(anchor.len());
    for i in 0..=max_start {
        if i != hint && matches_anchor(lines, i, anchor) {
            return Some(i);
        }
    }
    None
}

fn matches_anchor(lines: &[String], start: usize, anchor: &[String]) -> bool {
    anchor.iter().enumerate().all(|(offset, expected)| {
        lines
            .get(start + offset)
            .is_some_and(|actual| actual == expected)
    })
}

fn apply_hunk_at(lines: &mut Vec<String>, mut idx: usize, hunk: &[HunkLine]) {
    for line in hunk {
        match line.kind {
            HunkLineKind::Add => {
                lines.insert(idx, line.text.clone());
                idx += 1;
            }
            HunkLineKind::Remove => {
                if idx < lines.len() {
                    lines.remove(idx);
                }
            }
            HunkLineKind::Context => {
                idx += 1;
            }
        }
    }
}

fn parse_hunk_old_start(line: &str) -> Option<usize> {
    let re = Regex::new(r"^@@ -(\d+)").ok()?;
    re.captures(line)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
}

fn git_apply(root: &Path, diff: &str, check_only: bool, patch_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(patch_dir).map_err(|e| e.to_string())?;
    let patch_path = patch_dir.join(format!("patch-{}.diff", uuid_simple()));
    std::fs::write(&patch_path, diff).map_err(|e| e.to_string())?;

    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(root).args(["apply", "--whitespace=nowarn"]);
    if check_only {
        cmd.arg("--check");
    }
    cmd.arg(&patch_path);

    let output = cmd.output().map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&patch_path);

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !stderr.is_empty() {
            Err(stderr)
        } else if !stdout.is_empty() {
            Err(stdout)
        } else {
            Err("git apply failed".into())
        }
    }
}

pub fn current_git_head(root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn uuid_simple() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    fn write_file(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn apply_single_hunk_edit() {
        let base = "# Title\n\nOld line\n\nFooter\n";
        let diff = "\
--- a/README.md
+++ b/README.md
@@ -1,5 +1,5 @@
 # Title

-Old line
+New line

 Footer
";
        let result = apply_patch_to_string(base, diff).unwrap();
        assert!(result.contains("New line"));
        assert!(!result.contains("Old line"));
    }

    #[test]
    fn apply_multi_hunk() {
        let base = "line1\nline2\nline3\nline4\n";
        let diff = "\
--- a/file.txt
+++ b/file.txt
@@ -1,2 +1,2 @@
-line1
+LINE1
 line2
@@ -3,2 +3,2 @@
 line3
-line4
+LINE4
";
        let result = apply_patch_to_string(base, diff).unwrap();
        assert_eq!(result, "LINE1\nline2\nline3\nLINE4\n");
    }

    #[test]
    fn apply_new_file_in_memory() {
        let dir = TempDir::new().unwrap();
        let diff = "\
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,2 @@
+hello
+world
";
        apply_unified_diff_in_memory(dir.path(), diff, false, None).unwrap();
        let content = fs::read_to_string(dir.path().join("new.txt")).unwrap();
        assert_eq!(content, "hello\nworld");
    }

    #[test]
    fn extract_file_diff_uses_relative_path() {
        let diff = "\
diff --git a/README.md b/README.md
--- a/README.md
+++ b/README.md
@@ -1,3 +1,3 @@
 # Title
-old
+new
";
        let extracted = extract_file_diff(diff, Path::new("README.md"));
        assert!(extracted.contains("-old"));
        assert!(extracted.contains("+new"));
    }

    #[test]
    fn context_mismatch_reports_line() {
        let base = "alpha\nbeta\ngamma\n";
        let diff = "\
--- a/f.txt
+++ b/f.txt
@@ -1,3 +1,3 @@
 alpha
-WRONG
+fixed
 gamma
";
        let err = apply_patch_to_string(base, diff).unwrap_err();
        assert!(err.contains("patch does not apply") || err.contains("context"));
    }

    #[test]
    fn fuzzy_finds_hunk_when_line_numbers_stale() {
        let base = "line1\nline2\nmacOS 14+\nline4\n";
        let diff = "\
--- a/README.md
+++ b/README.md
@@ -99,1 +99,1 @@
-macOS 14+
+macOS 15+
";
        let result = apply_patch_to_string(base, diff).unwrap();
        assert!(result.contains("macOS 15+"));
        assert!(!result.contains("macOS 14+"));
    }

    #[test]
    fn propose_structure_allows_stale_context() {
        let diff = "\
--- a/README.md
+++ b/README.md
@@ -34,1 +34,1 @@
-stale line not in file
+new line
";
        assert!(validate_patch_structure(diff).is_ok());
        assert!(validate_patch_applicable(Path::new("/nonexistent"), diff).is_err());
    }

    #[test]
    fn parse_patch_files_counts_additions_deletions() {
        let dir = TempDir::new().unwrap();
        let diff = "\
--- a/a.txt
+++ b/a.txt
@@ -1,2 +1,2 @@
-old
+new
-extra
";
        let files = parse_patch_files(diff, dir.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].additions, 1);
        assert_eq!(files[0].deletions, 2);
    }

    #[test]
    fn normalize_fixes_unprefixed_hunk_lines_and_header_counts() {
        let raw = "\
--- a/foo.swift
+++ b/foo.swift
@@ -1,1 +1,1 @@
import Foundation
-old
+new
";
        let normalized = normalize_unified_diff(raw);
        assert!(normalized.contains(" import Foundation"));
        assert!(normalized.contains("-old"));
        assert!(normalized.contains("+new"));
        assert!(normalized.contains("@@ -1,3 +1,3 @@") || normalized.contains("@@ -1,2 +1,2 @@"));
    }

    #[test]
    fn patch_diff_from_args_accepts_diff_key() {
        let args = json!({
            "diff": "--- a/x.txt\n+++ b/x.txt\n@@ -1 +1 @@\n-old\n+new\n"
        });
        let diff = patch_diff_from_args(&args).unwrap();
        assert!(diff.contains("@@"));
    }

    #[test]
    fn patch_diff_from_args_rejects_empty_unified_diff() {
        let args = json!({ "unified_diff": "", "summary": "fix" });
        assert!(patch_diff_from_args(&args).is_err());
    }

    #[test]
    fn normalize_strips_markdown_fence() {
        let raw = "```diff\n--- a/x.txt\n+++ b/x.txt\n@@ -1 +1 @@\n-old\n+new\n```";
        let normalized = normalize_unified_diff(raw);
        assert!(normalized.starts_with("--- a/x.txt"));
        assert!(!normalized.contains("```"));
    }

    #[test]
    fn validate_accepts_model_style_patch_in_git_repo() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(root)
            .output()
            .unwrap();
        write_file(root, "foo.swift", "import Foundation\nold\n");
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(root)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@test.com")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@test.com")
            .output()
            .unwrap();

        let raw = "\
--- a/foo.swift
+++ b/foo.swift
@@ -1,2 +1,2 @@
import Foundation
-old
+new
";
        assert!(validate_patch(root, raw).is_ok());
    }

    #[test]
    fn generated_edit_diff_applies() {
        let old = "line1\nline2\nTARGET\nline4\nline5\n";
        let new = "line1\nline2\nREPLACED\nline4\nline5\n";
        let diff = generate_unified_diff("f.txt", old, new);
        assert!(diff.contains("-TARGET"));
        assert!(diff.contains("+REPLACED"));
        let result = apply_patch_to_string(old, &diff).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn generated_new_file_diff_applies_in_memory() {
        let dir = TempDir::new().unwrap();
        let diff = generate_unified_diff("new.txt", "", "alpha\nbeta\n");
        apply_unified_diff_in_memory(dir.path(), &diff, false, None).unwrap();
        let content = fs::read_to_string(dir.path().join("new.txt")).unwrap();
        assert_eq!(content, "alpha\nbeta");
    }

    #[test]
    fn generated_deletion_diff_removes_file() {
        let dir = TempDir::new().unwrap();
        write_file(dir.path(), "gone.txt", "a\nb\nc\n");
        let diff = generate_deletion_diff("gone.txt", "a\nb\nc\n");
        let files = parse_patch_files(&diff, dir.path()).unwrap();
        assert_eq!(files.len(), 1);
        apply_unified_diff_in_memory(dir.path(), &diff, false, None).unwrap();
        assert!(!dir.path().join("gone.txt").exists());
    }

    #[test]
    fn git_apply_in_git_repo() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(root)
            .output()
            .unwrap();
        write_file(root, "README.md", "# Title\n\nOld line\n\nFooter\n");
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(root)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@test.com")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@test.com")
            .output()
            .unwrap();

        let diff = "\
diff --git a/README.md b/README.md
--- a/README.md
+++ b/README.md
@@ -1,5 +1,5 @@
 # Title

-Old line
+New line

 Footer
";
        assert!(validate_patch(root, diff).is_ok());
        apply_unified_diff(root, diff, root, None).unwrap();
        let content = fs::read_to_string(root.join("README.md")).unwrap();
        assert!(content.contains("New line"));
    }
}
