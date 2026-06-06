# Vortex — Streaming Thread Performance Handoff

Use this document as the **system prompt / task brief** for a fresh agent session. Goal: **zero perceptible lag** during assistant streaming on long threads — smooth at native refresh rate (120 Hz on ProMotion), with cost **O(visible + delta)**, never O(total thread length) per token.

---

## Product goal

The streaming thread must feel **instant and smooth**:

- **Native refresh rate** where the compositor can repaint, with **at most one UI commit per frame** for streaming tail updates.
- **No Long Tasks > 50 ms** on the UI thread during stream, scroll, or typing.
- **Cost scales with O(visible + delta)**, never O(total thread length) or O(full message length) per frame.
- Collapsing sidebar/diff must be irrelevant — the main thread view alone must not lag.

Mental model:

```
tokens → delta buffer → batch per frame (~8ms @120Hz) → patch hot tail only
      → sealed blocks (cached markdown) + live tail (plain text)
      → cold virtual list (history) + hot tail surface (streaming row)
      → paint
```

**Critical GPUI fact:** the root view can repaint every frame when animated or notified. Do **not** drive full-thread repaints via animations. Drive **frame-aligned content updates** with **minimal element rebuild**.

**Critical markdown fact:** do **not** render full markdown in real time for every token. Render the stream as cheap text while unstable, then progressively **seal** stable markdown blocks and render those as cached markdown.

---

## Stack (Vortex)

| Layer | Path | Notes |
|-------|------|-------|
| UI window | `crates/app/src/ui/agent_window.rs` | Owns state; must NOT `cx.notify()` on streaming deltas |
| Thread surface | `crates/app/src/ui/layouts/thread_view.rs` | `ThreadView` entity + cold `v_virtual_list` + hot tail |
| Streaming markdown | `crates/app/src/ui/components/streaming_markdown.rs` | Sealed blocks + live tail renderer |
| Row manifest | `crates/app/src/ui/state/thread_manifest.rs` | Flattened `RowRef` + variable heights |
| Reducer | `crates/app/src/agent/reducer.rs` | `DeltaBuffer`, event routing |
| Event sink | `crates/agent_core/src/sink.rs` | Per-token SQLite persist (async task, not UI) |
| Markdown | `crates/app/src/ui/components/markdown_preview.rs` | Heavy — only for sealed/finished blocks |

Architecture rules: see repo root `AGENTS.md` and `crates/app/src/ui/AGENTS.md`.

---

## Target architecture — sealed blocks + live tail

Do not treat one assistant answer as one giant markdown string.

```rust
struct AssistantMessageVm {
    id: MessageId,
    sealed_blocks: Vec<MarkdownBlockVm>,  // finalized, cached
    live_tail: String,                     // unstable streamed text
    is_streaming: bool,
}

struct MarkdownBlockVm {
    id: BlockId,
    source: Arc<str>,
    parsed: Option<Arc<ParsedBlock>>,
    measured_height: Option<Pixels>,
    version: u64,
}
```

**Safe seal boundaries** (move text from `live_tail` → `sealed_blocks`):

