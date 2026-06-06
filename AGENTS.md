# Vortex - Agent Guide

Rust desktop agent UI built on GPUI and `gpui-component`. This document is the
source of truth for agent work in this repository.

## Commands

```bash
cargo run -p app          # run the desktop app
cargo check -p app        # fast compile check
cargo build -p app        # build the desktop app
cargo test --workspace    # run Rust tests
```

Before opening a PR, prefer:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo check -p app
```

## Repository rules

- Keep credentials out of the repo. Real provider keys must come from the
  process environment or OS-level secret storage.
- Do not commit generated outputs: `target/`, `node_modules/`, logs, traces,
  local SQLite data, or local runtime directories.
- Keep `Cargo.lock` tracked. This is an application workspace.
- Keep public-facing documentation accurate before publishing branches.
- Prefer small, reviewable changes with verification commands in the PR body.

## Architecture crates

| Crate | Role |
| --- | --- |
| `agent_protocol` | Shared `AgentCommand`, `AgentEvent`, tool, risk, patch, status, and cancellation types |
| `agent_store` | SQLite event log, sessions, projects, and replayable state |
| `agent_core` | Runtime state machine, run loop, model context, tool execution, and cancellation |
| `agent_models` | Provider adapters, currently Mock and OpenRouter |
| `agent_tools` | Tool registry, schemas, assessment, execution, and presentation metadata |
| `agent_sandbox` | Command risk, approval, path, and secret policies |
| `agent_context` | Context builder and live system prompt |
| `agent_shell` | Rust-native fake shell over virtual/workspace filesystems |
| `real_process` | Approved real command executor |
| `project_index` | File scanning, search, summaries, and index cache |
| `context_providers` | Context provider service and adapters |
| `terminal` | Terminal core, PTY IO, rendering, and tests |
| `android_device` | Android CLI, emulator, observation, and action helpers |
| `app` | GPUI desktop shell and UI feature modules |

Runtime contracts:

- [Context and tool contract](docs/context-and-tool-contract.md)
- [Edit transaction contract](docs/edit-transaction-contract.md)
- Live agent prompt: [crates/agent_context/src/prompt.rs](crates/agent_context/src/prompt.rs)

## Runtime safety rules

These rules are enforced across `agent_core`, `agent_tools`, and
`agent_sandbox`:

1. The model never writes directly to disk.
2. The model never executes real shell commands directly.
3. The UI never renders provider-native events directly; it renders
   `AgentEvent`.
4. Every external provider stream is normalized into `AgentEvent`.
5. Every tool call is validated before execution.
6. Every write is represented as a patch first.
7. Every patch is previewable before apply.
8. Every dangerous command requires approval.
9. Network is disabled by default.
10. Secrets are never inserted into prompts or sandbox files.
11. Sessions are event-sourced and replayable from SQLite in
    `~/.config/vortex/vortex.db`.
12. Cancellation must work at run, tool, and process level.
13. A crash must not leave unknown file changes.
14. The user can always see what ran, why it ran, and what changed.

Default agent safety mode is `ApplyWithApproval`. Do not default to full
access.

## UI architecture

Current app UI code is organized under `crates/app/src/features`,
`crates/app/src/shared`, `crates/app/src/tokens`, `crates/app/src/window`, and a
small compatibility module in `crates/app/src/ui`.

| Layer | Path | Responsibility |
| --- | --- | --- |
| Tokens | `crates/app/src/tokens/` | Colors, spacing, radii, sizes, fonts, motion, icons |
| Feature state | `crates/app/src/features/*/state.rs` | Plain feature data and projections; no GPUI rendering |
| Shared state | `crates/app/src/shared/state/` | Reusable catalogs, transcript mode, readiness summaries |
| Shared components | `crates/app/src/shared/components/` | Stateless reusable leaf widgets |
| Feature components | `crates/app/src/features/*/components/` | Feature-specific stateless widgets |
| Feature layouts | `crates/app/src/features/*/layout.rs` | Region composition and feature layout |
| Window orchestration | `crates/app/src/window/` | `AgentWindow`, bridge modules, orchestration, and top-level render |
| UI compatibility | `crates/app/src/ui/` | Re-exports and thread update action/effect types |

### Separation rules

- Components and layouts should stay stateless unless GPUI requires local view
  state for an owned widget.
- Mutating application state belongs in `AgentWindow` and window orchestration
  modules.
- Layouts call back into `Entity<AgentWindow>` or feature entities; components
  should accept data and callbacks.
- Plain data stays in feature/shared `state` modules. Keep GPUI imports out of
  pure state.
- All visual values come from `Tokens`. Avoid inline spacing, radius, and row
  height numbers when a token exists.
- If two places render the same list row pattern, extract or reuse a shared row
  component.

## Visual design - flat first, cards only for input

The app should feel like a modern IDE sidebar plus document thread, not a
dashboard of boxes.

Use card treatment (`surface` background, border, radius, shadow) for elements
that accept or block user input:

| Card? | Examples |
| --- | --- |
| Yes | Composer, search field, text inputs, approval prompt, modal, popover |
| No | Sidebar rows, project/session tree nodes, assistant messages, tool-call rows, reasoning steps, diff summaries, drawer list items |

### Default row styling

For navigational and informational rows, prefer flat rows:

```rust
div()
    .h(px(Tokens::ROW_HEIGHT_MD))
    .px(Tokens::spacing_2())
    .flex()
    .items_center()
    .gap(Tokens::spacing_2())
    .rounded(Tokens::radius_xs())
    .cursor_pointer()
    .hover(|s| s.bg(Tokens::surface_hover()))
    .when(is_selected, |s| s.bg(Tokens::surface_active()))
```

Avoid persistent `surface` background plus border plus large radius on
read-only rows.

### Thread surface

- User messages may use an accent bubble.
- Assistant messages should be flat text on `main_bg`.
- Tool calls, reasoning, and diffs should be collapsible flat rows. Use borders
  for expanded detail areas only.
- Approval requests remain cards because they block action.
- The composer remains the primary elevated input surface.

## Spacing system

Use the 4 px base scale from `Tokens::spacing_*`.

| Context | Token | Value |
| --- | --- | --- |
| Tight stack | `spacing_0p5` | 2 px |
| Related items | `spacing_1` | 4 px |
| Icon-to-label gap | `spacing_2` | 8 px |
| Section inner padding | `spacing_2` | 8 px |
| Major sidebar sections | `spacing_5` or `spacing_6` | 20-24 px |
| Thread outer padding | `spacing_6` | 24 px |
| Thread item gap | `spacing_3` | 12 px |
| Composer outer padding | `spacing_5` or `spacing_6` | 20-24 px |

Concrete UI rules:

- Row heights should use `ROW_HEIGHT_SM`, `ROW_HEIGHT_MD`, or
  `ROW_HEIGHT_LG`.
- Sidebar scroll areas should have one horizontal inset; rows align to that
  inset.
- Tree indent should use `Tokens::tree_indent(...)` or the equivalent
  `spacing_3` per nesting level.
- Section labels are uppercase, `text_xs`, `text_tertiary`, and semibold.
- Selection is a row background, not a card outline.

## Session and project tree

Relevant files:

- `crates/app/src/features/shell/state.rs`
- `crates/app/src/features/shell/layout.rs`
- `crates/app/src/features/shell/components/tree_row.rs`
- `crates/app/src/features/shell/components/sidebar_row_menu.rs`

Rules:

1. Use one shared session row renderer for top-level and nested conversations.
2. Projects are disclosure rows with chevron, folder icon, name, and count.
3. Expanded state is tracked by stable project/folder keys.
4. Do not render the same conversation in two sections.
5. Stable element ids should be derived from `ProjectId` or `ConversationId`
   without `Box::leak`.

## Thread and activity UI

Relevant files:

- `crates/app/src/features/chat/manifest.rs`
- `crates/app/src/features/chat/thread_view/mod.rs`
- `crates/app/src/features/chat/thread_view/sync.rs`
- `crates/app/src/features/chat/thread_view/rows.rs`
- `crates/app/src/features/chat/thread_view/render.rs`
- `crates/app/src/features/chat/thread_view/caches.rs`
- `crates/app/src/features/chat/thread_view/scroll.rs`
- `crates/app/src/features/agent_activity/components/`

Hot tail vs cold list:

- While the last assistant message is streaming, render it in the hot-tail layer
  so token updates avoid relayouting the full virtual list.
- Stable rows use the virtual list with precomputed row references and heights
  from `features/chat/manifest.rs`.

When adding a row variant:

1. Add the `ThreadRow`/manifest projection.
2. Add height behavior in the manifest.
3. Add a renderer branch in `thread_view/rows.rs`.
4. Pass row actions through callbacks. Do not put `Entity<AgentWindow>` inside
   low-level components unless no GPUI alternative exists.

## Tool architecture

Adding a tool should be one folder under `crates/agent_tools/src/tools/<name>/`
plus one registry entry in `crates/agent_tools/src/registry.rs`.

Each tool module owns:

- schema
- `assess()` risk and approval logic
- `execute()`
- presentation metadata: `icon`, `label`, `row_label`, `args_preview`,
  `finish_summary`

Shared engines live in `crates/agent_tools/src/shared/`.

Per-directory guide:

- [crates/agent_tools/AGENTS.md](crates/agent_tools/AGENTS.md)

Local editor skill files under `.cursor/` are intentionally ignored and should
not be required for repository work.

## Public release checklist

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets`
- [ ] `cargo check -p app`
- [ ] `cargo test --workspace`
- [ ] Secret scan current tracked files and git history
- [ ] No local runtime data, generated outputs, or credentials in `git status`
- [ ] README setup instructions match the current app behavior
- [ ] UI changes follow flat-row/token rules
