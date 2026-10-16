//! Bundled semantic UI glyphs, with the component library as fallback.
use gpui::{AssetSource, SharedString};
use std::borrow::Cow;
pub struct AppAssets;
impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let bytes: Option<&'static [u8]> = match path {
            "vortex/paperclip.svg" => Some(include_bytes!("../assets/icons/paperclip.svg")),
            "vortex/branch.svg" => Some(include_bytes!("../assets/icons/branch.svg")),
            "vortex/shield.svg" => Some(include_bytes!("../assets/icons/shield.svg")),
            "vortex/new-thread.svg" => Some(include_bytes!("../assets/icons/new-thread.svg")),
            "vortex/brain.svg" => Some(include_bytes!("../assets/icons/brain.svg")),
            "vortex/file-edit.svg" => Some(include_bytes!("../assets/icons/file-edit.svg")),
            "vortex/wrap-lines.svg" => Some(include_bytes!("../assets/icons/wrap-lines.svg")),
            _ => None,
        };
        match bytes {
            Some(bytes) => Ok(Some(Cow::Borrowed(bytes))),
            None => gpui_component_assets::Assets.load(path),
        }
    }
    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let mut entries = gpui_component_assets::Assets.list(path)?;
        if path == "vortex" {
            entries.push("vortex/wrap-lines.svg".into());
            entries.push("vortex/brain.svg".into());
            entries.push("vortex/file-edit.svg".into());

            entries.push("vortex/paperclip.svg".into());
            entries.push("vortex/branch.svg".into());
            entries.push("vortex/shield.svg".into());
            entries.push("vortex/new-thread.svg".into());
        }
        Ok(entries)
    }
}
