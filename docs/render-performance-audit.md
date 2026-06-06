# Rendering Performance Audit

## Profiling

Set `VORTEX_RENDER_PROFILE=1` in a debug build to log render samples once per second through `tracing::debug`.

Tracked areas:

- `AgentWindow::render`
- `inspector_content`
- `ThreadView::render`
- `ThreadView::render_visible`
- markdown parse cache hits and misses
- markdown block rendering

The profiler is compiled as a no-op outside debug builds.

## Scroll Surface Inventory

Use `gpui_component::v_virtual_list` for variable-height transcript-style rows:

- main chat thread (`ThreadView`)
- embedded subagent inspector transcript (`ThreadView::new_embedded`)
- sidebar tree (`SidebarView`)

Use `uniform_list` only when rows are fixed-height:

- diff hunk rows
- dropdown/menu catalogs

Plain scroll columns should remain bounded forms or small detail panels:

- settings sections
- terminal artifact text
- context inspector groups

## Current Hot Paths

- Subagent inspector now renders through an embedded `ThreadView` entity. The old path cloned full thread items, filtered related rows, parsed markdown, and rendered every child row in a normal scroll column on each inspector render.
- Assistant deltas append directly into `ThreadView` and schedule a frame-batched row patch.
- Reasoning deltas and tool output deltas now use append-style bridge methods instead of generic full-item mutation for streaming updates.

## Follow-Up Targets

- Batch SQLite delta persistence in `agent_core::sink`.
- Move sealed markdown parsing to an idle/background parse queue.
- Add block-level markdown height caching keyed by width bucket.
- Consider pruning inactive `subagent_transcripts` when tabs close or conversations change.
