//! Thread sync and state machine

use super::*;

impl ThreadView {
    pub(crate) fn set_conversation_inner(
        &mut self,
        conversation_id: ConversationId,
        items: Vec<ThreadItem>,
        cx: &mut Context<Self>,
    ) -> Vec<ThreadEffect> {
        self.conversation_id = Some(conversation_id.clone());
        self.stick_to_bottom = true;
        self.pending_scroll_bottom = true;
        self.pending_items = None;
        self.pending_item_updates.clear();
        self.pending_streaming_row_patches.clear();
        self.pending_mutated_item_patches.clear();
        self.last_stream_sync = None;
        self.markdown_cache.clear();
        self.tool_output_cache.clear();
        self.item_index.clear();
        self.header_manifest_index.clear();
        self.user_scrolled_up = false;
        self.last_scroll_offset = None;
        self.sealed_blocks.clear();
        self.pending_sealed_parses.clear();
        self.sync_inner(conversation_id, items, cx, true, false)
    }

    pub(crate) fn push_item_inner(&mut self, item: ThreadItem) -> Vec<ThreadEffect> {
        let item_ix = self.items.len() as u32;
        self.item_index
            .insert(item.id().to_string(), item_ix as usize);
        self.items.push(item);
        let new_refs = row_refs_for_item_with_mode(
            item_ix,
            self.items.last().expect("just pushed"),
            self.transcript_mode,
            &self.items,
        );
        let new_len = new_refs.len();

        let insert_at = self.manifest.len().saturating_sub(1);
        let prev_before = self.manifest.get(insert_at.wrapping_sub(1)).copied();
        let new_sizes = row_sizes_for_rows(&new_refs, &self.items, prev_before);
        let header_ix = new_refs
            .iter()
            .position(|row| row.is_header() && row.item_ix() == Some(item_ix))
            .map(|offset| insert_at + offset);

        self.manifest.splice(insert_at..insert_at, new_refs);
        self.set_header_manifest_ix(item_ix as usize, header_ix);

        let size_insert = self.row_sizes.len().saturating_sub(1);
        self.mutate_row_sizes(|sizes| {
            sizes.splice(size_insert..size_insert, new_sizes);
        });
        self.ensure_end_spacer();
        self.refresh_end_spacer_size();
        self.refresh_row_size_at(size_insert + new_len);
        debug_assert_eq!(self.manifest.len(), self.row_sizes.len());
        let mut effects = vec![ThreadEffect::Notify];
        if self.stick_to_bottom && !self.user_scrolled_up {
            effects.push(ThreadEffect::ScrollToBottom);
        }
        effects
    }

    /// Insert manifest rows when an item exists in `items` but was omitted (e.g. transcript filter drift).
    pub(crate) fn ensure_manifest_rows_for_item(&mut self, item_ix: usize) -> bool {
        if self.header_ix_for_item(item_ix as u32).is_some() {
            return false;
        }
        let Some(item) = self.items.get(item_ix).cloned() else {
            return false;
        };
        let new_refs =
            row_refs_for_item_with_mode(item_ix as u32, &item, self.transcript_mode, &self.items);
        if new_refs.is_empty() {
            return false;
        }
        let new_len = new_refs.len();
        let insert_at = self.manifest.len().saturating_sub(1);
        let prev_before = self.manifest.get(insert_at.wrapping_sub(1)).copied();
        let new_sizes = row_sizes_for_rows(&new_refs, &self.items, prev_before);
        let header_ix = new_refs
            .iter()
            .position(|row| row.is_header() && row.item_ix() == Some(item_ix as u32))
            .map(|offset| insert_at + offset);

        self.manifest.splice(insert_at..insert_at, new_refs);
        self.set_header_manifest_ix(item_ix, header_ix);

        let size_insert = self.row_sizes.len().saturating_sub(1);
        self.mutate_row_sizes(|sizes| {
            sizes.splice(size_insert..size_insert, new_sizes);
        });
        self.ensure_end_spacer();
        self.refresh_row_size_at(size_insert + new_len);
        debug_assert_eq!(self.manifest.len(), self.row_sizes.len());
        true
    }

    pub(crate) fn mutate_row_sizes(
        &mut self,
        update: impl FnOnce(&mut Vec<gpui::Size<gpui::Pixels>>),
    ) {
        update(Rc::make_mut(&mut self.row_sizes));
    }

