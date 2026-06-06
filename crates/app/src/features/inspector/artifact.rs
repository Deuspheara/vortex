//! Artifact store — diffs, terminal output, plans, and selection state.

use std::collections::HashMap;

use crate::features::shell::state::{DiffFile, DiffFileSummary, PlanArtifact};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArtifactId(pub String);

impl ArtifactId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ArtifactKind {
    Diff,
    File,
    Terminal,
    Test,
    Plan,
    Summary,
    Approval,
    WebSource,
    Screenshot,
    Vision,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Artifact {
    pub id: ArtifactId,
    pub kind: ArtifactKind,
    pub title: String,
    pub subtitle: Option<String>,
    pub thread_item_id: Option<String>,
    pub tool_call_id: Option<String>,
    pub diff_files: Vec<DiffFile>,
    pub file_summaries: Vec<DiffFileSummary>,
    pub terminal_output: Option<String>,
    pub plan: Option<PlanArtifact>,
    pub selected_file_ix: usize,
}

impl Artifact {
    pub fn diff(id: impl Into<String>, title: impl Into<String>, files: Vec<DiffFile>) -> Self {
        let summaries: Vec<_> = files
            .iter()
            .map(|f| DiffFileSummary {
                path: f.path.clone(),
                added: f.added,
                removed: f.removed,
            })
            .collect();
        Self {
            id: ArtifactId::new(id),
            kind: ArtifactKind::Diff,
            title: title.into(),
            subtitle: None,
            thread_item_id: None,
            tool_call_id: None,
            diff_files: files,
            file_summaries: summaries,
            terminal_output: None,
            plan: None,
            selected_file_ix: 0,
        }
    }

    pub fn terminal(
        id: impl Into<String>,
        title: impl Into<String>,
        output: String,
        tool_call_id: Option<String>,
    ) -> Self {
        Self {
            id: ArtifactId::new(id),
            kind: ArtifactKind::Terminal,
            title: title.into(),
            subtitle: None,
            thread_item_id: None,
            tool_call_id,
            diff_files: Vec::new(),
            file_summaries: Vec::new(),
            terminal_output: Some(output),
            plan: None,
            selected_file_ix: 0,
        }
    }

    pub fn tool_evidence(
        id: impl Into<String>,
        kind: ArtifactKind,
        title: impl Into<String>,
        output: String,
        tool_call_id: Option<String>,
    ) -> Self {
        Self {
            id: ArtifactId::new(id),
            kind,
            title: title.into(),
            subtitle: Some("Untrusted tool evidence".into()),
            thread_item_id: None,
            tool_call_id,
            diff_files: Vec::new(),
            file_summaries: Vec::new(),
            terminal_output: Some(output),
            plan: None,
            selected_file_ix: 0,
        }
    }

    pub fn plan(id: impl Into<String>, plan: PlanArtifact) -> Self {
        Self {
            id: ArtifactId::new(id),
            kind: ArtifactKind::Plan,
            title: "Plan".into(),
            subtitle: None,
            thread_item_id: None,
            tool_call_id: None,
            diff_files: Vec::new(),
            file_summaries: Vec::new(),
            terminal_output: None,
            plan: Some(plan),
            selected_file_ix: 0,
        }
    }

    pub fn patch_preview(id: impl Into<String>, files: Vec<DiffFile>) -> Self {
        let mut artifact = Self::diff(id, "Pending changes", files);
        artifact.kind = ArtifactKind::Summary;
        artifact
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ArtifactSelection {
    #[default]
    None,
    Selected(ArtifactId),
}

#[derive(Clone, Debug, Default)]
pub struct ArtifactStore {
    artifacts: HashMap<String, Artifact>,
    order: Vec<String>,
    primary_patch_id: Option<String>,
}

impl ArtifactStore {
    pub fn upsert(&mut self, artifact: Artifact) {
        let key = artifact.id.0.clone();
        if !self.artifacts.contains_key(&key) {
            self.order.push(key.clone());
        }
        self.artifacts.insert(key, artifact);
    }

    pub fn get(&self, id: &ArtifactId) -> Option<&Artifact> {
        self.artifacts.get(&id.0)
    }

    #[allow(dead_code)]
    pub fn get_mut(&mut self, id: &ArtifactId) -> Option<&mut Artifact> {
        self.artifacts.get_mut(&id.0)
    }

    #[allow(dead_code)]
    pub fn primary(&self) -> Option<&Artifact> {
        self.order.last().and_then(|id| self.artifacts.get(id))
    }

    #[allow(dead_code)]
    pub fn all(&self) -> impl Iterator<Item = &Artifact> {
        self.order.iter().filter_map(|id| self.artifacts.get(id))
    }

    pub fn set_primary_patch(&mut self, patch_id: Option<String>) {
        self.primary_patch_id = patch_id;
    }

    #[allow(dead_code)]
    pub fn primary_patch_id(&self) -> Option<&str> {
        self.primary_patch_id.as_deref()
    }

    pub fn update_diff_files(&mut self, id: &str, files: Vec<DiffFile>) {
        if let Some(artifact) = self.artifacts.get_mut(id) {
            artifact.file_summaries = files
                .iter()
                .map(|f| DiffFileSummary {
                    path: f.path.clone(),
                    added: f.added,
                    removed: f.removed,
                })
                .collect();
            artifact.diff_files = files;
        } else {
            self.upsert(Artifact::patch_preview(id, files));
        }
    }
}
