use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agent_models::ModelProvider;
use agent_protocol::{
    AgentError, CancellationToken, ContextEntryKind, ContextTraceEntry, ModelDelta, ModelId,
    ModelMessage, ModelMessageRole, ModelRequest, ProjectId, RunId, TaskClass,
};
use context_providers::{
    ContextNodeId, ContextService, GitProvider, PageIndexProvider, RepoIndexProvider,
};
use futures::StreamExt;
use project_index::{MapBudget, RepoIndex, project_db_path};

use crate::ChannelEventSink;

pub const CONTEXT_SELECTION_ENV: &str = "VORTEX_CONTEXT_SELECTION";

const CONTEXT_SELECTION_SNIPPET: &str = r#"
[CONTEXT SELECTION]
Before coding, choose repository context to open. Reply with exactly one <context_selection>...</context_selection> block and no other structured output.

Format:
<context_selection>
<required id="NODE_ID" reason="why this is required"/>
<optional id="NODE_ID" reason="why this is helpful"/>
</context_selection>

Use node ids from the repo map (relative file paths) or symbol ids from search. Prefix git paths as git:path/to/file when referring to changed files. Keep lists small (at most 5 required and 5 optional).
"#;

/// Parsed `<context_selection>` items from the model.
#[derive(Clone, Debug)]
pub struct ContextSelectionItem {
    pub id: String,
    #[allow(dead_code)]
    pub required: bool,
    pub reason: String,
}

/// Whether the optional pre-loop context-selection model turn is enabled.
pub fn context_selection_enabled() -> bool {
    matches!(
        std::env::var(CONTEXT_SELECTION_ENV).as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

/// Base directory for rebuildable index caches: `~/.config/vortex` (falls back to `.vortex`).
pub fn index_cache_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".config").join("vortex"))
        .unwrap_or_else(|| PathBuf::from(".vortex"))
}

/// Open (or build) the repo index for a workspace run.
pub fn open_repo_index(root: &Path, project_id: &ProjectId) -> Option<Arc<Mutex<RepoIndex>>> {
    let db_path = project_db_path(&index_cache_dir(), &project_id.0);
    RepoIndex::build(root, &db_path)
        .ok()
        .map(|index| Arc::new(Mutex::new(index)))
}

/// Compact `<repo_index>` text from a built index.
#[allow(dead_code)]
pub fn repo_map_from_index(index: &Mutex<RepoIndex>) -> Option<String> {
    let guard = index.lock().ok()?;
    let map = guard.compact_map(MapBudget::compact());
    if map.trim().is_empty() {
        None
    } else {
        Some(map)
    }
}

/// Construct the multi-provider context service for a run.
pub fn build_context_service(index: Arc<Mutex<RepoIndex>>, root: &Path) -> ContextService {
    let providers: Vec<Box<dyn context_providers::ContextProvider>> = vec![
        Box::new(RepoIndexProvider::new(index)),
        Box::new(GitProvider::new(root.to_path_buf())),
        Box::new(PageIndexProvider::new()),
    ];
    ContextService::new(providers)
}

pub fn deterministic_context_for_prompt(
    index: Option<&Arc<Mutex<RepoIndex>>>,
    root: &Path,
    project_id: &ProjectId,
    prompt: &str,
    task_class: TaskClass,
    include_repo_map: bool,
) -> (Vec<String>, Vec<ContextTraceEntry>) {
    match task_class {
        TaskClass::DependencyUpdate => gradle_dependency_context(root),
        _ => indexed_context_for_prompt(index, root, project_id, prompt, include_repo_map),
    }
}

fn gradle_dependency_context(root: &Path) -> (Vec<String>, Vec<ContextTraceEntry>) {
    let mut candidates = Vec::new();
    collect_gradle_context_files(root, root, &mut candidates, 0);
    candidates.sort();
    candidates.dedup();
    candidates.truncate(8);

    if candidates.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let mut block = String::from("[DEPENDENCY_CONTEXT]\n");
    for rel in candidates.iter().take(8) {
        block.push_str("- ");
        block.push_str(&rel.display().to_string());
        block.push('\n');
    }
    (
        vec![block],
        vec![ContextTraceEntry {
            kind: ContextEntryKind::RepoMap,
            label: "dependency context".into(),
            detail: Some(format!("{} files", candidates.len().min(8))),
            reason: "dependency update task uses Gradle/catalog files".into(),
        }],
    )
}

