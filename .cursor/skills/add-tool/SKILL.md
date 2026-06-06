---
name: add-tool
description: Scaffold a new Vortex agent tool module end-to-end. Use when adding a tool to agent_tools or when the user asks how to register a new tool.
---

# Add a Vortex tool

Read `crates/agent_tools/AGENTS.md` first.

## Steps

1. **Create module** at `crates/agent_tools/src/tools/<snake_name>/mod.rs`:
   - Struct (unit struct or with deps like `checkpoint_dir`)
   - `AgentTool` impl: `name`, `description`, `schema`, `assess`, `execute`
   - Override `icon`, `label`, `row_label`, `args_preview`, `finish_summary` as needed

2. **Export** in `crates/agent_tools/src/tools/mod.rs`:
   ```rust
   pub mod my_tool;
   pub use my_tool::MyTool;
   ```

3. **Register** one line in `ToolRegistry::new()` inside `crates/agent_tools/src/registry.rs`:
   ```rust
   Box::new(MyTool),
   ```

4. **Risk in assess()** — set `ToolAssessment` fields; use `shared/risk.rs` for command tools. Set `denied: true` for blocked operations.

5. **Verify**: `cargo check -p app`

## Do not

- Add `match` arms in registry, sandbox, runtime, or UI for the new tool name
- Put GPUI types in agent_tools
- Skip `denied` / `requires_approval` in assess()

## Template

```rust
use agent_protocol::{IconToken, NetworkAccess, RiskLevel, ToolAssessment, ToolContext, ToolResult};
use async_trait::async_trait;
use serde_json::{Value, json};
use crate::tool::AgentTool;

pub struct MyTool;

#[async_trait]
impl AgentTool for MyTool {
    fn name(&self) -> &'static str { "my_tool" }
    fn description(&self) -> &'static str { "…" }
    fn schema(&self) -> Value { json!({ "type": "object", "properties": {} }) }
    fn icon(&self) -> IconToken { IconToken::Terminal }

    async fn assess(&self, _args: &Value, ctx: &ToolContext) -> Result<ToolAssessment, String> {
        Ok(ToolAssessment {
            risk: RiskLevel::SafeRead,
            requires_approval: false,
            reason: "…".into(),
            affected_paths: vec![ctx.project_root.clone()],
            network_access: NetworkAccess::Disabled,
            writes_to_disk: false,
            runs_real_process: false,
            denied: false,
        })
    }

    async fn execute(&self, _args: Value, ctx: ToolContext) -> Result<ToolResult, String> {
        Ok(ToolResult {
            call_id: agent_protocol::ToolCallId::new(""),
            name: self.name().to_string(),
            output: "done".into(),
            is_error: false,
        })
    }
}
```
