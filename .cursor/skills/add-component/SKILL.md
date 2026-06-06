---
name: add-component
description: Add or reuse a Vortex UI component without duplicating row markup. Use when building thread, sidebar, or drawer UI.
---

# Add a Vortex UI component

Read `crates/app/src/ui/components/AGENTS.md` and root `AGENTS.md` (flat-row rules).

## Before writing new markup

Search `crates/app/src/ui/components/` for an existing primitive:

- List/tree rows → `tree_row`, `collapsible_row::timeline_row`
- Tool activity → `tool_call`, `activity_step`, `step_icon`
- Buttons → `buttons.rs`

Reuse and extend rather than copy `div()` chains.

## Steps

1. Add `components/my_widget.rs` with a pure function:
   ```rust
   pub fn my_widget(label: &str) -> impl IntoElement { … }
   ```
2. Use only `Tokens::spacing_*`, `ROW_HEIGHT_*`, `Tokens::text_*`, `Tokens::surface_hover()` etc.
3. Export from `components/mod.rs`
4. Call from a **layout** (`layouts/`), not from `AgentWindow` render directly
5. `cargo check -p app`

## Flat row pattern

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
```

No persistent `.bg(Tokens::surface())` + border on read-only rows.

## Tool rows

Use `render_tool_header_row(item_id, &display_label, command, …)` — compute label via `window.tool_row_label()`, not local `match tool_name`.