    pub(crate) fn rebuild_header_manifest_index(&mut self) {
        self.header_manifest_index = vec![None; self.items.len()];
        for (row_ix, row) in self.manifest.iter().copied().enumerate() {
            if row.is_header() {
                if let Some(item_ix) = row.item_ix().map(|ix| ix as usize) {
                    if item_ix < self.header_manifest_index.len() {
                        self.header_manifest_index[item_ix] = Some(row_ix);
                    }
                }
            }
        }
    }

    pub(crate) fn header_ix_for_item(&self, item_ix: u32) -> Option<usize> {
        self.header_manifest_index
            .get(item_ix as usize)
            .and_then(|ix| *ix)
    }

    pub(crate) fn set_header_manifest_ix(&mut self, item_ix: usize, row_ix: Option<usize>) {
        if self.header_manifest_index.len() <= item_ix {
            self.header_manifest_index.resize(item_ix + 1, None);
        }
        self.header_manifest_index[item_ix] = row_ix;
    }

    pub(crate) fn shift_header_manifest_indices_after(
        &mut self,
        item_ix: usize,
        start: usize,
        delta: isize,
    ) {
        if delta == 0 {
            return;
        }

        for (ix, header_ix) in self.header_manifest_index.iter_mut().enumerate() {
            if ix == item_ix {
                continue;
            }
            let Some(current) = *header_ix else {
                continue;
            };
            if current > start {
                *header_ix = Some(current.saturating_add_signed(delta));
            }
        }
    }

    pub(crate) fn replace_manifest_span_for_item(
        &mut self,
        item_ix: usize,
        start: usize,
        end: usize,
        new_refs: Vec<RowRef>,
        new_sizes: Vec<gpui::Size<gpui::Pixels>>,
    ) -> usize {
        let old_len = end.saturating_sub(start);
        let new_len = new_refs.len();
        let header_ix = new_refs
            .iter()
            .position(|row| row.is_header() && row.item_ix() == Some(item_ix as u32))
            .map(|offset| start + offset);

        self.manifest.splice(start..end, new_refs);
        self.mutate_row_sizes(|sizes| {
            sizes.splice(start..end, new_sizes);
        });
        self.set_header_manifest_ix(item_ix, header_ix);
        self.shift_header_manifest_indices_after(
            item_ix,
            start,
            new_len as isize - old_len as isize,
        );
        new_len
    }

    pub(crate) fn refresh_row_size_at(&mut self, ix: usize) {
        if ix >= self.manifest.len() {
            return;
        }
        let row = self.manifest[ix];
        let prev = self.manifest.get(ix.wrapping_sub(1)).copied();
        let next = row_size(row, prev, &self.items);
        self.mutate_row_sizes(|sizes| {
            if ix < sizes.len() {
                sizes[ix] = next;
            }
        });
    }

    pub(crate) fn row_height_for_patch(&mut self, row: RowRef, prev: Option<RowRef>) -> f32 {
        let _profile = crate::shared::render_profile::span("row_height_with_collapsed");
        if matches!(row, RowRef::EndSpacer) {
            return self.end_spacer_height();
        }
        if let RowRef::AssistantMessage { item_ix } = row {
            let assistant_data: Option<(String, String)> =
                self.items.get(item_ix as usize).and_then(|item| {
                    if let ThreadItem::AssistantMessage {
                        id,
                        markdown,
                        streaming: true,
                        ..
                    } = item
                    {
                        let sanitized = crate::agent::text::sanitize_assistant_text(markdown);
                        Some((id.clone(), sanitized))
                    } else {
                        None
                    }
                });
            if let Some((id, sanitized)) = assistant_data {
                let blocks = self.cached_markdown_blocks(&id, &sanitized, true);
                let gap = row_top_gap(row, prev, &self.items);
                return assistant_row_height_from_blocks(
                    item_ix as usize,
                    &self.items,
                    blocks.as_ref(),
                    true,
                    !sanitized.is_empty(),
                ) + gap;
            }
        }
        row_height_with_collapsed(row, prev, &self.items, &std::collections::HashSet::new())
    }

