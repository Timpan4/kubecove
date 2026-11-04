//! Embedded fonts and icons. Icons are the Lucide SVGs the Svelte frontend
//! renders through lucide-svelte; fonts are static cuts of the same pinned
//! fontsource files (see `scripts/gpui-fonts.ts`).

use gpui_kit::{App, AssetSource, Result, SharedString, assets::Assets};
use std::borrow::Cow;

macro_rules! embedded {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_bytes!(concat!("../assets/icons/", $name, ".svg")))),*]
    };
}

const ICONS: &[(&str, &[u8])] = embedded![
    "arrow-right-left",
    "arrow-up",
    "box",
    "boxes",
    "briefcase-business",
    "cable",
    "chevron-down",
    "chevron-right",
    "chevron-up",
    "chevrons-down-up",
    "chevrons-up-down",
    "circle-alert",
    "clock-3",
    "copy",
    "database",
    "earth",
    "file-cog",
    "folder-open",
    "folder-tree",
    "git-branch",
    "git-fork",
    "hard-drive",
    "key-round",
    "layers",
    "loader-circle",
    "minus",
    "network",
    "package",
    "panel-left-close",
    "panel-right-close",
    "pause",
    "pencil",
    "pin",
    "play",
    "plus",
    "rocket",
    "scan",
    "search",
    "server",
    "settings",
    "shield",
    "table-2",
    "triangle-alert",
    "waypoints",
    "workflow",
    "x",
];

const FONTS: &[&[u8]] = &[
    include_bytes!("../assets/fonts/inter-400.ttf"),
    include_bytes!("../assets/fonts/inter-500.ttf"),
    include_bytes!("../assets/fonts/inter-600.ttf"),
    include_bytes!("../assets/fonts/inter-700.ttf"),
    include_bytes!("../assets/fonts/geist-500.ttf"),
    include_bytes!("../assets/fonts/geist-600.ttf"),
    include_bytes!("../assets/fonts/geist-mono-400.ttf"),
    include_bytes!("../assets/fonts/geist-mono-500.ttf"),
];

/// Application icons first, then GPUI Kit's component icons.
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let icon = path
            .strip_prefix("app-icons/")
            .and_then(|name| name.strip_suffix(".svg"));
        if let Some(name) = icon {
            return Ok(ICONS
                .iter()
                .find(|(candidate, _)| *candidate == name)
                .map(|(_, bytes)| Cow::Borrowed(*bytes)));
        }
        Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Assets.list(path)
    }
}

/// Registers the bundled font families before any window renders text.
pub fn load_fonts(cx: &mut App) -> Result<()> {
    cx.text_system()
        .add_fonts(FONTS.iter().map(|bytes| Cow::Borrowed(*bytes)).collect())
}
