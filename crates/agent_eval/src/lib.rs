//! Reproducible eval harness for the Vortex agent tool + context layer.
//!
//! The harness runs a **fixed** suite of scripted scenarios (explore, single-file edit,
//! multi-file edit, error-recovery) against the real [`ToolRegistry`] and
//! [`ContextBuilder`], simulating a model trajectory turn-by-turn. It never calls a real
//! model; instead it measures the *inputs* the system would send and the trajectory the
//! agent would take, which is exactly what changes when we lean tool schemas, mode-gate
//! tools, add structured-edit tools, and tighten the context engine.
//!
//! Signals captured per task (see plan Phase 5):
//! - **tokens-per-task**: summed estimated request tokens across all turns.
//! - **tool-tokens-per-turn**: average tokens spent advertising tools (the mode-gating /
//!   lean-schema lever).
//! - **trajectory length**: number of tool calls taken to accomplish the task.
//! - **tool-selection accuracy**: fraction of intents resolved to their preferred,
//!   currently-available tool (e.g. a single `edit_file` vs. a two-step patch dance).
//! - **argument F1**: how well the emitted arguments match each tool's schema.
//! - **recovery rate**: fraction of tool errors followed by a *different* recovery tool.
//!
//! The token estimator is owned by the harness (not the builder) so that the metric is
//! consistent across the baseline and after runs regardless of internal builder changes.

use agent_context::ContextBuilder;
use agent_protocol::{
    AgentMode, AssistantToolCall, ModelId, ModelMessage, ModelMessageRole, ModelRequest,
    ToolCallId, ToolSpec,
};
use agent_tools::{ToolRegistry, mode_visible_tool_specs};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

/// A high-level step in a scenario. Intents are resolved against the live registry, so the
/// same fixed scenario yields a shorter, higher-accuracy trajectory once better tools exist.
#[derive(Clone)]
pub enum Intent {
    /// A concrete read/inspect call (always available).
    Call {
        tool: &'static str,
        args: Value,
        result: String,
    },
    /// Edit an existing file. Prefers `edit_file`; falls back to propose_patch + apply_patch.
    Edit {
        path: String,
        old: String,
        new: String,
    },
    /// Create/overwrite a file. Prefers `write_file`; falls back to propose_patch + apply_patch.
    Write { path: String, content: String },
    /// A deliberately failing call, followed by a sensible recovery with a different tool.
    ErrorRecover {
        bad_tool: &'static str,
        bad_args: Value,
        recover_tool: &'static str,
        recover_args: Value,
        recover_result: String,
    },
}

pub struct Scenario {
    pub name: &'static str,
    pub task: String,
    pub mode: AgentMode,
    pub intents: Vec<Intent>,
}