fn collect_gradle_context_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 5 || out.len() >= 32 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.')
            || matches!(
                name.as_str(),
                "target" | "build" | "node_modules" | ".gradle" | ".idea"
            )
        {
            continue;
        }
        if path.is_dir() {
            collect_gradle_context_files(root, &path, out, depth + 1);
            continue;
        }
        if matches!(
            name.as_str(),
            "build.gradle"
                | "build.gradle.kts"
                | "settings.gradle"
                | "settings.gradle.kts"
                | "gradle.properties"
                | "libs.versions.toml"
                | "package.json"
        ) {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_path_buf());
            }
        }
    }
}

fn indexed_context_for_prompt(
    index: Option<&Arc<Mutex<RepoIndex>>>,
    root: &Path,
    project_id: &ProjectId,
    prompt: &str,
    include_repo_map: bool,
) -> (Vec<String>, Vec<ContextTraceEntry>) {
    let Some(index) = index.cloned().or_else(|| open_repo_index(root, project_id)) else {
        return (Vec::new(), Vec::new());
    };
    let Ok(guard) = index.lock() else {
        return (Vec::new(), Vec::new());
    };
    let mut blocks = Vec::new();
    let mut traces = Vec::new();
    if include_repo_map {
        let map = guard.compact_map(MapBudget {
            max_depth: 4,
            max_entries: 120,
            focus: None,
        });
        if !map.trim().is_empty() {
            blocks.push(map);
            traces.push(ContextTraceEntry {
                kind: ContextEntryKind::RepoMap,
                label: "Repo map".into(),
                detail: Some("compact".into()),
                reason: "lightweight repository structure for initial orientation".into(),
            });
        }
    }
    let mut summary_tokens = 0usize;
    for hit in guard.rank(prompt, 4).into_iter().take(2) {
        let id = if hit.symbol_id.is_empty() {
            hit.path.clone()
        } else {
            hit.symbol_id.clone()
        };
        if let Ok(summary) = guard.summarize_node(&id) {
            let block = format!(
                "[NODE_SUMMARY]\n{} :: {} {}\n{}\n",
                hit.path, hit.kind, hit.name, summary
            );
            let block_tokens = agent_context::estimate_tokens(&block);
            if summary_tokens + block_tokens > 500 {
                break;
            }
            summary_tokens += block_tokens;
            blocks.push(block);
            traces.push(ContextTraceEntry {
                kind: ContextEntryKind::Symbol,
                label: hit.path,
                detail: Some(format!("{} {}", hit.kind, hit.name)),
                reason: "ranked by repository index for this prompt".into(),
            });
        }
    }
    (blocks, traces)
}

/// One model turn that asks for `<context_selection>`, opens chosen nodes, and returns trace entries.
pub async fn run_context_selection(
    provider: &dyn ModelProvider,
    sink: &ChannelEventSink,
    run_id: &RunId,
    model: ModelId,
    prompt: &str,
    dynamic_prefix: Option<String>,
    service: &ContextService,
    cancel: &CancellationToken,
) -> Result<(Option<String>, Vec<ContextTraceEntry>), AgentError> {
    let mut system = String::from(CONTEXT_SELECTION_SNIPPET);
    if let Some(prefix) = dynamic_prefix {
        system = format!("{prefix}\n{system}");
    }
    let request = ModelRequest {
        model,
        messages: vec![
            ModelMessage {
                role: ModelMessageRole::System,
                content: system.into(),
                tool_call_id: None,
                name: None,
                tool_calls: None,
            },
            ModelMessage {
                role: ModelMessageRole::User,
                content: format!("[USER_REQUEST]\n{prompt}").into(),
                tool_call_id: None,
                name: None,
                tool_calls: None,
            },
        ],
        tools: Vec::new(),
        temperature: Some(0.0),
        max_tokens: Some(1024),
        prompt_cache_key: None,
        previous_response_id: None,
    };

    let mut stream = provider.stream(request, cancel.clone()).await?;
    let mut text = String::new();
    while let Some(delta) = stream.next().await {
        cancel.check_cancelled()?;
        if let ModelDelta::Text(chunk) = delta? {
            text.push_str(&chunk);
        }
    }

    let items = parse_context_selection(&text);
    if items.is_empty() {
        return Ok((None, Vec::new()));
    }

    let ids: Vec<ContextNodeId> = items
        .iter()
        .map(|item| ContextNodeId::new(&item.id))
        .collect();
    let blocks = service.open_many(&ids).await;
    let mut opened_text = String::new();
    if !blocks.is_empty() {
        opened_text.push_str("[CONTEXT OPENED]\n");
        for block in &blocks {
            opened_text.push_str(&block.content);
            opened_text.push('\n');
        }
    }

    let entries: Vec<ContextTraceEntry> = items
        .iter()
        .map(|item| {
            let kind = trace_kind_for_id(&item.id);
            ContextTraceEntry {
                kind,
                label: trace_label(&item.id),
                detail: None,
                reason: item.reason.clone(),
            }
        })
        .collect();

    if !entries.is_empty() {
        sink.emit(
            run_id,
            agent_protocol::AgentEvent::ContextTrace {
                run_id: run_id.clone(),
                entries: entries.clone(),
            },
        )
        .await
        .map_err(AgentError::Store)?;
    }

    let injected = if opened_text.trim().is_empty() {
        None
    } else {
        Some(opened_text)
    };
    Ok((injected, entries))
}

