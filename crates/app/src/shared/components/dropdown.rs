//! Custom picker dropdown — rounded menu panel with animated reveal.
//!
//! Uses gpui-component Popover for positioning (snaps to window edges).
//! Menu rows render through `uniform_list` so large catalogs stay cheap.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, Context, Corner, ElementId, Entity, FontWeight, IntoElement, Render, SharedString,
    UniformListScrollHandle, Window, div, prelude::*, px, uniform_list,
};
use gpui_component::Icon;
use gpui_component::IconName;
use gpui_component::StyledExt;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::popover::Popover;
use gpui_component::{Icon as GpuiIcon, Sizable, Size};

use crate::tokens::icons;
use crate::tokens::motion::fade_in;
use crate::tokens::{Tokens, element_key};

/// Anchor direction for the dropdown panel.
#[derive(Clone, Copy, Debug)]
pub enum DropdownAnchor {
    /// Opens below the trigger (default for top-of-screen controls).
    Below,
    /// Opens above the trigger (composer toolbar at bottom).
    Above,
}

/// Optional icon for a dropdown row.
#[derive(Clone)]
pub struct DropdownItem {
    pub label: String,
    pub icon: Option<IconName>,
}

/// Props for a single-select picker dropdown.
pub struct PickerDropdownProps {
    pub id: ElementId,
    pub label: String,
    pub items: Vec<DropdownItem>,
    pub selected: Option<String>,
    pub anchor: DropdownAnchor,
    /// Minimum width of the popover menu (trigger sizes to label).
    pub menu_min_width: f32,
    /// Optional leading icon shown on the trigger button.
    pub trigger_icon: Option<IconName>,
    /// When true, show a filter field above the list (requires parallel `search_texts`).
    pub searchable: bool,
    /// Lowercase search keys aligned with `items`; defaults to item labels when omitted.
    pub search_texts: Option<Vec<Arc<str>>>,
    pub search_placeholder: Option<String>,
    pub on_select: Rc<dyn Fn(usize, String, &mut App)>,
}

fn menu_width_for_items(items: &[DropdownItem], floor: f32) -> f32 {
    let longest = items
        .iter()
        .map(|item| item.label.chars().count())
        .max()
        .unwrap_or(0);
    let estimated = 24.0 + longest as f32 * 7.2 + 36.0;
    estimated.max(floor)
}

const PICKER_MENU_MAX_HEIGHT: f32 = 320.0;
const PICKER_SEARCH_HEIGHT: f32 = Tokens::ROW_HEIGHT_SM + 8.0;

fn filter_indices(search_keys: &[Arc<str>], query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return (0..search_keys.len()).collect();
    }
    search_keys
        .iter()
        .enumerate()
        .filter_map(|(i, key)| key.contains(&q).then_some(i))
        .collect()
}

fn default_search_keys(items: &[DropdownItem]) -> Vec<Arc<str>> {
    items
        .iter()
        .map(|item| Arc::from(item.label.to_lowercase()))
        .collect()
}

struct PickerMenuView {
    items: Rc<[DropdownItem]>,
    search_keys: Rc<[Arc<str>]>,
    matched_indices: Vec<usize>,
    search_input: Entity<InputState>,
    selected: Option<String>,
    searchable: bool,
    scroll_handle: UniformListScrollHandle,
    on_select: Rc<dyn Fn(usize, String, &mut App)>,
    items_signature: u64,
}

