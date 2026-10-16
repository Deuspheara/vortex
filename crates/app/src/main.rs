mod agent;
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

fn main() {
    tracing_subscriber::fmt::init();
    let app = Application::new().with_assets(gpui_component_assets::Assets);

    app.run(move |cx: &mut App| {
        gpui_component::init(cx);
        fonts::register_bundled_fonts(cx);
        init_themes(cx);

        let use_mock = match std::env::var("OPENROUTER_API_KEY") {
            Ok(key) if !key.trim().is_empty() => false,
            _ => true,
        };
        if use_mock {
            tracing::warn!(
                "OPENROUTER_API_KEY not set — using mock provider. \
                 Get a key at https://openrouter.ai/keys and export it before launching."
            );
        }
        let bridge = Arc::new(AgentBridge::new(use_mock));
        let bridge_for_window = bridge.clone();

        // Compute bounds before spawn (cx is &App here)
        #[cfg(all(feature = "demo_mode", debug_assertions))]
        let preferred_size = size(px(1100.), px(720.));
        #[cfg(not(all(feature = "demo_mode", debug_assertions)))]
        let preferred_size = size(px(1400.), px(900.));
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
        })
        .detach();
    });
}