- Blank line after paragraph (`\n\n`)
- Closed code fence (` ``` `)
- Completed list/table followed by blank line
- End of tool call output / assistant turn

**During streaming, render:**

```
[sealed markdown blocks — full parse + highlight]
[live tail — plain text OR open monospace code fence, no AST]
[static cursor ▍]
```

**After completion:** full markdown parse once; clear sealed/live split.

### Two renderers

| Mode | When | Rules |
|------|------|-------|
| `LivePlainText` | streaming tail | One text element, no markdown AST |
| `LiveCodeFence` | open ` ``` ` in tail | Monospace block, no syntax highlight |
| `FinalMarkdown` | sealed blocks / finished message | Full parse, highlight closed fences, tables |

Policy: `parse_links_only`-style degradation while streaming; enable syntax highlight / tables only on sealed or finished blocks.

### Parse vs layout

- **Background thread:** markdown source → block AST / code ranges (pure data, `Send`)
- **UI thread:** AST → GPUI elements / `TextLayout` / measured height (`TextLayout` is `!Send`)

Do not background-spawn GPUI elements.

### Cache by block

```rust
struct MarkdownCacheKey {
    block_hash: u64,
    theme_id: ThemeId,
    width_bucket: u16,      // (width / 16).round()
    font_size_bucket: u16,
    render_options: MarkdownRenderOptions,
}
```

Include width bucket — wrapping changes height on resize.

---

## Already implemented (do not redo)

Verify these still work, then build on them:

### Virtualization & layout
- [x] `gpui_component::v_virtual_list` with flattened `RowRef` manifest
- [x] Incremental `patch_item_rows` / `push_item_inner`
- [x] **`patch_streaming_assistant_row` fast path** (fallback when hot tail inactive)
- [x] **`mutate_row_sizes`** — in-place `Rc::make_mut`, no double `.to_vec()` clone
- [x] **Grow row height only when estimate increases by ≥ half line** (`STREAMING_HEIGHT_EPSILON`)
- [x] **Hot tail detached from virtual list** — streaming assistant row renders below cold list; token updates skip O(N) `row_sizes` relayout

### Sealed blocks + live tail
- [x] **`split_at_seal_boundary`** — paragraph breaks + closed fences outside fences
- [x] **`SealedBlockCache`** — incremental parse of sealed prefix only (blake3 hash)
- [x] **`render_live_tail`** — plain text or open code fence (monospace, no highlight)
- [x] **`streaming_assistant_body`** — sealed blocks + live tail + static cursor

### Event routing & batching
- [x] Channel batching in `AgentWindow` spawn loop (`try_recv` coalesce)
- [x] `DeltaBuffer` 16 ms flush in reducer (`state.rs`)
- [x] **`event_needs_window_notify`** skips `AgentWindow::cx.notify()` for deltas / usage
- [x] Live events skip `sync_thread_view` (no full `thread_items.clone()` during stream)
- [x] **Frame-aligned flush** — `STREAM_FRAME_SYNC_MS = 8` in `schedule_item_update`
- [x] **Adaptive backpressure** — 8 ms / 33 ms / 66 ms batch interval from patch cost

### Streaming tail hot path
- [x] **`append_assistant_delta`** — append chunk without cloning full `ThreadItem`
- [x] **`streaming_assistant_plain`** — fallback plain text when hot tail inactive
- [x] **`static_streaming_cursor`** — no braille animation repaints
- [x] **`thread_item_indices`** — O(1) lookup
- [x] Per-view **`MarkdownCacheEntry`** (blake3 hash) for finished messages
- [x] **Scroll-aware streaming** — `user_scrolled_up` shows "New output ↓" pill, accumulates silently

### MVU thread updates
- [x] `crates/app/src/ui/thread_update/{action,effect,mod}.rs`

---

## Known remaining bottlenecks (investigate first)

If lag persists, profile in this order:

### 1. `v_virtual_list` O(N) on any cold `row_sizes` change
**File:** `gpui-component` virtual_list.rs

Hot tail mitigates this for the streaming row. **Next:**
- Prefix-sum cache for cold list offsets; patch only tail range
- Or normalized store `{ ids, entities }` with tail pointer

### 2. Per-token SQLite persist (agent side backpressure)
**File:** `crates/agent_core/src/sink.rs`

Coalesce assistant/reasoning deltas (100–200 ms or N bytes) before INSERT.

### 3. Reasoning & tool streaming still use full `update_item` clone path
Apply same **`append_*_delta`** + plain render + patch fast path for `ReasoningStep` and `ToolCall` output.

### 4. Finished assistant rows in viewport re-render every notify
Cache rendered element subtrees or skip rebuild when `content_hash` unchanged.

### 5. Background parse queue for sealed blocks
Today sealed blocks parse on UI thread at seal time. **Next:** `cx.background_spawn` parse → send `Arc<[MarkdownBlock]>` to UI on idle (80 ms debounce).

### 6. Lazy syntax highlight for sealed code blocks
Highlight only closed fences, only visible blocks, only when idle.

### 7. Dual store copies
**Files:** `Conversation.thread_items` + `ThreadView.items`

Long-term: single source of truth or `Arc<str>` for markdown bodies.

### 8. Tables while streaming
Render tables as monospace plaintext in live tail; parse table only after blank line / message complete.

---

## Priority todo list (fresh session)

Execute in order. Verify with `cargo check -p app` after each phase.

### P0 — Done in latest session ✓

1. ~~Frame-aligned streaming (8 ms batch)~~
2. ~~Cold virtual list + hot tail surface~~
3. ~~Sealed blocks + live tail renderer~~
4. ~~Adaptive batch interval (backpressure)~~
5. ~~Scroll-aware "New output" pill~~

### P1 — Eliminate remaining O(n) work

1. **Background parse for sealed blocks** — idle queue, 80 ms debounce
2. **Extend `append_*_delta` to reasoning + tool output**
3. **Batch SQLite delta persistence** in `ChannelEventSink`
4. **Throttle status bar** (`UsageUpdated`) via separate entity, 500 ms refresh
5. **Skip notify when hot tail text changed but sealed/live split unchanged** (micro-opt)

### P2 — Scale & polish

1. **Normalized thread store** `{ ids, entities }` + tail pointer
2. **Cursor-based history pagination** (load older messages on scroll)
3. **Block-level height cache** with width bucket invalidation
4. **Inline runs flattening** — one `StyledText` per paragraph, not nested `AnyElement` spans
5. **Soak test harness** — 20k messages, 100 deltas/s, assert no RSS drift / long tasks

---

## Files to touch (cheat sheet)

```
crates/app/src/ui/layouts/thread_view.rs           # cold list + hot tail
crates/app/src/ui/components/streaming_markdown.rs # seal + live render
crates/app/src/ui/agent_window.rs                  # append_*, event loop
crates/app/src/agent/reducer.rs                    # DeltaBuffer, notify gating
crates/app/src/ui/state/state.rs                   # DeltaBuffer
crates/app/src/ui/state/thread_manifest.rs         # row heights, RowRef
crates/app/src/ui/components/message.rs            # finished assistant
crates/app/src/ui/components/markdown_preview.rs   # sealed/finished parse
crates/agent_core/src/sink.rs                      # batched persist
```

---

## Anti-patterns (never reintroduce)

- Full `thread_items.clone()` on streaming deltas
- `Rc::make_mut(&row_sizes).to_vec()` — full vector clone per patch
- Markdown `parse_markdown_blocks_*` on every token or every animation frame
- Braille / repeating GPUI animations on the streaming row
- `AgentWindow::cx.notify()` for `AssistantTextDelta` / `UsageUpdated`
- String equality on full markdown for cache lookup (use blake3 hash)
- Re-parsing entire message for height when line-count estimate suffices
- Putting the streaming assistant row back inside `v_virtual_list` (reintroduces O(N) relayout)
- Syntax highlight on open/incomplete code fences

---

## Success criteria (definition of done)

| Test | Pass condition |
|------|----------------|
| Stream 60 s @ 50+ tokens/s | No fan spike; UI thread Long Tasks = 0 |
| Thread with 5k+ flattened rows | Stream tail latency same as empty thread |
| Scroll up during stream | "New output ↓" pill; no forced scroll jump; 60 fps scroll |
| Sidebar/diff collapsed vs open | **No measurable difference** in stream FPS |
| 120 Hz display | Visual updates continuous, not 10 Hz stair-step |
| Memory soak 30 min | RSS plateau, no unbounded growth |
| Long plan/markdown reply | Sealed paragraphs render as markdown while tail streams plain |

---

## Prompt to paste in new Cursor chat

```
You are optimizing Vortex (Rust GPUI desktop agent UI) for streaming assistant threads.

Read first:
- /AGENTS.md
- /docs/streaming-thread-performance-handoff.md

Goal: native refresh-rate smooth streaming. Cost must be O(visible + delta).

Done: virtual list, hot tail outside v_virtual_list, sealed blocks + live tail,
append_assistant_delta, frame batching (8ms), adaptive backpressure, scroll pill,
plain/hybrid streaming render, static cursor, skip AgentWindow notify on deltas.

Your job: P1 items — background parse for sealed blocks, append_*_delta for
reasoning/tools, batched SQLite persist, finished-row render cache.

Do NOT redo completed items. Profile before large refactors.
Run `cargo check -p app` after changes.

Start by reading thread_view.rs and streaming_markdown.rs.
```

---

## Reference — recommended rendering pipeline

```
incoming token
  ↓
append to String buffer (append_assistant_delta)
  ↓
batch 8–66 ms (adaptive)
  ↓
update only active message / hot tail
  ↓
split complete markdown blocks (split_at_seal_boundary)
  ↓
live tail = plain text / open code fence
  ↓
sealed blocks → cached markdown parse (UI thread today; background P1)
  ↓
measure height once (hot tail auto-height; cold list untouched)
  ↓
paint (cold virtual list + hot tail div)
```

Cost target:

```
tokens × full_message_markdown_parse × full_thread_render   ← BAD

batches × live_tail_text_render
+ sealed_blocks × parse_once
+ visible_cold_rows × cached_render                        ← GOOD
```

---

*Last updated: hot tail split, sealed blocks + live tail, adaptive batching, scroll pill.*