impl PickerMenuView {
    fn new(searchable: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search…")
                .clean_on_escape()
        });

        cx.subscribe(&search_input, |view, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                view.refresh_filter(cx);
                cx.notify();
            }
        })
        .detach();

        Self {
            items: Rc::new([]),
            search_keys: Rc::new([]),
            matched_indices: Vec::new(),
            search_input,
            selected: None,
            searchable,
            scroll_handle: UniformListScrollHandle::new(),
            on_select: Rc::new(|_, _, _| {}),
            items_signature: 0,
        }
    }

    fn items_signature(items: &[DropdownItem]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        items.len().hash(&mut hasher);
        for item in items {
            item.label.hash(&mut hasher);
        }
        hasher.finish()
    }

    fn sync(
        &mut self,
        items: &[DropdownItem],
        search_texts: Option<&[Arc<str>]>,
        selected: Option<String>,
        searchable: bool,
        search_placeholder: Option<String>,
        on_select: Rc<dyn Fn(usize, String, &mut App)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.searchable = searchable;
        self.selected = selected;
        self.on_select = on_select;

        if searchable {
            if let Some(placeholder) = search_placeholder {
                self.search_input.update(cx, |state, cx| {
                    state.set_placeholder(placeholder, window, cx);
                });
            }
        }

        let signature = Self::items_signature(items);
        if signature != self.items_signature {
            self.items = Rc::from(items.to_vec().into_boxed_slice());
            let keys = search_texts
                .map(|keys| keys.to_vec())
                .unwrap_or_else(|| default_search_keys(items));
            self.search_keys = Rc::from(keys);
            self.items_signature = signature;
            self.search_input.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });
            self.matched_indices = (0..self.items.len()).collect();
        } else {
            self.refresh_filter(cx);
        }
    }

    fn refresh_filter(&mut self, cx: &mut Context<Self>) {
        let query = self.search_input.read(cx).value().to_string();
        self.matched_indices = filter_indices(&self.search_keys, &query);
    }

    fn list_height(&self) -> f32 {
        let chrome = if self.searchable {
            PICKER_SEARCH_HEIGHT + 4.0
        } else {
            4.0
        };
        let visible_rows = self.matched_indices.len().max(1) as f32;
        let fit_height = visible_rows * Tokens::ROW_HEIGHT_MD;
        fit_height.min((PICKER_MENU_MAX_HEIGHT - chrome).max(Tokens::ROW_HEIGHT_MD))
    }
}

impl Render for PickerMenuView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.matched_indices.len();
        let items = self.items.clone();
        let matched = self.matched_indices.clone();
        let selected = self.selected.clone();
        let on_select = self.on_select.clone();
        let scroll_handle = self.scroll_handle.clone();
        let list_height = px(self.list_height());
        let row_height = px(Tokens::ROW_HEIGHT_MD);

        div()
            .flex()
            .flex_col()
            .when(self.searchable, |menu| {
                menu.child(
                    div()
                        .px(Tokens::spacing_1p5())
                        .pt(Tokens::spacing_1p5())
                        .pb(Tokens::spacing_1())
                        .child(
                            Input::new(&self.search_input)
                                .with_size(Size::Small)
                                .prefix(
                                    GpuiIcon::new(icons::SEARCH)
                                        .size(px(14.0))
                                        .text_color(Tokens::text_tertiary()),
                                )
                                .cleanable(true)
                                .appearance(false),
                        ),
                )
            })
            .when(count == 0, |menu| {
                menu.child(
                    div()
                        .h(row_height)
                        .px(Tokens::spacing_2())
                        .flex()
                        .items_center()
                        .text_size(Tokens::text_sm())
                        .text_color(Tokens::text_tertiary())
                        .child("No matches"),
                )
            })
            .when(count > 0, |menu| {
                menu.child(
                    uniform_list(
                        element_key("picker-menu-list", &self.items_signature.to_string()),
                        count,
                        move |range, _window, _cx| {
                            range
                                .map(|visible_ix| {
                                    let source_ix = matched[visible_ix];
                                    let item = &items[source_ix];
                                    let is_selected =
                                        selected.as_deref() == Some(item.label.as_str());
                                    let item_label = item.label.clone();
                                    let on_select = on_select.clone();
                                    dropdown_item(
                                        element_key("picker-item", &source_ix.to_string()),
                                        &item.label,
                                        item.icon.clone(),
                                        is_selected,
                                        move |_, _, app| {
                                            on_select(source_ix, item_label.clone(), app);
                                        },
                                    )
                                    .into_any_element()
                                })
                                .collect()
                        },
                    )
                    .h(list_height)
                    .track_scroll(scroll_handle),
                )
            })
    }
}