    pub(crate) fn row_sizes_for_patch(
        &mut self,
        rows: &[RowRef],
        prev_before_first: Option<RowRef>,
    ) -> Vec<gpui::Size<gpui::Pixels>> {
        let _profile = crate::shared::render_profile::span("row_sizes_for_patch");
        rows.iter()
            .enumerate()
            .map(|(i, &row)| {
                let prev = if i == 0 {
                    prev_before_first
                } else {
                    Some(rows[i - 1])
                };
                gpui::size(
                    gpui::px(Tokens::THREAD_MAX_WIDTH),
                    gpui::px(self.row_height_for_patch(row, prev)),
                )
            })
            .collect()
    }

    pub(crate) fn ensure_end_spacer(&mut self) {
        if self
            .manifest
            .last()
            .is_some_and(|row| matches!(row, RowRef::EndSpacer))
        {
            return;
        }
        self.manifest.push(RowRef::EndSpacer);
        let end_spacer = gpui::size(
            gpui::px(Tokens::THREAD_MAX_WIDTH),
            gpui::px(self.end_spacer_height()),
        );
        self.mutate_row_sizes(|sizes| {
            sizes.push(end_spacer);
        });
    }

    pub(crate) fn update_item_inner(
        &mut self,
        item: ThreadItem,
        cx: &mut Context<Self>,
    ) -> Vec<ThreadEffect> {
        let debounced = matches!(
            &item,
            ThreadItem::ToolCall {
                status: crate::features::shell::state::AgentStatus::RunningTool,
                ..
            } | ThreadItem::AssistantMessage {
                streaming: true,
                ..
            } | ThreadItem::ReasoningStep {
                status: crate::features::shell::state::AgentStatus::Thinking,
                ..
            }
        );
        if debounced {
            self.pending_item_updates
                .insert(item.id().to_string(), item);
            return vec![ThreadEffect::ScheduleItemUpdate];
        }
        self.apply_item_update(item, cx)
    }

    pub(crate) fn dispatch(&mut self, action: ThreadAction, cx: &mut Context<Self>) {
        let effects = self.update_thread_state(action, cx);
        self.apply_thread_effects(effects, cx);
    }

    pub(crate) fn update_thread_state(
        &mut self,
        action: ThreadAction,
        cx: &mut Context<Self>,
    ) -> Vec<ThreadEffect> {
        match action {
            ThreadAction::SetConversation {
                conversation_id,
                items,
            } => self.set_conversation_inner(conversation_id, items, cx),
            ThreadAction::Sync {
                conversation_id,
                items,
                immediate,
                run_active,
            } => self.sync_inner(conversation_id, items, cx, immediate, run_active),
            ThreadAction::PushItem(item) => self.push_item_inner(item),
            ThreadAction::UpdateItem(item) => self.update_item_inner(item, cx),
            ThreadAction::RefreshItem(item) => {
                self.pending_item_updates.remove(item.id());
                self.patch_item_rows(item, cx)
            }
            ThreadAction::SetApprovalActive(active) => {
                if self.approval_active == active {
                    Vec::new()
                } else {
                    self.approval_active = active;
                    vec![ThreadEffect::Notify]
                }
            }
        }
    }

    pub(crate) fn apply_thread_effects(
        &mut self,
        effects: Vec<ThreadEffect>,
        cx: &mut Context<Self>,
    ) {
        for effect in effects {
            self.apply_thread_effect_with_context(effect, cx);
        }
    }

    pub(crate) fn apply_thread_effect_with_context(
        &mut self,
        effect: ThreadEffect,
        cx: &mut Context<Self>,
    ) {
        match effect {
            ThreadEffect::Notify => cx.notify(),
            ThreadEffect::ScrollToBottom => self.apply_thread_effect(effect),
            ThreadEffect::ScheduleStreamSync { interval_ms } => {
                self.schedule_stream_sync(interval_ms, cx)
            }
            ThreadEffect::ScheduleItemUpdate => self.schedule_item_update(cx),
        }
    }

    pub(crate) fn apply_thread_effect(&mut self, effect: ThreadEffect) {
        if matches!(effect, ThreadEffect::ScrollToBottom) {
            self.stick_to_bottom = true;
            self.user_scrolled_up = false;
            self.pending_scroll_bottom = true;
        }
    }

    pub(crate) fn apply_item_update(
        &mut self,
        item: ThreadItem,
        cx: &mut Context<Self>,
    ) -> Vec<ThreadEffect> {
        let Some(&item_ix) = self.item_index.get(item.id()) else {
            return Vec::new();
        };
        if let Some(old) = self.items.get(item_ix) {
            if !collapsed_item_header_changed(&item, old) {
                return Vec::new();
            }
        }
        self.patch_item_rows(item, cx)
    }

