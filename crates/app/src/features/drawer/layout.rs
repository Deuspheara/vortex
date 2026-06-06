//! Optional right drawer — reveals on demand.
//!
//! Stateless: accepts the current mode and close callback.

use gpui::{AnyElement, FontWeight, IntoElement, div, prelude::*, px};
use gpui_component::Icon;
use gpui_component::button::{Button, ButtonVariants};

use crate::features::shell::state::DrawerMode;
use crate::tokens::Tokens;
use crate::tokens::icons;

/// Render the optional drawer.  When `DrawerMode::Hidden`, renders a zero-width placeholder.
#[allow(dead_code)]
pub fn render_drawer(
    mode: &DrawerMode,
    on_close: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
) -> AnyElement {
    match mode {
        DrawerMode::Hidden => div()
            .id("drawer-hidden")
            .w(px(0.0))
            .overflow_hidden()
            .into_any_element(),
        _ => {
            let label = mode.label().to_string();
            div()
                .id("optional-drawer")
                .w(px(Tokens::DRAWER_WIDTH))
                .h_full()
                .flex()
                .flex_col()
                .bg(Tokens::panel_bg())
                .border_l_1()
                .border_color(Tokens::border())
                .overflow_hidden()
                .child(render_drawer_header(&label, on_close))
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .child(render_drawer_content(mode)),
                )
                .into_any_element()
        }
    }
}

#[allow(dead_code)]
fn render_drawer_header(
    label: &str,
    on_close: Option<Box<dyn Fn(&mut gpui::App) + 'static>>,
) -> impl IntoElement {
    let label = label.to_string();
    div()
        .h(px(36.0))
        .px(Tokens::spacing_3())
        .flex()
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(Tokens::border())
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(Tokens::text_primary())
                .child(label),
        )
        .when_some(on_close, |el, cb| {
            el.child(
                Button::new("close-drawer")
                    .icon(icons::CLOSE)
                    .ghost()
                    .compact()
                    .on_click(move |_, _, app: &mut gpui::App| cb(app)),
            )
        })
}

#[allow(dead_code)]
fn render_drawer_content(mode: &DrawerMode) -> AnyElement {
    match mode {
        DrawerMode::Changes => render_changes_panel().into_any_element(),
        DrawerMode::FileView => render_file_panel().into_any_element(),
        DrawerMode::Browser => render_browser_panel().into_any_element(),
        DrawerMode::Terminal => render_terminal_panel().into_any_element(),
        DrawerMode::Hidden => div().into_any_element(),
    }
}

// ── Panel content ──────────────────────────────────────────

#[allow(dead_code)]
fn render_changes_panel() -> impl IntoElement {
    div()
        .p(Tokens::spacing_3())
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(Tokens::text_tertiary())
                .child("Pending Changes"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(Tokens::spacing_1())
                .children(vec![
                    change_row("src/auth/login.tsx", 3, 1, "modified", Tokens::warning()),
                    change_row("src/auth/useAuth.ts", 12, 0, "added", Tokens::success()),
                    change_row("src/auth/types.ts", 0, 5, "deleted", Tokens::danger()),
                ]),
        )
}

#[allow(dead_code)]
fn change_row(
    path: &str,
    additions: usize,
    deletions: usize,
    badge: &str,
    badge_color: gpui::Hsla,
) -> impl IntoElement {
    let path = path.to_string();
    let badge = badge.to_string();
    div()
        .px(Tokens::spacing_2p5())
        .py(Tokens::spacing_2p5())
        .rounded(Tokens::radius_md())
        .bg(Tokens::surface())
        .border_1()
        .border_color(Tokens::border())
        .hover(|style| style.bg(Tokens::surface_hover()))
        .cursor_pointer()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_sm()
                        .text_color(Tokens::text_primary())
                        .child(path),
                )
                .child(
                    div()
                        .px(Tokens::spacing_1p5())
                        .py(Tokens::spacing_0p5())
                        .rounded(Tokens::radius_sm())
                        .bg(badge_color)
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Tokens::text_bright())
                                .child(badge),
                        ),
                ),
        )
        .child(
            div()
                .mt(Tokens::spacing_1())
                .flex()
                .gap(Tokens::spacing_2p5())
                .child(
                    div()
                        .text_xs()
                        .text_color(Tokens::success())
                        .child(format!("+{}", additions)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(Tokens::danger())
                        .child(format!("-{}", deletions)),
                ),
        )
}

#[allow(dead_code)]
fn render_file_panel() -> impl IntoElement {
    div()
        .p(Tokens::spacing_3())
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_xs()
                .text_color(Tokens::text_tertiary())
                .child("File View"),
        )
        .child(
            div()
                .text_sm()
                .text_color(Tokens::text_secondary())
                .child("File content will appear here"),
        )
}

#[allow(dead_code)]
fn render_browser_panel() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .h_full()
        .child(
            div()
                .px(Tokens::spacing_3())
                .py(Tokens::spacing_2())
                .flex()
                .items_center()
                .gap_2()
                .border_b_1()
                .border_color(Tokens::border())
                .child(
                    Icon::new(icons::GLOBE)
                        .size(px(14.0))
                        .text_color(Tokens::success()),
                )
                .child(
                    div()
                        .flex_1()
                        .px(Tokens::spacing_2())
                        .py(Tokens::spacing_1p5())
                        .rounded(Tokens::radius_sm())
                        .bg(Tokens::surface())
                        .child(
                            div()
                                .text_xs()
                                .text_color(Tokens::text_secondary())
                                .child("http://localhost:3000"),
                        ),
                ),
        )
        .child(
            div().flex_1().flex().items_center().justify_center().child(
                div()
                    .text_sm()
                    .text_color(Tokens::text_secondary())
                    .child("Page preview"),
            ),
        )
}

#[allow(dead_code)]
fn render_terminal_panel() -> impl IntoElement {
    div()
        .p(Tokens::spacing_4())
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .h_full()
        .child(
            div()
                .text_sm()
                .text_color(Tokens::text_secondary())
                .child("Terminal"),
        )
        .child(
            div()
                .mt(Tokens::spacing_2())
                .text_xs()
                .text_color(Tokens::text_tertiary())
                .child("Terminal output will appear here"),
        )
}