#[cfg(test)]
mod deterministic_context_tests {
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
    fn deterministic_context_uses_summaries_not_file_slices() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "src/lib.rs", "pub struct Alpha;\npub fn beta() {}\n");
        let project_id = ProjectId::new("ctx-test");
        let index = Arc::new(std::sync::Mutex::new(
            RepoIndex::build_in_memory(root).expect("repo index"),
        ));
        let (blocks, traces) = deterministic_context_for_prompt(
            Some(&index),
            root,
            &project_id,
            "find beta",
            TaskClass::BugFix,
            true,
        );
        assert!(!blocks.is_empty());
        assert!(blocks.iter().all(|block| !block.contains("<file_slice")));
        assert!(
            traces
                .iter()
                .any(|entry| entry.kind == ContextEntryKind::RepoMap)
        );
    }
}

fn trace_kind_for_id(id: &str) -> ContextEntryKind {
    if id.starts_with("git:") {
        ContextEntryKind::FileSlice
    } else if id.contains(':') && !id.starts_with("page:") {
        ContextEntryKind::Symbol
    } else {
        ContextEntryKind::FileSlice
    }
}

fn trace_label(id: &str) -> String {
    id.strip_prefix("git:")
        .or_else(|| id.strip_prefix("repo:"))
        .or_else(|| id.strip_prefix("page:"))
        .unwrap_or(id)
        .to_string()
}

/// Extract required/optional context items from model output.
pub fn parse_context_selection(text: &str) -> Vec<ContextSelectionItem> {
    let start_tag = "<context_selection>";
    let end_tag = "</context_selection>";
    let Some(start) = text.find(start_tag) else {
        return Vec::new();
    };
    let inner_start = start + start_tag.len();
    let Some(end_rel) = text[inner_start..].find(end_tag) else {
        return Vec::new();
    };
    let inner = &text[inner_start..inner_start + end_rel];
    parse_context_selection_inner(inner)
}

fn parse_context_selection_inner(inner: &str) -> Vec<ContextSelectionItem> {
    let mut items = Vec::new();
    for line in inner.lines() {
        let line = line.trim();
        if !line.starts_with('<') {
            continue;
        }
        let tag = line
            .trim_start_matches('<')
            .split_whitespace()
            .next()
            .unwrap_or("");
        let required = matches!(tag, "required");
        let optional = matches!(tag, "optional");
        if !required && !optional {
            continue;
        }
        let id = extract_xml_attr(line, "id").unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        let reason = extract_xml_attr(line, "reason").unwrap_or_else(|| "selected".into());
        items.push(ContextSelectionItem {
            id,
            required,
            reason,
        });
    }
    items
}

fn extract_xml_attr(line: &str, key: &str) -> Option<String> {
    let marker = format!("{key}=\"");
    let start = line.find(&marker)? + marker.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_required_and_optional_items() {
        let text = r#"Notes
<context_selection>
<required id="repo:src/lib.rs" reason="main entry"/>
<optional id="git:src/lib.rs" reason="local edits"/>
</context_selection>
"#;
        let items = parse_context_selection(text);
        assert_eq!(items.len(), 2);
        assert!(items[0].required);
        assert!(!items[1].required);
        assert_eq!(items[0].id, "repo:src/lib.rs");
    }
}
