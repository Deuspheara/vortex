use crate::features::shell::state::IndexPhase;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadinessState {
    Ready,
    InProgress,
    NeedsAttention,
}

impl ReadinessState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::InProgress => "In progress",
            Self::NeedsAttention => "Needs attention",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceReadinessCheck {
    pub key: &'static str,
    pub title: &'static str,
    pub detail: String,
    pub state: ReadinessState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceReadiness {
    pub provider_connected: bool,
    pub uses_mock_provider: bool,
    pub has_project: bool,
    pub project_trusted: bool,
    pub index_phase: Option<IndexPhase>,
    pub checks: Vec<WorkspaceReadinessCheck>,
    pub recommended_action: String,
}

impl Default for WorkspaceReadiness {
    fn default() -> Self {
        build_workspace_readiness(WorkspaceReadinessInputs::default())
    }
}

impl WorkspaceReadiness {
    pub fn ready_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|check| check.state == ReadinessState::Ready)
            .count()
    }

    pub fn total_count(&self) -> usize {
        self.checks.len()
    }

    pub fn is_ready(&self) -> bool {
        self.ready_count() == self.total_count()
    }

    pub fn summary_label(&self) -> String {
        if self.is_ready() {
            "Workspace ready".to_string()
        } else {
            format!(
                "Workspace {} / {} ready",
                self.ready_count(),
                self.total_count()
            )
        }
    }

    pub fn next_step_label(&self) -> String {
        if self.is_ready() {
            "All core checks are ready for grounded runs.".to_string()
        } else {
            self.recommended_action.clone()
        }
    }

    pub fn overall_state(&self) -> ReadinessState {
        if self.is_ready() {
            ReadinessState::Ready
        } else if self
            .checks
            .iter()
            .any(|check| check.state == ReadinessState::InProgress)
        {
            ReadinessState::InProgress
        } else {
            ReadinessState::NeedsAttention
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceReadinessInputs {
    pub provider_connected: bool,
    pub uses_mock_provider: bool,
    pub has_project: bool,
    pub project_trusted: bool,
    pub index_phase: Option<IndexPhase>,
    pub context_trace_groups: usize,
    pub read_cache_entries: usize,
    pub page_cache_configured: bool,
}

pub fn build_workspace_readiness(inputs: WorkspaceReadinessInputs) -> WorkspaceReadiness {
    let provider_check = if inputs.provider_connected {
        WorkspaceReadinessCheck {
            key: "provider",
            title: "Provider",
            detail: "Live model provider is connected.".to_string(),
            state: ReadinessState::Ready,
        }
    } else {
        WorkspaceReadinessCheck {
            key: "provider",
            title: "Provider",
            detail: if inputs.uses_mock_provider {
                "Using the mock provider. Connect OpenRouter in Settings for real runs.".to_string()
            } else {
                "Provider is not connected yet.".to_string()
            },
            state: ReadinessState::NeedsAttention,
        }
    };

    let project_check = if inputs.has_project {
        WorkspaceReadinessCheck {
            key: "project",
            title: "Project",
            detail: "A workspace project is open and selected.".to_string(),
            state: ReadinessState::Ready,
        }
    } else {
        WorkspaceReadinessCheck {
            key: "project",
            title: "Project",
            detail: "Open a project folder so the agent can ground its work in a repo.".to_string(),
            state: ReadinessState::NeedsAttention,
        }
    };

    let trust_check = if !inputs.has_project {
        WorkspaceReadinessCheck {
            key: "trust",
            title: "Trust",
            detail: "Open a project before reviewing trust.".to_string(),
            state: ReadinessState::NeedsAttention,
        }
    } else if inputs.project_trusted {
        WorkspaceReadinessCheck {
            key: "trust",
            title: "Trust",
            detail: "Project is trusted for real work.".to_string(),
            state: ReadinessState::Ready,
        }
    } else {
        WorkspaceReadinessCheck {
            key: "trust",
            title: "Trust",
            detail: "Review and trust this project before you rely on it for edits.".to_string(),
            state: ReadinessState::NeedsAttention,
        }
    };

    let index_check = match inputs.index_phase {
        Some(IndexPhase::Ready) => WorkspaceReadinessCheck {
            key: "index",
            title: "Repo index",
            detail: "Repo index is ready for grounded search and reads.".to_string(),
            state: ReadinessState::Ready,
        },
        Some(
            IndexPhase::Queued
            | IndexPhase::Scanning
            | IndexPhase::Parsing
            | IndexPhase::Summarizing,
        ) => WorkspaceReadinessCheck {
            key: "index",
            title: "Repo index",
            detail:
                "Indexing is running now. Search and context quality will improve as it finishes."
                    .to_string(),
            state: ReadinessState::InProgress,
        },
        Some(IndexPhase::Stale) => WorkspaceReadinessCheck {
            key: "index",
            title: "Repo index",
            detail: "Repo changed since the last index. Refresh context before relying on results."
                .to_string(),
            state: ReadinessState::NeedsAttention,
        },
        Some(IndexPhase::Failed) => WorkspaceReadinessCheck {
            key: "index",
            title: "Repo index",
            detail: "Indexing failed. Open Context to inspect repo indexing status.".to_string(),
            state: ReadinessState::NeedsAttention,
        },
        Some(IndexPhase::Unindexed) | None => WorkspaceReadinessCheck {
            key: "index",
            title: "Repo index",
            detail: "Repo index has not finished yet.".to_string(),
            state: ReadinessState::NeedsAttention,
        },
    };

    let context_check = if inputs.context_trace_groups > 0 {
        WorkspaceReadinessCheck {
            key: "context",
            title: "Context",
            detail: format!(
                "Recent runs recorded {} context trace group{}.",
                inputs.context_trace_groups,
                if inputs.context_trace_groups == 1 {
                    ""
                } else {
                    "s"
                }
            ),
            state: ReadinessState::Ready,
        }
    } else if inputs.read_cache_entries > 0 {
        WorkspaceReadinessCheck {
            key: "context",
            title: "Context",
            detail: format!(
                "Read cache already holds {} file entr{} for this session.",
                inputs.read_cache_entries,
                if inputs.read_cache_entries == 1 {
                    "y"
                } else {
                    "ies"
                }
            ),
            state: ReadinessState::Ready,
        }
    } else if matches!(inputs.index_phase, Some(IndexPhase::Ready)) {
        WorkspaceReadinessCheck {
            key: "context",
            title: "Context",
            detail: if inputs.page_cache_configured {
                "Repo index is ready and page-cache support is configured.".to_string()
            } else {
                "Repo index is ready. Your next run can build grounded context from it.".to_string()
            },
            state: ReadinessState::Ready,
        }
    } else if matches!(
        inputs.index_phase,
        Some(
            IndexPhase::Queued
                | IndexPhase::Scanning
                | IndexPhase::Parsing
                | IndexPhase::Summarizing
        )
    ) {
        WorkspaceReadinessCheck {
            key: "context",
            title: "Context",
            detail: "Context quality will improve once indexing completes and runs begin."
                .to_string(),
            state: ReadinessState::InProgress,
        }
    } else {
        WorkspaceReadinessCheck {
            key: "context",
            title: "Context",
            detail: "Run the index and a first task to build context evidence you can inspect."
                .to_string(),
            state: ReadinessState::NeedsAttention,
        }
    };

    let checks = vec![
        provider_check,
        project_check,
        trust_check,
        index_check,
        context_check,
    ];

    let recommended_action = checks
        .iter()
        .find(|check| check.state != ReadinessState::Ready)
        .map(|check| match check.key {
            "provider" => "Connect a live model provider in Settings.".to_string(),
            "project" => "Open a project folder to ground the workspace.".to_string(),
            "trust" => "Review and trust the selected project.".to_string(),
            "index" => "Let indexing finish, or open Context to inspect index status.".to_string(),
            "context" => "Run a grounded task or open Context to inspect what the agent is using."
                .to_string(),
            _ => check.detail.clone(),
        })
        .unwrap_or_else(|| "Workspace is ready for productive runs.".to_string());

    WorkspaceReadiness {
        provider_connected: inputs.provider_connected,
        uses_mock_provider: inputs.uses_mock_provider,
        has_project: inputs.has_project,
        project_trusted: inputs.project_trusted,
        index_phase: inputs.index_phase,
        checks,
        recommended_action,
    }
}

#[cfg(test)]
mod tests {
    use super::{ReadinessState, WorkspaceReadinessInputs, build_workspace_readiness};
    use crate::features::shell::state::IndexPhase;

    #[test]
    fn flags_mock_provider_and_untrusted_project() {
        let readiness = build_workspace_readiness(WorkspaceReadinessInputs {
            has_project: true,
            index_phase: Some(IndexPhase::Ready),
            ..Default::default()
        });

        assert_eq!(readiness.ready_count(), 3);
        assert_eq!(readiness.overall_state(), ReadinessState::NeedsAttention);
        assert!(
            readiness
                .next_step_label()
                .contains("Connect a live model provider")
        );
    }

    #[test]
    fn becomes_fully_ready_when_all_signals_are_present() {
        let readiness = build_workspace_readiness(WorkspaceReadinessInputs {
            provider_connected: true,
            has_project: true,
            project_trusted: true,
            index_phase: Some(IndexPhase::Ready),
            context_trace_groups: 2,
            read_cache_entries: 4,
            page_cache_configured: true,
            ..Default::default()
        });

        assert!(readiness.is_ready());
        assert_eq!(readiness.overall_state(), ReadinessState::Ready);
    }

    #[test]
    fn marks_indexing_as_in_progress() {
        let readiness = build_workspace_readiness(WorkspaceReadinessInputs {
            provider_connected: true,
            has_project: true,
            project_trusted: true,
            index_phase: Some(IndexPhase::Scanning),
            ..Default::default()
        });

        let index = readiness
            .checks
            .iter()
            .find(|check| check.key == "index")
            .expect("index check");
        assert_eq!(index.state, ReadinessState::InProgress);
    }
}