/// A concrete tool call in the resolved trajectory.
struct ConcreteCall {
    tool: String,
    args: Value,
    result: String,
    is_error: bool,
    /// Whether this call belongs to an intent that was resolved to its preferred single-tool form.
    #[allow(dead_code)]
    optimal_intent: bool,
    /// Whether this call is a recovery step after an error.
    is_recovery: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ScenarioReport {
    pub name: String,
    pub mode: String,
    pub tokens_per_task: usize,
    pub avg_tool_tokens_per_turn: usize,
    pub trajectory_length: usize,
    pub tool_selection_accuracy: f64,
    pub argument_f1: f64,
    pub recovery_rate: f64,
    pub turns: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EvalReport {
    pub label: String,
    pub scenarios: Vec<ScenarioReport>,
    pub total_tokens_per_task: usize,
    pub mean_trajectory_length: f64,
    pub mean_tool_selection_accuracy: f64,
    pub mean_argument_f1: f64,
    pub mean_recovery_rate: f64,
}

/// Build the fixed scenario suite. This must stay stable across baseline/after runs.
pub fn default_scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "explore",
            task: "Explain the architecture of this project.".into(),
            mode: AgentMode::ReadOnlyInspect,
            intents: vec![
                Intent::Call {
                    tool: "list_files",
                    args: json!({ "pattern": "**/*.rs", "max_files": 50 }),
                    result: lines("crates/app/src/main.rs\ncrates/agent_core/src/lib.rs", 30),
                },
                Intent::Call {
                    tool: "search_project",
                    args: json!({ "query": "fn main", "include": "**/*.rs" }),
                    result: lines("crates/app/src/main.rs:1: fn main() {", 12),
                },
                Intent::Call {
                    tool: "read_file",
                    args: json!({ "path": "crates/app/src/main.rs", "start_line": 1, "end_line": 60 }),
                    result: lines("use gpui::*;", 40),
                },
            ],
        },
        Scenario {
            name: "single_file_edit",
            task: "Fix the typo in the README heading.".into(),
            mode: AgentMode::ApplyWithApproval,
            intents: vec![
                Intent::Call {
                    tool: "read_file",
                    args: json!({ "path": "README.md", "start_line": 1, "end_line": 20 }),
                    result: lines("# Vortx\n\nA Rust agent.", 12),
                },
                Intent::Edit {
                    path: "README.md".into(),
                    old: "# Vortx".into(),
                    new: "# Vortex".into(),
                },
            ],
        },
        Scenario {
            name: "multi_file_edit",
            task: "Add a new module and wire it into the crate root.".into(),
            mode: AgentMode::ApplyWithApproval,
            intents: vec![
                Intent::Call {
                    tool: "list_files",
                    args: json!({ "path": "crates/app/src", "pattern": "*.rs" }),
                    result: lines("crates/app/src/lib.rs", 8),
                },
                Intent::Write {
                    path: "crates/app/src/feature.rs".into(),
                    content: "pub fn feature() {}\n".into(),
                },
                Intent::Edit {
                    path: "crates/app/src/lib.rs".into(),
                    old: "// modules".into(),
                    new: "// modules\npub mod feature;".into(),
                },
            ],
        },
        Scenario {
            name: "error_recovery",
            task: "Open the config file and summarize it.".into(),
            mode: AgentMode::ReadOnlyInspect,
            intents: vec![
                Intent::ErrorRecover {
                    bad_tool: "read_file",
                    bad_args: json!({ "path": "config.toml" }),
                    recover_tool: "search_project",
                    recover_args: json!({ "query": "config", "names_only": true }),
                    recover_result: lines("app.config.toml", 3),
                },
                Intent::Call {
                    tool: "read_file",
                    args: json!({ "path": "app.config.toml", "start_line": 1, "end_line": 40 }),
                    result: lines("[package]\nname = \"app\"", 20),
                },
            ],
        },
    ]
}

fn lines(seed: &str, count: usize) -> String {
    let base: Vec<&str> = seed.lines().collect();
    let mut out = Vec::new();
    for i in 0..count {
        out.push(base[i % base.len()].to_string());
    }
    out.join("\n")
}

fn resolve_intent(intent: &Intent, registry: &ToolRegistry) -> Vec<ConcreteCall> {
    match intent {
        Intent::Call { tool, args, result } => vec![ConcreteCall {
            tool: (*tool).to_string(),
            args: args.clone(),
            result: result.clone(),
            is_error: false,
            optimal_intent: true,
            is_recovery: false,
        }],
        Intent::Edit { path, old, new } => {
            if registry.get("edit_file").is_some() {
                vec![ConcreteCall {
                    tool: "edit_file".into(),
                    args: json!({ "path": path, "old_string": old, "new_string": new }),
                    result: format!("Edited {path}"),
                    is_error: false,
                    optimal_intent: true,
                    is_recovery: false,
                }]
            } else {
                let diff = fake_diff(path, old, new);
                vec![
                    ConcreteCall {
                        tool: "propose_patch".into(),
                        args: json!({ "unified_diff": diff }),
                        result: "Proposed patch".into(),
                        is_error: false,
                        optimal_intent: false,
                        is_recovery: false,
                    },
                    ConcreteCall {
                        tool: "apply_patch".into(),
                        args: json!({ "unified_diff": diff }),
                        result: format!("Applied patch to {path}"),
                        is_error: false,
                        optimal_intent: false,
                        is_recovery: false,
                    },
                ]
            }
        }
        Intent::Write { path, content } => {
            if registry.get("write_file").is_some() {
                vec![ConcreteCall {
                    tool: "write_file".into(),
                    args: json!({ "path": path, "content": content }),
                    result: format!("Wrote {path}"),
                    is_error: false,
                    optimal_intent: true,
                    is_recovery: false,
                }]
            } else {
                let diff = fake_diff(path, "", content);
                vec![
                    ConcreteCall {
                        tool: "propose_patch".into(),
                        args: json!({ "unified_diff": diff }),
                        result: "Proposed patch".into(),
                        is_error: false,
                        optimal_intent: false,
                        is_recovery: false,
                    },
                    ConcreteCall {
                        tool: "apply_patch".into(),
                        args: json!({ "unified_diff": diff }),
                        result: format!("Created {path}"),
                        is_error: false,
                        optimal_intent: false,
                        is_recovery: false,
                    },
                ]
            }
        }
        Intent::ErrorRecover {
            bad_tool,
            bad_args,
            recover_tool,
            recover_args,
            recover_result,
        } => vec![
            ConcreteCall {
                tool: (*bad_tool).to_string(),
                args: bad_args.clone(),
                result: "error: file not found".into(),
                is_error: true,
                optimal_intent: true,
                is_recovery: false,
            },
            ConcreteCall {
                tool: (*recover_tool).to_string(),
                args: recover_args.clone(),
                result: recover_result.clone(),
                is_error: false,
                optimal_intent: true,
                is_recovery: true,
            },
        ],
    }
}