/// Renders a compact trigger + animated rounded menu.
pub fn picker_dropdown(props: PickerDropdownProps) -> impl IntoElement {
    let label = props.label;
    let items = props.items;
    let selected = props.selected;
    let on_select = props.on_select;
    let trigger_icon = props.trigger_icon;
    let searchable = props.searchable;
    let search_texts = props.search_texts;
    let search_placeholder = props.search_placeholder;
    let menu_width = menu_width_for_items(&items, props.menu_min_width);
    let popover_corner = match props.anchor {
        DropdownAnchor::Below => Corner::TopLeft,
        DropdownAnchor::Above => Corner::BottomLeft,
    };

    let trigger_label = label.clone();
    let menu_state_key = element_key("picker-menu-state", &props.id.to_string());

    let mut trigger = Button::new(element_key("picker-trigger", &trigger_label))
        .label(SharedString::from(label))
        .ghost()
        .compact()
        .font_normal()
        .text_color(Tokens::text_secondary())
        .h(px(Tokens::ROW_HEIGHT_XS))
        .px(Tokens::spacing_1p5())
        .rounded(Tokens::radius_xs());

    if let Some(icon) = trigger_icon {
        trigger = trigger.icon(icon).dropdown_caret(true);
    } else {
        trigger = trigger.icon(icons::CHEVRON_DOWN);
    }

    Popover::new(props.id)
        .anchor(popover_corner)
        .appearance(false)
        .overlay_closable(true)
        .trigger(trigger)
        .content(move |_, window, cx| {
            let menu = window.use_keyed_state(menu_state_key.clone(), cx, |window, cx| {
                PickerMenuView::new(searchable, window, cx)
            });

            menu.update(cx, |view, cx| {
                view.sync(
                    &items,
                    search_texts.as_deref(),
                    selected.clone(),
                    searchable,
                    search_placeholder.clone(),
                    on_select.clone(),
                    window,
                    cx,
                );
            });

            fade_in(
                div()
                    .w(px(menu_width))
                    .min_w(px(menu_width))
                    .max_h(px(PICKER_MENU_MAX_HEIGHT))
                    .rounded(Tokens::radius_lg())
                    .bg(Tokens::surface_overlay())
                    .border_1()
                    .border_color(Tokens::border())
                    .shadow_md()
                    .overflow_hidden()
                    .child(menu.clone()),
                "picker-menu-in",
            )
        })
}

fn dropdown_item(
    id: impl Into<ElementId>,
    label: &str,
    icon: Option<IconName>,
    selected: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let label = label.to_string();
    div()
        .id(id)
        .h(px(Tokens::ROW_HEIGHT_MD))
        .px(Tokens::spacing_2())
        .rounded(Tokens::radius_sm())
        .flex()
        .items_center()
        .justify_between()
        .gap(Tokens::spacing_2())
        .cursor_pointer()
        .when(selected, |el| {
            el.bg(Tokens::surface_active())
                .text_color(Tokens::text_primary())
        })
        .when(!selected, |el| {
            el.text_color(Tokens::text_secondary())
                .hover(|s| s.bg(Tokens::surface_hover()))
        })
        .on_click(on_click)
        .child(
            div()
                .flex()
                .items_center()
                .gap(Tokens::spacing_2())
                .min_w(px(0.0))
                .flex_1()
                .when_some(icon, |el, icon| {
                    el.child(Icon::new(icon).size(px(14.0)).text_color(if selected {
                        Tokens::accent()
                    } else {
                        Tokens::text_tertiary()
                    }))
                })
                .child(
                    div()
                        .text_size(Tokens::text_sm())
                        .font_weight(FontWeight::NORMAL)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(label),
                ),
        )
        .when(selected, |el| {
            el.child(
                Icon::new(icons::CHECK)
                    .size(px(12.0))
                    .text_color(Tokens::accent()),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_matches_label_substring() {
        let keys = vec![
            Arc::from("anthropic claude sonnet"),
            Arc::from("openai gpt"),
        ];
        assert_eq!(filter_indices(&keys, "gpt"), vec![1]);
        assert_eq!(filter_indices(&keys, ""), vec![0, 1]);
    }
}
