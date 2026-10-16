//! Current context occupancy; cumulative usage belongs to the separate usage ledger.
use agent_protocol::AgentEvent;

#[derive(Clone, Debug, Default)]
pub struct ContextUsageState {
    pub used_tokens: Option<u64>,
    pub max_tokens: Option<u64>,
}
impl ContextUsageState {
    pub fn observe(&mut self, event: &AgentEvent, codex: bool) -> bool {
        match event {
            AgentEvent::ContextWindowUpdated {
                used_tokens,
                max_tokens,
                ..
            } => {
                self.used_tokens = Some(*used_tokens);
                self.max_tokens = max_tokens.filter(|n| *n > 0);
            }
            AgentEvent::ContextBuilt { token_estimate, .. } if !codex => {
                self.used_tokens = Some(*token_estimate as u64);
            }
            AgentEvent::UsageUpdated {
                input_tokens,
                output_tokens,
                ..
            } if !codex => {
                self.used_tokens = Some(input_tokens.saturating_add(*output_tokens));
            }
            _ => return false,
        }
        true
    }
    pub fn capacity(&self, fallback: Option<u64>) -> Option<u64> {
        self.max_tokens.or(fallback).filter(|n| *n > 0)
    }
    pub fn is_over_half(&self, fallback: Option<u64>) -> bool {
        self.used_tokens
            .zip(self.capacity(fallback))
            .is_some_and(|(used, max)| used > max / 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_protocol::RunId;
    fn usage(input: u64, output: u64) -> AgentEvent {
        AgentEvent::UsageUpdated {
            run_id: RunId::new("r"),
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: None,
            cache_write_tokens: None,
            estimated_cost_usd: None,
        }
    }
    fn snapshot(used: u64, max: Option<u64>) -> AgentEvent {
        AgentEvent::ContextWindowUpdated {
            run_id: RunId::new("r"),
            used_tokens: used,
            max_tokens: max,
        }
    }
    #[test]
    fn codex_cumulative_processing_is_not_context_occupancy() {
        let mut state = ContextUsageState::default();
        // The real todo run processed 102,469 input tokens across five requests.
        for (input, output) in [
            (19360, 95),
            (19523, 29),
            (19691, 2135),
            (21850, 107),
            (22045, 53),
        ] {
            state.observe(&usage(input, output), true);
            state.observe(&snapshot(input + output, Some(475000)), true);
        }
        assert_eq!(state.used_tokens, Some(22098));
        assert_eq!(state.capacity(None), Some(475000));
        assert!(!state.is_over_half(None));
    }
    #[test]
    fn compaction_and_capacity_changes_replace_the_snapshot() {
        let mut state = ContextUsageState::default();
        state.observe(&snapshot(300000, Some(475000)), true);
        assert!(state.is_over_half(None));
        state.observe(&snapshot(12000, Some(200000)), true);
        assert_eq!(state.used_tokens, Some(12000));
        assert!(!state.is_over_half(None));
    }
    #[test]
    fn native_usage_uses_the_latest_request_not_the_run_sum() {
        let mut state = ContextUsageState::default();
        state.observe(&usage(10000, 1000), false);
        state.observe(&usage(12000, 500), false);
        assert_eq!(state.used_tokens, Some(12500));
        assert_eq!(state.capacity(Some(200000)), Some(200000));
    }
    #[test]
    fn legacy_codex_usage_cannot_invent_context_or_capacity() {
        let mut state = ContextUsageState::default();
        assert!(!state.observe(&usage(102469, 2419), true));
        assert_eq!(state.used_tokens, None);
        assert_eq!(state.capacity(None), None);
        assert!(!state.is_over_half(None));
    }
}
