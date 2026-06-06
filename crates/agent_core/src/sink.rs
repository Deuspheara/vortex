use std::sync::Arc;
use std::{collections::HashMap, sync::Mutex};

use agent_protocol::{AgentEvent, RunId};
use agent_store::EventStore;
use flume::Sender;

pub struct ChannelEventSink {
    store: Arc<dyn EventStore>,
    tx: Sender<AgentEvent>,
    pending: Mutex<HashMap<RunId, PendingEvent>>,
}

impl ChannelEventSink {
    pub fn new(store: Arc<dyn EventStore>, tx: Sender<AgentEvent>) -> Self {
        Self {
            store,
            tx,
            pending: Mutex::new(HashMap::new()),
        }
    }

    /// Push to the UI channel immediately, then persist. Use for streaming deltas.
    pub async fn emit_delta(&self, run_id: &RunId, event: AgentEvent) -> Result<(), String> {
        let _ = self.tx.send(event.clone());
        self.persist_delta(run_id, event)
    }

    /// Notify UI first so terminal states (tool finished, run ended) never stick loading.
    pub async fn emit(&self, run_id: &RunId, event: AgentEvent) -> Result<(), String> {
        let _ = self.tx.send(event.clone());
        self.flush_pending(run_id)?;
        self.persist(run_id, event)
    }

    fn persist(&self, run_id: &RunId, event: AgentEvent) -> Result<(), String> {
        self.store.record_event(run_id, event)?;
        Ok(())
    }

    /// Persist without notifying the UI (used after live chunked output).
    pub async fn persist_only(&self, run_id: &RunId, event: AgentEvent) -> Result<(), String> {
        self.flush_pending(run_id)?;
        self.persist(run_id, event)
    }

    fn persist_delta(&self, run_id: &RunId, event: AgentEvent) -> Result<(), String> {
        const MAX_COALESCED_DELTA_BYTES: usize = 2048;

        let mut guard = self.pending.lock().map_err(|e| e.to_string())?;
        if let Some(mut pending) = guard.remove(run_id) {
            if pending.try_merge(&event) {
                if pending.approx_len() >= MAX_COALESCED_DELTA_BYTES {
                    let event = pending.into_event(run_id.clone());
                    drop(guard);
                    return self.persist(run_id, event);
                }
                guard.insert(run_id.clone(), pending);
                return Ok(());
            }

            let flushed = pending.into_event(run_id.clone());
            drop(guard);
            self.persist(run_id, flushed)?;
            let mut guard = self.pending.lock().map_err(|e| e.to_string())?;
            if let Some(new_pending) = PendingEvent::new(event.clone()) {
                guard.insert(run_id.clone(), new_pending);
                Ok(())
            } else {
                drop(guard);
                self.persist(run_id, event)
            }
        } else if let Some(new_pending) = PendingEvent::new(event.clone()) {
            guard.insert(run_id.clone(), new_pending);
            Ok(())
        } else {
            drop(guard);
            self.persist(run_id, event)
        }
    }

    fn flush_pending(&self, run_id: &RunId) -> Result<(), String> {
        let pending = self
            .pending
            .lock()
            .map_err(|e| e.to_string())?
            .remove(run_id);
        if let Some(pending) = pending {
            self.persist(run_id, pending.into_event(run_id.clone()))?;
        }
        Ok(())
    }
}

enum PendingEvent {
    AssistantTextDelta {
        text: String,
    },
    ReasoningDelta {
        text: String,
    },
    ToolCallUpdated {
        call_id: agent_protocol::ToolCallId,
        args_preview: String,
        dedupe_key: Option<String>,
    },
    PatchPreviewUpdated {
        call_id: agent_protocol::ToolCallId,
        unified_diff: String,
    },
}

impl PendingEvent {
    fn new(event: AgentEvent) -> Option<Self> {
        match event {
            AgentEvent::AssistantTextDelta { text, .. } => Some(Self::AssistantTextDelta { text }),
            AgentEvent::ReasoningDelta { text, .. } => Some(Self::ReasoningDelta { text }),
            AgentEvent::ToolCallUpdated {
                call_id,
                args_preview,
                dedupe_key,
                ..
            } => Some(Self::ToolCallUpdated {
                call_id,
                args_preview,
                dedupe_key,
            }),
            AgentEvent::PatchPreviewUpdated {
                call_id,
                unified_diff,
            } => Some(Self::PatchPreviewUpdated {
                call_id,
                unified_diff,
            }),
            _ => None,
        }
    }

    fn try_merge(&mut self, event: &AgentEvent) -> bool {
        match (self, event) {
            (
                Self::AssistantTextDelta { text },
                AgentEvent::AssistantTextDelta { text: next, .. },
            ) => {
                text.push_str(next);
                true
            }
            (Self::ReasoningDelta { text }, AgentEvent::ReasoningDelta { text: next, .. }) => {
                text.push_str(next);
                true
            }
            (
                Self::ToolCallUpdated {
                    call_id,
                    args_preview,
                    dedupe_key,
                },
                AgentEvent::ToolCallUpdated {
                    call_id: next_call_id,
                    args_preview: next_preview,
                    dedupe_key: next_dedupe_key,
                    ..
                },
            ) if call_id == next_call_id => {
                *args_preview = next_preview.clone();
                *dedupe_key = next_dedupe_key.clone();
                true
            }
            (
                Self::PatchPreviewUpdated {
                    call_id,
                    unified_diff,
                },
                AgentEvent::PatchPreviewUpdated {
                    call_id: next_call_id,
                    unified_diff: next_diff,
                },
            ) if call_id == next_call_id => {
                *unified_diff = next_diff.clone();
                true
            }
            _ => false,
        }
    }

    fn approx_len(&self) -> usize {
        match self {
            Self::AssistantTextDelta { text } | Self::ReasoningDelta { text } => text.len(),
            Self::ToolCallUpdated { args_preview, .. } => args_preview.len(),
            Self::PatchPreviewUpdated { unified_diff, .. } => unified_diff.len(),
        }
    }

    fn into_event(self, run_id: RunId) -> AgentEvent {
        match self {
            Self::AssistantTextDelta { text } => AgentEvent::AssistantTextDelta { run_id, text },
            Self::ReasoningDelta { text } => AgentEvent::ReasoningDelta { run_id, text },
            Self::ToolCallUpdated {
                call_id,
                args_preview,
                dedupe_key,
            } => AgentEvent::ToolCallUpdated {
                run_id,
                call_id,
                args_preview,
                dedupe_key,
            },
            Self::PatchPreviewUpdated {
                call_id,
                unified_diff,
            } => AgentEvent::PatchPreviewUpdated {
                call_id,
                unified_diff,
            },
        }
    }
}
