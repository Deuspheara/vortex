//! Thread view GPUI render.

use super::*;

impl Render for ThreadView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _profile = crate::shared::render_profile::span("ThreadView::render");
        self.clear_motion_pause_if_idle();
        self.observe_layout_width(self.scroll_handle.base_handle().bounds().size.width, cx);
        if self.chrome == ThreadChrome::Main {
            self.sync_composer_footer(cx);
        }
        self.handle_scroll_if_changed();

        // Set the exact hot-row height before v_virtual_list computes row origins.
        // Alternating an estimate with a deferred measured height caused jitter
        // and briefly put subsequent activity on top of streaming text.
        if let Some(item_ix) = self.items.iter().rposition(|item| {
            matches!(
                item,
                ThreadItem::AssistantMessage {
                    streaming: true,
                    ..
                }
            )
        }) && let Some(row_ix) = self.header_ix_for_item(item_ix as u32)
        {
            let row = RowRef::AssistantMessage {
                item_ix: item_ix as u32,
            };
            if let Some(height) = self.measure_visible_row(row, window, cx) {
                let prev = self.manifest.get(row_ix.wrapping_sub(1)).copied();
                let height = height + px(row_top_gap(row, prev, &self.items));
                if self
                    .row_sizes
                    .get(row_ix)
                    .is_some_and(|size| size.height != height)
                {
                    self.mutate_row_sizes(|sizes| sizes[row_ix].height = height);
                    if self.stick_to_bottom && !self.user_scrolled_up {
                        self.pending_scroll_bottom = true;
                    }
                }
            }
        }

        if self.pending_scroll_bottom && self.stick_to_bottom && !self.user_scrolled_up {
            let last = self.manifest.len().saturating_sub(1);
            if last > 0 {
                self.scroll_handle
                    .scroll_to_item(last, ScrollStrategy::Bottom);
            }
            self.pending_scroll_bottom = false;
        }
        if let Some(item_id) = self.pending_scroll_item_id.take() {
            if let Some(row_ix) = self.manifest.iter().position(|row| {
                row.item_ix()
                    .and_then(|ix| self.items.get(ix as usize))
                    .is_some_and(|item| item.id() == item_id)
            }) {
                self.scroll_handle
                    .scroll_to_item(row_ix, ScrollStrategy::Top);
            }
        }

        let row_sizes = Rc::clone(&self.row_sizes);
        let entity = cx.entity();

        div()
            .id("thread-surface")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .w_full()
            .font_family(Tokens::ui_font_family())
            .when(self.chrome == ThreadChrome::Main, |el| {
                el.px(Tokens::thread_padding_x())
                    .pt(Tokens::thread_padding_top())
            })
            .when(self.chrome == ThreadChrome::Embedded, |el| {
                el.px(Tokens::spacing_3()).pt(Tokens::spacing_3())
            })
            .child(
                div()
                    .id("thread-scroll")
                    .flex_1()
                    .min_h(px(0.0))
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(if self.items.is_empty() {
                        render_empty_thread_state().into_any_element()
                    } else {
                        div()
                            .id("thread-list")
                            .relative()
                            .w_full()
                            .child({
                                let view = entity.downgrade();
                                gpui::canvas(
                                    move |bounds, _, cx| {
                                        let view = view.clone();
                                        cx.defer(move |cx| {
                                            let _ = view.update(cx, |view, cx| {
                                                view.observe_layout_width(bounds.size.width, cx)
                                            });
                                        });
                                    },
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .size_full()
                            })
                            .when(self.chrome == ThreadChrome::Main, |el| {
                                el.max_w(px(Tokens::THREAD_MAX_WIDTH))
                            })
                            .flex_1()
                            .min_h(px(0.0))
                            .flex()
                            .flex_col()
                            .justify_start()
                            .child(
                                v_virtual_list(
                                    entity.clone(),
                                    "thread-virtual-list",
                                    row_sizes,
                                    |this, range, window, cx| {
                                        this.render_visible(range, window, cx)
                                    },
                                )
                                .track_scroll(&self.scroll_handle)
                                .flex_1()
                                .min_h(px(0.0))
                                .w_full(),
                            )
                            .into_any_element()
                    }),
            )
            .when(self.chrome == ThreadChrome::Main, |el| {
                el.child(render_composer_fade(self.composer_thread_inset_px()))
            })
            .when(self.user_scrolled_up, |el| {
                let view = cx.entity();
                el.child(
                    div()
                        .absolute()
                        .bottom(Tokens::spacing_3())
                        .right(Tokens::spacing_6())
                        .child(
                            gpui_component::button::Button::new("thread-jump-latest")
                                .label("Jump to latest")
                                .icon(gpui_component::IconName::ArrowDown)
                                .on_click(move |_, _, cx| {
                                    view.update(cx, |view, cx| {
                                        view.stick_to_bottom = true;
                                        view.user_scrolled_up = false;
                                        view.pending_scroll_bottom = true;
                                        cx.notify();
                                    })
                                }),
                        ),
                )
            })
    }
}