    pub(crate) fn patch_effects(&self, height_changed: bool) -> Vec<ThreadEffect> {
        let mut effects = vec![ThreadEffect::Notify];
        if height_changed && self.stick_to_bottom && !self.user_scrolled_up {
            effects.push(ThreadEffect::ScrollToBottom);
        }
        effects
    }

    /// Hot path for streaming assistant ticks: update item + render cache, grow shell
    /// height only when needed so `v_virtual_list` skips O(n) offset recomputation.
    pub(crate) fn patch_streaming_assistant_row(
        &mut self,
        item_ix: usize,
        row_ix: usize,
    ) -> Vec<ThreadEffect> {
        let _profile = crate::shared::render_profile::span("patch_streaming_assistant_row");
        let row = RowRef::AssistantMessage {
            item_ix: item_ix as u32,
        };
        let prev = self.manifest.get(row_ix.wrapping_sub(1)).copied();
        let new_height = match self.items.get(item_ix) {
            Some(ThreadItem::AssistantMessage { id, markdown, .. }) => {
                let id = id.clone();
                let sanitized = crate::agent::text::sanitize_assistant_text(markdown);
                let gap = row_top_gap(row, prev, &self.items);
                self.flush_sealed_blocks_sanitized(&id, &sanitized);
                self.streaming_assistant_height_estimate_cached(&id, &sanitized, gap)
            }
            _ => return vec![ThreadEffect::Notify],
        };
        let current_height = self
            .row_sizes
            .get(row_ix)
            .map(|size| f32::from(size.height))
            .unwrap_or(0.0);

        let height_changed = (new_height - current_height).abs() > STREAMING_HEIGHT_EPSILON;
        if height_changed {
            let new_size = gpui::size(gpui::px(Tokens::THREAD_MAX_WIDTH), gpui::px(new_height));
            self.mutate_row_sizes(|sizes| {
                if row_ix < sizes.len() {
                    sizes[row_ix] = new_size;
                }
            });
        }

        self.patch_effects(height_changed)
    }

    pub(crate) fn patch_item_rows(
        &mut self,
        item: ThreadItem,
        _cx: &mut Context<Self>,
    ) -> Vec<ThreadEffect> {
        let item_id = item.id().to_string();
        let Some(&item_ix) = self.item_index.get(&item_id) else {
            return Vec::new();
        };
        let was_streaming = matches!(
            self.items.get(item_ix),
            Some(ThreadItem::AssistantMessage {
                streaming: true,
                ..
            })
        );
        self.items[item_ix] = item;
        if was_streaming {
            if let ThreadItem::AssistantMessage {
                id,
                streaming: false,
                ..
            } = &self.items[item_ix]
            {
                self.sealed_blocks.remove(id);
                self.pending_sealed_parses.remove(id);
                self.markdown_cache.remove(id);
            }
        }

        if matches!(
            self.items[item_ix],
            ThreadItem::AssistantMessage {
                streaming: true,
                ..
            }
        ) {
            if let Some(header_ix) = self.header_ix_for_item(item_ix as u32) {
                let (start, end) = manifest_span(&self.manifest, header_ix);
                if end - start == 1
                    && matches!(
                        self.manifest.get(start),
                        Some(RowRef::AssistantMessage {
                            item_ix: ix
                        }) if *ix == item_ix as u32
                    )
                {
                    return self.patch_streaming_assistant_row(item_ix, start);
                }
            }
        }

        let item_ref = &self.items[item_ix];
        let Some(header_ix) = self.header_ix_for_item(item_ix as u32) else {
            return vec![ThreadEffect::Notify];
        };

        let (start, end) = manifest_span(&self.manifest, header_ix);
        let new_refs = row_refs_for_item_with_mode(
            item_ix as u32,
            item_ref,
            self.transcript_mode,
            &self.items,
        );
        let prev_before = self.manifest.get(start.wrapping_sub(1)).copied();
        let new_sizes = self.row_sizes_for_patch(&new_refs, prev_before);

        let new_len = self.replace_manifest_span_for_item(item_ix, start, end, new_refs, new_sizes);
        self.refresh_row_size_at(start + new_len);
        debug_assert_eq!(self.manifest.len(), self.row_sizes.len());

        self.patch_effects(true)
    }