fn fake_diff(path: &str, old: &str, new: &str) -> String {
    format!(
        "--- a/{path}\n+++ b/{path}\n@@ -1,1 +1,1 @@\n-{old}\n+{new}\n",
        path = path,
        old = old,
        new = new
    )
}

/// Harness-owned token estimate, applied identically to baseline and after runs.
fn estimate_request_tokens(req: &ModelRequest) -> usize {
    let mut chars = 0usize;
    for m in &req.messages {
        chars += m.content.estimated_chars();
        if let Some(name) = &m.name {
            chars += name.len();
        }
        if let Some(tcs) = &m.tool_calls {
            for tc in tcs {
                chars += tc.name.len();
                chars += tc.arguments.to_string().len();
            }
        }
    }
    chars += tools_chars(&req.tools);
    chars / 4
}

fn tools_chars(tools: &[ToolSpec]) -> usize {
    serde_json::to_string(tools).map(|s| s.len()).unwrap_or(0)
}

fn estimate_tool_tokens(tools: &[ToolSpec]) -> usize {
    tools_chars(tools) / 4
}

fn assistant_tool_call_msg(call: &ConcreteCall, id: &str) -> ModelMessage {
    ModelMessage {
        role: ModelMessageRole::Assistant,
        content: String::new().into(),
        tool_call_id: None,
        name: None,
        tool_calls: Some(vec![AssistantToolCall {
            id: ToolCallId::new(id.to_string()),
            name: call.tool.clone(),
            arguments: call.args.clone(),
        }]),
    }
}

fn tool_result_msg(call: &ConcreteCall, id: &str) -> ModelMessage {
    ModelMessage {
        role: ModelMessageRole::Tool,
        content: call.result.clone().into(),
        tool_call_id: Some(ToolCallId::new(id.to_string())),
        name: Some(call.tool.clone()),
        tool_calls: None,
    }
}

/// Argument F1 for a single call against its tool schema.
fn argument_f1(call: &ConcreteCall, registry: &ToolRegistry) -> f64 {
    let Some(tool) = registry.get(&call.tool) else {
        // Tool doesn't exist in this registry → arguments cannot match → 0.
        return 0.0;
    };
    let schema = tool.schema();
    let valid: Vec<String> = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    let required: Vec<String> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let provided: Vec<String> = call
        .args
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();

    if provided.is_empty() && required.is_empty() {
        return 1.0;
    }
    let tp = provided.iter().filter(|k| valid.contains(k)).count() as f64;
    let precision = if provided.is_empty() {
        1.0
    } else {
        tp / provided.len() as f64
    };
    let recall = if required.is_empty() {
        1.0
    } else {
        let have = required.iter().filter(|k| provided.contains(k)).count() as f64;
        have / required.len() as f64
    };
    if precision + recall == 0.0 {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    }
}

