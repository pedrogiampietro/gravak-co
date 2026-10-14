//! The browser build's start (docs/web-spike.md): the page loads the wasm,
//! this fetches the game files the login screens need (listed in
//! `assets/manifest.txt` beside the page, made by tools/build-web.ps1) and
//! shows the screens on a canvas. The sessions reach the servers through
//! the WebSocket bridge (`crates/ws-bridge`); entering the world needs
//! streamed game data, a later phase.

use crate::assets::Assets;
use std::cell::Cell;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

pub fn start() {
    console_error_panic_hook::set_once();
    wasm_bindgen_futures::spawn_local(async {
        if let Err(error) = run().await {
            web_sys::console::error_1(&format!("Gravak: {error}").into());
            show_error(&error);
        }
    });
}

async fn run() -> Result<(), String> {
    let started = web_time::Instant::now();
    let manifest = String::from_utf8(fetch("assets/manifest.txt").await?)
        .map_err(|_| "assets/manifest.txt is not text")?;
    let mut files = Vec::new();
    let mut bytes = 0;
    for name in manifest.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')) {
        let data = fetch(&format!("assets/{}", name.to_ascii_lowercase())).await?;
        bytes += data.len();
        files.push((name.to_owned(), data));
    }
    log(&format!(
        "Gravak: {} game files ({} KiB) in {} ms",
        files.len(),
        bytes / 1024,
        started.elapsed().as_millis()
    ));
    let mut assets = Assets::from_memory(files);
    gravak_protocol::transport::set_bridge(&bridge_url()?);
    // The worlds of `servers.ini` beside the page (with the login keys the
    // passwords are sealed to); without it, the server on this machine.
    let fallback = crate::worlds::World::new("Gravak", "127.0.0.1:9958", "Gravak");
    let worlds = match fetch("servers.ini").await {
        Ok(text) => crate::worlds::parse(&String::from_utf8_lossy(&text)),
        Err(_) => Vec::new(),
    };
    let worlds = if worlds.is_empty() { vec![fallback] } else { worlds };
    crate::launcher::spawn(&mut assets, worlds)
}

/// The bridge: `?bridge=ws://host:port` on the page's address, or `/ws` on
/// the page's own host (`wss://` behind https).
fn bridge_url() -> Result<String, String> {
    let location = web_sys::window().ok_or("no window")?.location();
    let search = location.search().unwrap_or_default();
    if let Some(url) = search
        .trim_start_matches('?')
        .split('&')
        .find_map(|pair| pair.strip_prefix("bridge="))
    {
        return js_sys::decode_uri_component(url)
            .map(String::from)
            .map_err(|_| "the bridge address is not valid".into());
    }
    let scheme = if location.protocol().unwrap_or_default() == "https:" { "wss" } else { "ws" };
    Ok(format!("{scheme}://{}/ws", location.host().unwrap_or_default()))
}

/// Resolves after `duration`, without blocking the page.
pub async fn sleep(duration: web_time::Duration) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                &resolve,
                duration.as_millis().min(i32::MAX as u128) as i32,
            );
        }
    });
    let _ = JsFuture::from(promise).await;
}

async fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let failed = |error: JsValue| format!("{url}: {error:?}");
    let window = web_sys::window().ok_or("no window")?;
    let response: web_sys::Response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(failed)?
        .dyn_into()
        .map_err(failed)?;
    if !response.ok() {
        return Err(format!("{url}: HTTP {}", response.status()));
    }
    let buffer = JsFuture::from(response.array_buffer().map_err(failed)?)
        .await
        .map_err(failed)?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

pub fn log(text: &str) {
    web_sys::console::log_1(&text.into());
}

fn show_error(error: &str) {
    if let Some(body) = web_sys::window().and_then(|window| window.document()).and_then(|document| document.body()) {
        let _ = body.insert_adjacent_text("beforeend", &format!("Gravak could not start: {error}"));
    }
}

thread_local! {
    static PAINTED: Cell<(u32, f64, f64)> = const { Cell::new((0, 0.0, 0.0)) };
}

/// One frame of the login screens painted in `millis`; every two seconds
/// the console gets the frame rate and the mean paint time.
pub fn frame_painted(millis: f64) {
    let now = js_sys::Date::now();
    PAINTED.with(|painted| {
        let (frames, total, since) = painted.get();
        let (frames, total) = (frames + 1, total + millis);
        let since = if since == 0.0 { now } else { since };
        if now - since >= 2_000.0 {
            log(&format!(
                "Gravak login: {:.1} fps, {:.1} ms to paint a frame",
                f64::from(frames) * 1_000.0 / (now - since),
                total / f64::from(frames)
            ));
            painted.set((0, 0.0, now));
        } else {
            painted.set((frames, total, since));
        }
    });
}