    pub(crate) fn patch_current_item_rows(&mut self, item_ix: usize) -> Vec<ThreadEffect> {
        let _profile = crate::shared::render_profile::span("patch_current_item_rows");
        let Some(item_ref) = self.items.get(item_ix) else {
            return Vec::new();
        };
        let Some(header_ix) = self.header_ix_for_item(item_ix as u32) else {
            return vec![ThreadEffect::Notify];
        };

        let (start, end) = manifest_span(&self.manifest, header_ix);
        let new_refs = row_refs_for_item_with_mode(
            item_ix as u32,
            item_ref,
            self.transcript_mode,
            &self.items,
        );
        let prev_before = self.manifest.get(start.wrapping_sub(1)).copied();
        let new_sizes = self.row_sizes_for_patch(&new_refs, prev_before);

        let new_len = self.replace_manifest_span_for_item(item_ix, start, end, new_refs, new_sizes);
        self.refresh_row_size_at(start + new_len);
        debug_assert_eq!(self.manifest.len(), self.row_sizes.len());

        self.patch_effects(true)
    }

    pub(crate) fn rebuild_all(&mut self, items: Vec<ThreadItem>) -> Vec<ThreadEffect> {
        self.items = items;
        self.tool_output_cache.clear();
        self.pending_sealed_parses.clear();
        self.item_index = self
            .items
            .iter()
            .enumerate()
            .map(|(ix, item)| (item.id().to_string(), ix))
            .collect();
        self.manifest = build_manifest_with_transcript(&self.items, self.transcript_mode);
        self.row_sizes = Rc::new(row_sizes_for_manifest(&self.manifest, &self.items));
        self.rebuild_header_manifest_index();
        self.refresh_end_spacer_size();
        vec![ThreadEffect::Notify]
    }

    pub(crate) fn try_incremental_sync(&mut self, items: &[ThreadItem]) -> bool {
        if items.len() < self.items.len() {
            return false;
        }
        for i in 0..self.items.len() {
            if self.items[i].id() != items[i].id() {
                return false;
            }
        }
        for i in 0..self.items.len() {
            if self.items[i] != items[i] {
                let item = items[i].clone();
                if let Some(&item_ix) = self.item_index.get(item.id()) {
                    self.items[item_ix] = item.clone();
                    if let Some(header_ix) = self.header_ix_for_item(item_ix as u32) {
                        let (start, end) = manifest_span(&self.manifest, header_ix);
                        let new_refs = row_refs_for_item_with_mode(
                            item_ix as u32,
                            &item,
                            self.transcript_mode,
                            &self.items,
                        );
                        let prev_before = self.manifest.get(start.wrapping_sub(1)).copied();
                        let new_sizes = self.row_sizes_for_patch(&new_refs, prev_before);
                        let new_len = self.replace_manifest_span_for_item(
                            item_ix, start, end, new_refs, new_sizes,
                        );
                        self.refresh_row_size_at(start + new_len);
                    }
                }
            }
        }
        for item in &items[self.items.len()..] {
            self.push_item_inner(item.clone());
        }
        true
    }

    pub(crate) fn sync_inner(
        &mut self,
        conversation_id: ConversationId,
        items: Vec<ThreadItem>,
        _cx: &mut Context<Self>,
        immediate: bool,
        run_active: bool,
    ) -> Vec<ThreadEffect> {
        self.conversation_id = Some(conversation_id);

        if immediate {
            self.pending_items = None;
            self.pending_item_updates.clear();
            self.pending_streaming_row_patches.clear();
            self.pending_mutated_item_patches.clear();
            self.last_stream_sync = None;
            self.stream_debounce_scheduled = false;
            return self.rebuild_all(items);
        }

        let item_streaming = items.iter().any(|item| match item {
            ThreadItem::AssistantMessage {
                streaming: true, ..
            } => true,
            ThreadItem::ToolCall {
                status: crate::features::shell::state::AgentStatus::RunningTool,
                ..
            } => true,
            ThreadItem::ReasoningStep {
                status: crate::features::shell::state::AgentStatus::Thinking,
                ..
            } => true,
            _ => false,
        });
        let streaming = run_active || item_streaming;

        if streaming {
            let interval_ms = if run_active {
                STREAM_FRAME_SYNC_MS
            } else if items.iter().any(|item| {
                matches!(
                    item,
                    ThreadItem::ToolCall {
                        status: crate::features::shell::state::AgentStatus::RunningTool,
                        ..
                    }
                )
            }) {
                STREAM_FRAME_SYNC_MS
            } else {
                ASSISTANT_STREAM_SYNC_MS
            };
            let defer = self
                .last_stream_sync
                .is_some_and(|t| t.elapsed() < Duration::from_millis(interval_ms));

            self.pending_items = Some(items.clone());

            if defer {
                return vec![ThreadEffect::ScheduleStreamSync { interval_ms }];
            }

            self.last_stream_sync = Some(Instant::now());
            self.pending_items = None;
        } else {
            self.pending_items = None;
            self.last_stream_sync = None;
            self.stream_debounce_scheduled = false;
        }

        self.effects_for_pending_items(items)
    }

