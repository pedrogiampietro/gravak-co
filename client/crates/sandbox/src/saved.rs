//! The player's settings files, `settings/<name>` beside the executable
//! (options, HUD, shortcuts, helper, Sell All lists, login). The browser has
//! no folder of its own (docs/web-spike.md): there each file is a key of the
//! page's localStorage, `gravak/settings/<name>`, kept by that browser.

use std::path::{Path, PathBuf};

/// Where the settings file `name` is kept.
pub fn path(name: &str) -> Option<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    return Some(std::env::current_exe().ok()?.parent()?.join("settings").join(name));
    #[cfg(target_arch = "wasm32")]
    return Some(Path::new("settings").join(name));
}

/// The text of the settings file at `path`, if there is one.
pub fn read(path: &Path) -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    return std::fs::read_to_string(path).ok();
    #[cfg(target_arch = "wasm32")]
    return storage()?.get_item(&key(path)).ok().flatten();
}

/// Keeps `text` as the settings file at `path`.
pub fn write(path: &Path, text: &str) -> std::io::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, text)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let failed = || std::io::Error::other("the browser keeps no settings here");
        storage().ok_or_else(failed)?.set_item(&key(path), text).map_err(|_| failed())
    }
}

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

#[cfg(target_arch = "wasm32")]
fn key(path: &Path) -> String {
    format!("gravak/{}", path.to_string_lossy().replace('\\', "/"))
}
