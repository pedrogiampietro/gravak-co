//! The player's settings files, `settings/<name>` beside the executable
//! (options, HUD, shortcuts, helper, Sell All lists, login). The browser has
//! no folder of its own (docs/web-spike.md): there each file is a key of the
//! page's localStorage, `gravak/settings/<name>`, and goes to the site
//! (`/api/settings`) with the signed-in account, so it follows the player
//! to any browser. The page takes the account's files at start (`pull`);
//! a file written goes up two seconds after its last change, or as the page
//! closes (`flush`).

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
        storage().ok_or_else(failed)?.set_item(&key(path), text).map_err(|_| failed())?;
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            web::changed(name);
        }
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::{flush, pull};

#[cfg(target_arch = "wasm32")]
mod web {
    use std::{cell::RefCell, collections::HashMap};
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;

    const PREFIX: &str = "gravak/settings/";
    /// How long a file rests after a change before it goes up.
    const SETTLE_MS: f64 = 2_000.0;

    #[derive(Default)]
    struct Sync {
        /// The account's files reached the page: from then on changes go up.
        on: bool,
        /// Files changed and not yet sent, with when they last changed.
        changed: HashMap<String, f64>,
        /// What was last sent of each file.
        sent: HashMap<String, String>,
    }

    thread_local! {
        static SYNC: RefCell<Sync> = RefCell::new(Sync::default());
    }

    pub(super) fn changed(name: &str) {
        SYNC.with(|sync| {
            sync.borrow_mut().changed.insert(name.to_owned(), js_sys::Date::now());
        });
    }

    /// Takes the signed-in account's files from the site into the page's
    /// storage (the site's win), and sends up those only the page has.
    /// Without a session the files stay in this browser.
    pub async fn pull() {
        let Some(storage) = super::storage() else { return };
        let Ok(files) = account_files().await else { return };
        let mut local_only = Vec::new();
        for index in 0..storage.length().unwrap_or(0) {
            if let Ok(Some(key)) = storage.key(index)
                && let Some(name) = key.strip_prefix(PREFIX)
                && !files.contains_key(name)
            {
                local_only.push(name.to_owned());
            }
        }
        SYNC.with(|sync| {
            let mut sync = sync.borrow_mut();
            for (name, body) in &files {
                let _ = storage.set_item(&format!("{PREFIX}{name}"), body);
                sync.sent.insert(name.clone(), body.clone());
            }
            for name in local_only {
                sync.changed.insert(name, 0.0);
            }
            sync.on = true;
        });
        crate::web::log(&format!("Gravak: {} settings files from the account", files.len()));
    }

    async fn account_files() -> Result<HashMap<String, String>, JsValue> {
        let window = web_sys::window().ok_or(JsValue::NULL)?;
        let response: web_sys::Response = JsFuture::from(window.fetch_with_str("/api/settings")).await?.dyn_into()?;
        if !response.ok() {
            return Err(JsValue::NULL);
        }
        let json = JsFuture::from(response.json()?).await?;
        let files = js_sys::Reflect::get(&json, &"files".into())?;
        let mut out = HashMap::new();
        for entry in js_sys::Object::entries(files.dyn_ref::<js_sys::Object>().ok_or(JsValue::NULL)?).iter() {
            let pair = js_sys::Array::from(&entry);
            if let (Some(name), Some(body)) = (pair.get(0).as_string(), pair.get(1).as_string()) {
                out.insert(name, body);
            }
        }
        Ok(out)
    }

    /// Sends up the files that rested long enough since their last change;
    /// `now`, every changed file (the page closing).
    pub fn flush(now: bool) {
        let Some(storage) = super::storage() else { return };
        let due: Vec<(String, String)> = SYNC.with(|sync| {
            let mut sync = sync.borrow_mut();
            if !sync.on {
                return Vec::new();
            }
            let clock = js_sys::Date::now();
            let names: Vec<String> = sync
                .changed
                .iter()
                .filter(|(_, at)| now || clock - **at >= SETTLE_MS)
                .map(|(name, _)| name.clone())
                .collect();
            let mut due = Vec::new();
            for name in names {
                sync.changed.remove(&name);
                let Ok(Some(body)) = storage.get_item(&format!("{PREFIX}{name}")) else { continue };
                if sync.sent.get(&name) != Some(&body) {
                    sync.sent.insert(name.clone(), body.clone());
                    due.push((name, body));
                }
            }
            due
        });
        let Some(window) = web_sys::window() else { return };
        for (name, body) in due {
            let init = web_sys::RequestInit::new();
            init.set_method("POST");
            init.set_body(&JsValue::from_str(&body));
            // Sent even while the page closes.
            let _ = js_sys::Reflect::set(&init, &"keepalive".into(), &JsValue::TRUE);
            let url = format!("/api/settings/{}", String::from(js_sys::encode_uri_component(&name)));
            let request = window.fetch_with_str_and_init(&url, &init);
            wasm_bindgen_futures::spawn_local(async move {
                let _ = JsFuture::from(request).await;
            });
        }
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
