//! Virtualized native review surface. This view owns only scrolling and layout caches;
//! review preferences and actions remain in window orchestration.
use super::{
    components::code_highlight::highlight_code,
    projection::{ReviewCell, ReviewRow, review_rows},
    state::DiffPresentation,
};
use crate::shared::components::{
    buttons::btn_icon_sm,
    collapsible_row::disclosure_chevron,
    markdown_preview::{sanitize_highlights, split_highlights_by_line},
};
use crate::{
    features::shell::state::{DiffFile, DiffRow, DiffRowKind},
    tokens::{Tokens, element_key, icons},
};
use gpui::{
    App, Context, IntoElement, Render, RenderOnce, StyledText, TextRun, TextStyle, Window, div,
    prelude::*, px,
};
use gpui_component::{Icon, VirtualListScrollHandle, v_virtual_list};
use std::{
    cell::Cell,
    collections::HashMap,
    hash::{Hash, Hasher},
    ops::Range,
    rc::Rc,
};

type ToggleFile = Rc<dyn Fn(String, &mut App)>;

#[derive(Clone)]
pub struct ReviewConfig {
    pub files: Rc<Vec<DiffFile>>,
    pub presentation: DiffPresentation,
    pub width: f32,
    pub selected_file: usize,
    pub on_toggle: ToggleFile,
}
#[derive(IntoElement)]
pub struct DiffReview(pub ReviewConfig);
impl RenderOnce for DiffReview {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let view = window.use_keyed_state("diff-review-view", cx, |_, _| ReviewView::new());
        view.update(cx, |view, cx| view.sync(self.0, window, cx));
        div().size_full().min_w(px(0.0)).min_h(px(0.0)).child(view)
    }
}
type LineStyles = Vec<(Range<usize>, gpui::HighlightStyle)>;
#[derive(Clone, Default)]
struct ReviewHorizontal {
    offset: Rc<Cell<f32>>,
    overflow: Rc<Cell<f32>>,
    origin: Rc<Cell<f32>>,
}
struct ReviewView {
    config: Option<ReviewConfig>,
    signature: u64,
    rows: Vec<ReviewRow>,
    sizes: Rc<Vec<gpui::Size<gpui::Pixels>>>,
    styles: HashMap<(usize, usize), LineStyles>,
    scroll: VirtualListScrollHandle,
    content_width: f32,
    measured_width: Option<f32>,
    code_content_width: f32,
    horizontal: ReviewHorizontal,
    code_visible_width: f32,
}
impl ReviewView {
    fn new() -> Self {
        Self {
            config: None,
            signature: 0,
            rows: vec![],
            sizes: Rc::new(vec![]),
            styles: HashMap::new(),
            scroll: VirtualListScrollHandle::new(),
            content_width: 0.0,
            measured_width: None,
            code_content_width: 0.0,
            horizontal: ReviewHorizontal::default(),
            code_visible_width: 0.0,
        }
    }
    fn sync(&mut self, mut config: ReviewConfig, window: &mut Window, cx: &mut Context<Self>) {
        config.width = self.measured_width.unwrap_or(config.width);
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}", config.files).hash(&mut hash);
        config.width.to_bits().hash(&mut hash);
        config.presentation.split.hash(&mut hash);
        config.presentation.wrap.hash(&mut hash);
        config.presentation.show_tree.hash(&mut hash);
        let mut collapsed: Vec<_> = config.presentation.collapsed.iter().collect();
        collapsed.sort();
        collapsed.hash(&mut hash);
        let signature = hash.finish();
        let selected_changed = self
            .config
            .as_ref()
            .is_some_and(|old| old.selected_file != config.selected_file);
        if self.signature == signature {
            if selected_changed {
                self.scroll_to_file(config.selected_file);
                cx.notify();
            }
            self.config = Some(config);
            return;
        }
        self.signature = signature;
        self.rows = review_rows(&config.files, &config.presentation);
        self.styles.clear();
        for (file_ix, file) in config.files.iter().enumerate() {
            let body = file
                .flat_rows
                .iter()
                .map(|row| match row {
                    DiffRow::Context { text, .. }
                    | DiffRow::Added { text, .. }
                    | DiffRow::Removed { text, .. } => text.as_str(),
                    _ => "",
                })
                .collect::<Vec<_>>()
                .join("\n");
            let lang = match file.path.rsplit('.').next().unwrap_or("") {
                "rs" => "rust",
                "ts" | "tsx" => "typescript",
                "js" | "jsx" => "javascript",
                "py" => "python",
                "sh" => "bash",
                other => other,
            };
            for (row, styles) in
                split_highlights_by_line(&body, &highlight_code(Some(lang), &body, true))
                    .into_iter()
                    .enumerate()
            {
                self.styles.insert((file_ix, row), styles);
            }
        }
        let tree = if config.presentation.show_tree {
            Tokens::DIFF_TREE_WIDTH
        } else {
            0.0
        };
        let width = (config.width - tree).max(Tokens::DIFF_REVIEW_MIN_WIDTH);
        self.content_width = width;
        let cell_width = if config.presentation.split {
            self.content_width / 2.0
        } else {
            self.content_width
        };
        let gutter = if config.presentation.split {
            Tokens::DIFF_REVIEW_GUTTER / 2.0
        } else {
            Tokens::DIFF_REVIEW_GUTTER
        };
        let code_width = (cell_width - gutter - f32::from(Tokens::spacing_3())).max(1.0);
        self.code_visible_width = code_width;
        self.code_content_width = code_width;
        if !config.presentation.wrap {
            for row in config.files.iter().flat_map(|file| &file.flat_rows) {
                if let DiffRow::Context { text, .. }
                | DiffRow::Added { text, .. }
                | DiffRow::Removed { text, .. } = row
                {
                    let run = TextRun {
                        len: text.len(),
                        font: gpui::font(Tokens::code_font_family()),
                        color: Tokens::code_fg(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    if let Ok(lines) = window.text_system().shape_text(
                        text.clone().into(),
                        Tokens::text_code(),
                        &[run],
                        None,
                        None,
                    ) {
                        for line in lines {
                            self.code_content_width =
                                self.code_content_width.max(f32::from(line.width()));
                        }
                    }
                }
            }
        }
        self.horizontal
            .overflow
            .set((self.code_content_width - self.code_visible_width).max(0.0));
        self.horizontal.offset.set(
            self.horizontal
                .offset
                .get()
                .min(self.horizontal.overflow.get()),
        );
        let line_height = |cell: &ReviewCell| {
            if !config.presentation.wrap {
                return Tokens::DIFF_LINE_HEIGHT;
            }
            let text = if cell.text.is_empty() {
                " "
            } else {
                &cell.text
            };
            let run = TextRun {
                len: text.len(),
                font: gpui::font(Tokens::code_font_family()),
                color: Tokens::code_fg(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let count = window
                .text_system()
                .shape_text(
                    text.to_string().into(),
                    Tokens::text_code(),
                    &[run],
                    Some(px(code_width)),
                    None,
                )
                .map(|lines| {
                    lines
                        .iter()
                        .map(|line| line.wrap_boundaries.len() + 1)
                        .sum::<usize>()
                })
                .unwrap_or(1)
                .max(1);
            count as f32 * Tokens::DIFF_LINE_HEIGHT
        };
        self.sizes = Rc::new(
            self.rows
                .iter()
                .map(|row| {
                    gpui::size(
                        px(self.content_width),
                        px(match row {
                            ReviewRow::File { .. } => Tokens::DIFF_PATH_HEIGHT,
                            ReviewRow::Section { .. } => Tokens::ROW_HEIGHT_XS,
                            ReviewRow::Code { left, right, .. } => left
                                .iter()
                                .chain(right)
                                .map(&line_height)
                                .fold(Tokens::DIFF_LINE_HEIGHT, f32::max),
                        }),
                    )
                })
                .collect(),
        );
        if selected_changed {
            self.scroll_to_file(config.selected_file);
        }
        self.config = Some(config);
        cx.notify();
    }
    fn scroll_to_file(&mut self, file_index: usize) {
        if let Some(index) = self
            .rows
            .iter()
            .position(|row| matches!(row,ReviewRow::File{file} if *file==file_index))
        {
            self.scroll
                .scroll_to_item(index, gpui::ScrollStrategy::Center);
        }
    }
    fn render_row(&self, index: usize, view: gpui::Entity<Self>) -> gpui::AnyElement {
        let config = self.config.as_ref().expect("review configured");
        let height = self.sizes[index].height;
        match &self.rows[index] {
            ReviewRow::File { file } => {
                let item = &config.files[*file];
                let path = item.path.clone();
                let copy_path = path.clone();
                let toggle = config.on_toggle.clone();
                let label = if std::path::Path::new(&path).is_absolute() {
                    std::path::Path::new(&path)
                        .file_name()
                        .and_then(|p| p.to_str())
                        .unwrap_or(&path)
                        .to_string()
                } else {
                    path.clone()
                };
                div()
                    .id(element_key("review-file", &path))
                    .h(height)
                    .w_full()
                    .px(Tokens::spacing_2())
                    .flex()
                    .items_center()
                    .gap(Tokens::spacing_2())
                    .bg(Tokens::main_bg())
                    .border_b_1()
                    .border_color(Tokens::border_subtle())
                    .cursor_pointer()
                    .on_click(move |_, _, app| toggle(path.clone(), app))
                    .child(disclosure_chevron(
                        !config.presentation.collapsed.contains(&item.path),
                    ))
                    .child(
                        Icon::new(icons::FILE_CODE)
                            .size(Tokens::spacing_4())
                            .text_color(Tokens::text_tertiary()),
                    )
                    .child(
                        div()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_size(Tokens::text_label())
                            .text_color(Tokens::text_primary())
                            .child(label),
                    )
                    .child(
                        btn_icon_sm(element_key("copy-diff-path", &item.path), icons::COPY)
                            .tooltip("Copy file path")
                            .on_click(move |_, _, app| {
                                app.stop_propagation();
                                app.write_to_clipboard(copy_path.clone().into());
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(Tokens::text_label())
                            .text_color(Tokens::success())
                            .child(format!("+{}", item.added)),
                    )
                    .child(
                        div()
                            .text_size(Tokens::text_label())
                            .text_color(Tokens::danger())
                            .child(format!("−{}", item.removed)),
                    )
                    .into_any_element()
            }
            ReviewRow::Section { label, .. } => div()
                .h(height)
                .w_full()
                .px(Tokens::spacing_3())
                .flex()
                .items_center()
                .bg(Tokens::surface_hover().opacity(0.35))
                .text_size(Tokens::text_label())
                .font_family(Tokens::code_font_family())
                .text_color(Tokens::text_tertiary())
                .child(label.clone())
                .into_any_element(),
            ReviewRow::Code { file, left, right } => {
                let split = config.presentation.split;
                let config_wrap = config.presentation.wrap;
                div()
                    .id(element_key("review-code", &index.to_string()))
                    .on_scroll_wheel(move |event, window, app| {
                        if split || !config_wrap {
                            let delta = event.delta.pixel_delta(window.line_height());
                            if !config_wrap && delta.x.abs() > delta.y.abs() && delta.x != px(0.0) {
                                let dx = f32::from(delta.x);
                                view.update(app, |view, cx| {
                                    view.horizontal.offset.set(
                                        (view.horizontal.offset.get() - dx)
                                            .clamp(0.0, view.horizontal.overflow.get()),
                                    );
                                    cx.notify();
                                });
                                app.stop_propagation();
                            }
                        }
                    })
                    .h(height)
                    .w_full()
                    .flex()
                    .when(split, |el| {
                        el.child(self.render_cell(*file, left.as_ref(), true, false))
                    })
                    .child(self.render_cell(*file, right.as_ref(), split, true))
                    .into_any_element()
            }
        }
    }
    fn render_cell(
        &self,
        file: usize,
        cell: Option<&ReviewCell>,
        split: bool,
        right: bool,
    ) -> gpui::AnyElement {
        let config = self.config.as_ref().expect("review configured");
        let (background, indicator) = match cell.map(|cell| &cell.kind) {
            Some(DiffRowKind::Add) => (Tokens::diff_add_bg(), Tokens::success()),
            Some(DiffRowKind::Remove) => (Tokens::diff_del_bg(), Tokens::danger()),
            _ => (Tokens::main_bg().opacity(0.0), Tokens::text_tertiary()),
        };
        let mut element = div()
            .flex_1()
            .min_w(px(0.0))
            .h_full()
            .flex()
            .items_start()
            .bg(background)
            .when(split && !right, |el| {
                el.border_r_1().border_color(Tokens::border())
            });
        let Some(cell) = cell else {
            return element.into_any_element();
        };
        let number = |number: Option<usize>| {
            div()
                .w(px(Tokens::DIFF_NUMBER_WIDTH))
                .flex_shrink_0()
                .pr(Tokens::spacing_1())
                .text_right()
                .font_family(Tokens::code_font_family())
                .text_size(Tokens::text_code())
                .line_height(px(Tokens::DIFF_LINE_HEIGHT))
                .text_color(
                    if matches!(cell.kind, DiffRowKind::Add | DiffRowKind::Remove) {
                        indicator
                    } else {
                        Tokens::diff_line_number()
                    },
                )
                .child(number.map(|n| n.to_string()).unwrap_or_default())
        };
        if !split {
            element = element.child(number(cell.old));
        }
        element = element.child(number(if split && !right { cell.old } else { cell.new }));
        let sign = match cell.kind {
            DiffRowKind::Add => "+",
            DiffRowKind::Remove => "−",
            _ => "",
        };
        let text = if cell.text.is_empty() {
            " ".to_string()
        } else {
            cell.text.clone()
        };
        let style = TextStyle {
            font_family: Tokens::code_font_family().into(),
            font_size: Tokens::text_code().into(),
            color: Tokens::code_fg(),
            ..Default::default()
        };
        let highlights = sanitize_highlights(
            &text,
            self.styles
                .get(&(file, cell.source_row))
                .cloned()
                .unwrap_or_default(),
        );
        element
            .child(
                div()
                    .w(px(Tokens::DIFF_SIGN_WIDTH))
                    .flex_shrink_0()
                    .text_size(Tokens::text_code())
                    .line_height(px(Tokens::DIFF_LINE_HEIGHT))
                    .font_family(Tokens::code_font_family())
                    .text_color(indicator)
                    .child(sign),
            )
            .child(
                div()
                    .id(element_key(
                        "diff-code-scroll",
                        &format!("{file}-{}-{right}", cell.source_row),
                    ))
                    .flex_1()
                    .min_w(px(0.0))
                    .h_full()
                    .overflow_hidden()
                    .child(
                        div()
                            .pr(Tokens::spacing_3())
                            .font_family(Tokens::code_font_family())
                            .text_size(Tokens::text_code())
                            .line_height(px(Tokens::DIFF_LINE_HEIGHT))
                            .when(config.presentation.wrap, |el| {
                                el.w_full().whitespace_normal()
                            })
                            .when(!config.presentation.wrap, |el| {
                                el.w(px(self.code_content_width + f32::from(Tokens::spacing_3())))
                                    .relative()
                                    .left(px(-self.horizontal.offset.get()))
                                    .whitespace_nowrap()
                            })
                            .child(
                                StyledText::new(text).with_default_highlights(&style, highlights),
                            ),
                    ),
            )
            .into_any_element()
    }
    fn horizontal_bar(&self, view: gpui::Entity<Self>) -> impl IntoElement {
        let click_origin = self.horizontal.origin.clone();
        let move_origin = click_origin.clone();
        let tree = if self.config.as_ref().unwrap().presentation.show_tree {
            Tokens::DIFF_TREE_WIDTH
        } else {
            0.0
        };
        let click_view = view.clone();
        let width = self.content_width;
        let thumb =
            (width * width / (width + self.horizontal.overflow.get())).max(Tokens::ROW_HEIGHT_LG);
        let travel = (width - thumb).max(1.0);
        let left = self.horizontal.offset.get() / self.horizontal.overflow.get() * travel;
        let set = move |view: &gpui::Entity<Self>, x: gpui::Pixels, origin: f32, app: &mut App| {
            let ratio = (((f32::from(x) - origin - tree) - thumb / 2.0) / travel).clamp(0.0, 1.0);
            view.update(app, |view, cx| {
                view.horizontal
                    .offset
                    .set(ratio * view.horizontal.overflow.get());
                cx.notify();
            });
            app.stop_propagation();
        };
        div()
            .id("review-horizontal-scrollbar")
            .absolute()
            .left_0()
            .bottom(Tokens::spacing_2())
            .w_full()
            .h(Tokens::spacing_4())
            .flex_shrink_0()
            .bg(Tokens::panel_bg())
            .cursor_pointer()
            .on_mouse_down(gpui::MouseButton::Left, move |event, _, app| {
                set(&click_view, event.position.x, click_origin.get(), app);
            })
            .on_mouse_move(move |event, _, app| {
                if event.pressed_button == Some(gpui::MouseButton::Left) {
                    set(&view, event.position.x, move_origin.get(), app);
                }
            })
            .flex()
            .items_center()
            .occlude()
            .child(div().w(px(left)).flex_shrink_0())
            .child(
                div()
                    .w(px(thumb))
                    .h(Tokens::spacing_2())
                    .flex_shrink_0()
                    .rounded(Tokens::radius_sm())
                    .bg(Tokens::text_tertiary()),
            )
    }
}
impl Render for ReviewView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.signature == 0 {
            self.sync(self.config.clone().expect("review configured"), window, cx);
        }
        let config = self.config.as_ref().expect("review configured").clone();
        let view = cx.entity();
        let measure = view.downgrade();
        let origin = self.horizontal.origin.clone();
        div()
            .relative()
            .child(
                gpui::canvas(
                    move |bounds, _, app| {
                        let width = f32::from(bounds.size.width);
                        origin.set(f32::from(bounds.origin.x));
                        let measure = measure.clone();
                        app.defer(move |app| {
                            let _ = measure.update(app, |view, cx| {
                                if width > 0.0
                                    && view
                                        .measured_width
                                        .is_none_or(|old| (old - width).abs() >= 1.0)
                                {
                                    view.measured_width = Some(width);
                                    view.signature = 0;
                                    cx.notify();
                                }
                            });
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .size_full()
            .min_h(px(0.0))
            .min_w(px(0.0))
            .flex()
            .bg(Tokens::main_bg())
            .when(config.presentation.show_tree, |el| {
                el.child(
                    div()
                        .id("diff-file-tree")
                        .w(px(Tokens::DIFF_TREE_WIDTH))
                        .flex_shrink_0()
                        .h_full()
                        .overflow_y_scroll()
                        .border_r_1()
                        .border_color(Tokens::border_subtle())
                        .p(Tokens::spacing_1())
                        .children(config.files.iter().enumerate().map(|(file_ix, file)| {
                            let target = self
                                .rows
                                .iter()
                                .position(
                                    |row| matches!(row,ReviewRow::File{file} if *file==file_ix),
                                )
                                .unwrap_or(0);
                            let view = view.clone();
                            let label = std::path::Path::new(&file.path)
                                .file_name()
                                .and_then(|p| p.to_str())
                                .unwrap_or(&file.path)
                                .to_string();
                            div()
                                .id(element_key("diff-tree-file", &file.path))
                                .w_full()
                                .h(px(Tokens::ROW_HEIGHT_MD))
                                .px(Tokens::spacing_2())
                                .rounded(Tokens::radius_xs())
                                .flex()
                                .items_center()
                                .gap(Tokens::spacing_2())
                                .cursor_pointer()
                                .hover(|s| s.bg(Tokens::surface_hover()))
                                .on_click(move |_, _, app| {
                                    view.update(app, |view, cx| {
                                        view.scroll
                                            .scroll_to_item(target, gpui::ScrollStrategy::Center);
                                        cx.notify();
                                    })
                                })
                                .child(
                                    Icon::new(icons::FILE_CODE)
                                        .size(Tokens::spacing_4())
                                        .text_color(Tokens::text_tertiary()),
                                )
                                .child(
                                    div()
                                        .min_w(px(0.0))
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_size(Tokens::text_label())
                                        .text_color(Tokens::text_secondary())
                                        .child(label),
                                )
                        })),
                )
            })
            .child(
                div()
                    .id("diff-review-horizontal")
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.0))
                    .h_full()
                    .overflow_hidden()
                    .child(
                        v_virtual_list(
                            cx.entity(),
                            "diff-review-list",
                            self.sizes.clone(),
                            |this, range, _, cx| {
                                let view = cx.entity();
                                range
                                    .map(|index| this.render_row(index, view.clone()))
                                    .collect()
                            },
                        )
                        .track_scroll(&self.scroll)
                        .w(px(self.content_width))
                        .flex_1()
                        .min_h(px(0.0)),
                    )
                    .when(
                        !config.presentation.wrap && self.horizontal.overflow.get() > 0.0,
                        |el| el.child(self.horizontal_bar(view.clone())),
                    ),
            )
    }
}