    fn effects_for_pending_items(&mut self, items: Vec<ThreadItem>) -> Vec<ThreadEffect> {
        let new_tail = tail_signature(&items);
        let content_grew = items.len() > self.items.len() || new_tail != self.last_tail_signature;
        self.last_tail_signature = new_tail;

        let mut effects = if self.try_incremental_sync(&items) {
            self.refresh_end_spacer_size();
            vec![ThreadEffect::Notify]
        } else {
            self.rebuild_all(items)
        };
        if self.stick_to_bottom && !self.user_scrolled_up && content_grew {
            effects.push(ThreadEffect::ScrollToBottom);
        }
        effects
    }

    pub(crate) fn schedule_stream_sync(&mut self, interval_ms: u64, cx: &mut Context<Self>) {
        if self.stream_debounce_scheduled {
            return;
        }
        self.stream_debounce_scheduled = true;
        let entity = cx.entity().downgrade();
        cx.spawn(async move |_weak, cx| {
            Timer::after(Duration::from_millis(interval_ms)).await;
            entity
                .update(cx, |view, cx| {
                    view.stream_debounce_scheduled = false;
                    if let Some(items) = view.pending_items.take() {
                        view.last_stream_sync = Some(Instant::now());
                        let effects = view.effects_for_pending_items(items);
                        view.apply_thread_effects(effects, cx);
                    }
                })
                .ok();
        })
        .detach();
    }

    pub(crate) fn schedule_item_update(&mut self, cx: &mut Context<Self>) {
        if self.item_update_debounce_scheduled {
            return;
        }
        self.item_update_debounce_scheduled = true;
        let batch_ms = if self.motion_paused || self.user_scrolled_up {
            self.stream_batch_ms.max(STREAM_BATCH_SLOW_MS)
        } else {
            self.stream_batch_ms
        };
        let entity = cx.entity().downgrade();
        cx.spawn(async move |_weak, cx| {
            Timer::after(Duration::from_millis(batch_ms)).await;
            entity
                .update(cx, |view, cx| {
                    let _profile =
                        crate::shared::render_profile::span("ThreadView::schedule_item_update");
                    view.item_update_debounce_scheduled = false;
                    let patch_start = Instant::now();
                    let mut effects = Vec::new();

                    let stream_ids: Vec<_> = view.pending_streaming_row_patches.drain().collect();
                    crate::shared::render_profile::record(
                        "ThreadView::stream_patch_batch",
                        Duration::ZERO,
                        stream_ids.len() as u64,
                    );
                    for id in &stream_ids {
                        if let Some(&item_ix) = view.item_index.get(id) {
                            if let Some(header_ix) = view.header_ix_for_item(item_ix as u32) {
                                let (start, end) = manifest_span(&view.manifest, header_ix);
                                if end - start == 1 {
                                    effects
                                        .extend(view.patch_streaming_assistant_row(item_ix, start));
                                }
                            }
                        }
                    }

                    let mutated_ids: Vec<_> = view.pending_mutated_item_patches.drain().collect();
                    for id in &mutated_ids {
                        if let Some(&item_ix) = view.item_index.get(id) {
                            effects.extend(view.patch_current_item_rows(item_ix));
                        }
                    }

                    let pending: Vec<_> = view.pending_item_updates.drain().collect();
                    for (_, item) in pending {
                        effects.extend(view.apply_item_update(item, cx));
                    }
                    view.record_stream_patch_cost(patch_start.elapsed());
                    view.apply_thread_effects(effects, cx);
                })
                .ok();
        })
        .detach();
    }
}
