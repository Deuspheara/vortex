mod agent;
mod assets;
mod features;
mod shared;
mod tokens;
mod ui;
mod window;

use std::sync::Arc;

use agent::AgentBridge;
use gpui::prelude::*;
use gpui::{
    App, Application, Bounds, SharedString, WindowBackgroundAppearance, WindowBounds,
    WindowOptions, px, size,
};

use tokens::{fonts, init_themes};
use ui::agent_window::AgentWindow;

gpui::actions!(vortex, [Quit, CloseWindow]);

fn main() {
    tracing_subscriber::fmt::init();
    let app = Application::new().with_assets(assets::AppAssets);

    app.run(move |cx: &mut App| {
        gpui_component::init(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &CloseWindow, cx| {
            if let Some(window) = cx
                .active_window()
                .or_else(|| cx.windows().into_iter().next())
            {
                let _ = window.update(cx, |_, window, _| window.remove_window());
            }
        });
        cx.bind_keys([
            gpui::KeyBinding::new("cmd-q", Quit, None),
            gpui::KeyBinding::new("cmd-w", CloseWindow, None),
        ]);
        cx.set_menus(vec![
            gpui::Menu {
                name: "Vortex".into(),
                items: vec![gpui::MenuItem::action("Quit Vortex", Quit)],
            },
            gpui::Menu {
                name: "File".into(),
                items: vec![gpui::MenuItem::action("Close Window", CloseWindow)],
            },
        ]);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        fonts::register_bundled_fonts(cx);
        init_themes(cx);

        // Demo builds never launch an external agent or reuse real credentials.
        let use_mock = cfg!(all(feature = "demo_mode", debug_assertions));
        let bridge = Arc::new(AgentBridge::new(use_mock));
        let bridge_for_window = bridge.clone();

        // Compute bounds before spawn (cx is &App here)
        #[cfg(all(feature = "demo_mode", debug_assertions))]
        let preferred_size = size(px(1100.), px(720.));
        #[cfg(not(all(feature = "demo_mode", debug_assertions)))]
        let preferred_size = size(px(1440.), px(900.));
        // Reproducible native QA sizes without a demo build or a user's store.
        #[cfg(debug_assertions)]
        let preferred_size = std::env::var("VORTEX_DATA_DIR")
            .ok()
            .and_then(|_| {
                let value = std::env::var("VORTEX_TEST_WINDOW_SIZE").ok()?;
                let (width, height) = value.split_once('x')?;
                let (width, height) = (width.parse::<f32>().ok()?, height.parse::<f32>().ok()?);
                (width.is_finite() && height.is_finite() && width >= 640.0 && height >= 480.0)
                    .then(|| size(px(width), px(height)))
            })
            .unwrap_or(preferred_size);
        // Keep controls reachable on laptop displays, including isolated previews.
        let window_size = cx.primary_display().map_or(preferred_size, |display| {
            let available = display.bounds().size;
            preferred_size.min(&size(available.width * 0.9, available.height * 0.9))
        });
        let bounds = Bounds::centered(None, window_size, cx);

        cx.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some(SharedString::from("Vortex Agent")),
                        appears_transparent: true,
                        traffic_light_position: None,
                    }),
                    window_background: WindowBackgroundAppearance::Blurred,
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| AgentWindow::new(cx, window, bridge_for_window.clone()));
                    cx.new(|cx| gpui_component::Root::new(view, window, cx))
                },
            )
            .expect("Failed to open window");
            cx.update(|cx| cx.activate(true)).ok();
        })
        .detach();
    });
}