pub fn run_scenario(
    scenario: &Scenario,
    registry: &ToolRegistry,
    builder: &ContextBuilder,
) -> ScenarioReport {
    let full_specs = registry.tool_specs();
    let model = ModelId::new("mock-eval");

    // Resolve the full concrete trajectory.
    let mut calls: Vec<ConcreteCall> = Vec::new();
    for intent in &scenario.intents {
        calls.extend(resolve_intent(intent, registry));
    }

    let mut history: Vec<ModelMessage> = Vec::new();
    let mut turn_tokens: Vec<usize> = Vec::new();
    let mut tool_token_turns: Vec<usize> = Vec::new();

    let build_turn = |history: &[ModelMessage]| -> (ModelRequest, Vec<ToolSpec>) {
        let specs = mode_visible_tool_specs(full_specs.clone(), &scenario.mode, 0, true);
        let built = builder
            .build(model.clone(), &scenario.task, &[], history, specs.clone())
            .expect("eval context build");
        (built.request, specs)
    };

    for (ix, call) in calls.iter().enumerate() {
        let (req, specs) = build_turn(&history);
        turn_tokens.push(estimate_request_tokens(&req));
        tool_token_turns.push(estimate_tool_tokens(&specs));
        let id = format!("call-{ix}");
        history.push(assistant_tool_call_msg(call, &id));
        history.push(tool_result_msg(call, &id));
    }

    // Final answer turn (model produces text, no tool call).
    {
        let (req, specs) = build_turn(&history);
        turn_tokens.push(estimate_request_tokens(&req));
        tool_token_turns.push(estimate_tool_tokens(&specs));
    }

    let tokens_per_task: usize = turn_tokens.iter().sum();
    let avg_tool_tokens_per_turn = if tool_token_turns.is_empty() {
        0
    } else {
        tool_token_turns.iter().sum::<usize>() / tool_token_turns.len()
    };

    let optimal_intents = scenario
        .intents
        .iter()
        .zip(intent_optimality(scenario, registry))
        .filter(|(_, opt)| *opt)
        .count();
    let tool_selection_accuracy = if scenario.intents.is_empty() {
        1.0
    } else {
        optimal_intents as f64 / scenario.intents.len() as f64
    };

    let f1s: Vec<f64> = calls.iter().map(|c| argument_f1(c, registry)).collect();
    let argument_f1 = if f1s.is_empty() {
        1.0
    } else {
        f1s.iter().sum::<f64>() / f1s.len() as f64
    };

    let errors = calls.iter().filter(|c| c.is_error).count();
    let recoveries = calls.iter().filter(|c| c.is_recovery).count();
    let recovery_rate = if errors == 0 {
        1.0
    } else {
        recoveries as f64 / errors as f64
    };

    ScenarioReport {
        name: scenario.name.to_string(),
        mode: format!("{:?}", scenario.mode),
        tokens_per_task,
        avg_tool_tokens_per_turn,
        trajectory_length: calls.len(),
        tool_selection_accuracy,
        argument_f1,
        recovery_rate,
        turns: turn_tokens.len(),
    }
}

fn intent_optimality(scenario: &Scenario, registry: &ToolRegistry) -> Vec<bool> {
    scenario
        .intents
        .iter()
        .map(|intent| match intent {
            Intent::Call { .. } | Intent::ErrorRecover { .. } => true,
            Intent::Edit { .. } => registry.get("edit_file").is_some(),
            Intent::Write { .. } => registry.get("write_file").is_some(),
        })
        .collect()
}

pub fn run_eval(label: &str) -> EvalReport {
    let registry = ToolRegistry::new(
        PathBuf::from("/tmp/vortex-eval-ckpt"),
        PathBuf::from("/tmp/nope.ts"),
    );
    let builder = ContextBuilder::default();
    let scenarios = default_scenarios();
    let reports: Vec<ScenarioReport> = scenarios
        .iter()
        .map(|s| run_scenario(s, &registry, &builder))
        .collect();

    let total_tokens_per_task = reports.iter().map(|r| r.tokens_per_task).sum();
    let n = reports.len().max(1) as f64;
    EvalReport {
        label: label.to_string(),
        total_tokens_per_task,
        mean_trajectory_length: reports.iter().map(|r| r.trajectory_length).sum::<usize>() as f64
            / n,
        mean_tool_selection_accuracy: reports
            .iter()
            .map(|r| r.tool_selection_accuracy)
            .sum::<f64>()
            / n,
        mean_argument_f1: reports.iter().map(|r| r.argument_f1).sum::<f64>() / n,
        mean_recovery_rate: reports.iter().map(|r| r.recovery_rate).sum::<f64>() / n,
        scenarios: reports,
    }
}
