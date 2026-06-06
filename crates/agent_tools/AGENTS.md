# agent_tools — Tool Module Guide

Each tool is a self-contained module. The registry is data-driven; do not add tool-name `match` arms elsewhere.

**Context contract:** discovery tools (`list_files`, `search_project`) before slice reads (`read_file` with line range). See [`docs/context-and-tool-contract.md`](../../docs/context-and-tool-contract.md).

**Edit contract:** batch edits per file; preview via `edit_file`/`propose_patch`, apply once via `apply_patch`. Target: `apply_edit_transaction` — see [`docs/edit-transaction-contract.md`](../../docs/edit-transaction-contract.md).

## Layout

```
src/
├── tool.rs              # AgentTool trait (schema, assess, execute, presentation)
├── registry.rs          # Tool list + catalog/dispatch only
├── shared/              # Engines reused by multiple tools
│   ├── patch_engine.rs
│   ├── git.rs
│   ├── risk.rs
│   └── browser_sidecar.rs
└── tools/
    ├── read_file/mod.rs
    ├── list_files/mod.rs
    └── …
```

## AgentTool contract

Implement all required methods on a unit struct or small struct with dependencies:

| Method | Purpose |
|--------|---------|
| `name` / `description` / `schema` | Model-facing tool spec |
| `assess` | Returns `ToolAssessment` with risk, `requires_approval`, `denied`, affected paths |
| `execute` | Runs the tool; returns `ToolResult` |
| `icon` | `IconToken` for UI |
| `label(running)` | Short status label |
| `row_label(command, running)` | Thread row label with args preview folded in |
| `args_preview` | One-line args summary for events |
| `finish_summary` | Post-run summary line |

Defaults exist in `tool.rs` for presentation methods; override when tool-specific.

## Risk rules

- Read-only tools: `RiskLevel::SafeRead`, `requires_approval: false`
- `bash_virtual`: wraps `agent_shell`; it must not spawn host processes
- `run_real_command`: use `shared/risk.rs` classifiers in `assess()`
- Mode gating (`can_run_real_commands`, `can_apply_patches`): check `ctx.mode` in `assess()`, set `denied: true`
- Never duplicate risk logic in `agent_sandbox` or UI

## Adding a tool (checklist)

1. Create `src/tools/my_tool/mod.rs` with struct + `AgentTool` impl
2. Export in `src/tools/mod.rs`
3. Add `Box::new(MyTool { … })` to `ToolRegistry::new()` in `registry.rs`
4. Run `cargo check -p app`

No changes to runtime, sandbox, or UI unless the tool needs special runtime hooks (e.g. `delegate`).

## Special cases

- **delegate**: registered for schema/catalog; execution stays in `agent_core` (`execute_delegate_tool`)
- **propose_patch** / **apply_patch**: thin wrappers around `shared/patch_engine.rs`
