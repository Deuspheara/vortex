use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentRunLimits {
    pub max_model_loops: usize,
    pub max_tool_calls: usize,
    pub max_runtime_seconds: u64,
    pub max_context_tokens: usize,
    pub max_output_tokens: usize,
    pub max_patch_bytes: usize,
    pub max_files_touched: usize,
}

impl Default for AgentRunLimits {
    fn default() -> Self {
        Self {
            max_model_loops: 32,
            max_tool_calls: 64,
            max_runtime_seconds: 900,
            max_context_tokens: 128_000,
            max_output_tokens: 16_384,
            max_patch_bytes: 512_000,
            max_files_touched: 50,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextBudget {
    pub max_tokens: usize,
    pub reserved_for_response: usize,
    pub reserved_for_tools: usize,
    pub max_file_tokens: usize,
    pub max_history_tokens: usize,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            max_tokens: 100_000,
            reserved_for_response: 8_000,
            reserved_for_tools: 16_000,
            max_file_tokens: 8_000,
            max_history_tokens: 32_000,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BudgetPolicy {
    pub max_tokens_per_run: u64,
    pub max_cost_per_run_usd: Option<f64>,
    pub max_cost_per_day_usd: Option<f64>,
    pub warn_at_percent: u8,
}

impl Default for BudgetPolicy {
    fn default() -> Self {
        Self {
            max_tokens_per_run: 500_000,
            max_cost_per_run_usd: Some(1.0),
            max_cost_per_day_usd: Some(10.0),
            warn_at_percent: 80,
        }
    }
}
