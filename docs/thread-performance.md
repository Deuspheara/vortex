# Thread performance

The thread view must stay responsive while assistant output streams into long
conversations. Streaming cost should scale with visible rows plus the latest
delta, not with total transcript length.

## Current architecture

| Area | Path | Responsibility |
| --- | --- | --- |
| Thread view entity | `crates/app/src/features/chat/thread_view/` | Virtual list, streaming patch queue, row rendering, scroll state |
| Manifest | `crates/app/src/features/chat/manifest.rs` | Stable row projections and height estimates |
| Markdown renderer | `crates/app/src/shared/components/markdown_preview.rs` | Markdown parsing and rendering caches |
| Streaming markdown | `crates/app/src/shared/components/streaming_markdown.rs` | Sealed blocks, live tail rendering, static cursor |
| Window bridge | `crates/app/src/window/bridge/thread_bridge.rs` | Syncs conversation state into `ThreadView` |
| Agent reducer | `crates/app/src/agent/reducer.rs` | Routes deltas, coalesces updates, skips full-window repaint for streaming |
| Render profiler | `crates/app/src/shared/render_profile.rs` | Debug-only timing instrumentation |

Stable history renders through a virtual list. The active assistant tail is
patched separately while streaming so token updates avoid rebuilding or
remeasuring the whole transcript.

## Streaming rules

- Append assistant deltas into the existing row instead of cloning the full
  thread item list.
- Coalesce token bursts to frame-aligned updates.
- Render unstable live text cheaply; reserve full markdown work for sealed or
  finished blocks.
- Keep the streaming cursor static. Do not drive repaint loops with decorative
  animation.
- Grow row height only when the estimate meaningfully changes.
- Skip `AgentWindow` notification for pure streaming deltas when `ThreadView`
  can patch itself directly.

## Profiling

Set `VORTEX_RENDER_PROFILE=1` in a debug build to log render samples through
`tracing::debug`.

Useful spans:

- `AgentWindow::render`
- `build_view_state`
- `inspector_content`
- `ThreadView::render`
- `ThreadView::render_visible`
- `ThreadView::schedule_item_update`
- `patch_streaming_assistant_row`
- `markdown_parse`
- `markdown_parse_cache_hit`
- `markdown_render_blocks`

## Anti-patterns

- Rebuilding `Conversation.thread_items` for every token.
- Re-parsing full markdown on every token.
- Reintroducing animated cursors or repaint loops in streaming rows.
- Moving the hot streaming tail back into the cold virtual list path.
- Updating row sizes for unchanged rows.
- Rendering syntax highlighting for open or incomplete code fences.

## Follow-up targets

- Batch SQLite delta persistence in `agent_core`.
- Move sealed markdown parsing to an idle or background queue where possible.
- Cache block heights by width bucket.
- Apply append-style delta handling consistently to reasoning and tool output.
- Add a soak test for large transcripts and high delta rates.

## Definition of done

For thread performance work, verify:

1. `cargo check -p app` passes.
2. Long transcripts stream at the same apparent latency as short transcripts.
3. Streaming while scrolled up does not force the viewport to jump.
4. Render profiling does not show full-window or full-thread work per token.
