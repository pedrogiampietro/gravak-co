//! The browser build's start (docs/web-spike.md): the page loads the wasm,
//! this fetches the game files the login screens need (listed in
//! `assets/manifest.txt` beside the page, made by tools/build-web.ps1) and
//! shows the screens on a canvas. The sessions reach the servers through
//! the WebSocket bridge (`crates/ws-bridge`). Entering the world, the game
//! files come from the site (`web_files`): the world is made over and over
//! until no file it reads is still on its way, then it takes the page.

use crate::assets::Assets;
use std::cell::Cell;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    platform::web::EventLoopExtWebSys,
    window::WindowId,
};
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
    // What entering any world reads, fetched while the login screens show
    // (web/world.txt): one request instead of one a file.
    want_listed("assets/world.txt").await;
    // The account's settings, before the screens read the login's.
    crate::saved::pull().await;
    watch_page_close();
    let screens = crate::launcher::prepare_web(&mut assets, worlds);
    winit::event_loop::EventLoop::new()
        .map_err(|error| error.to_string())?
        .spawn_app(Game::Screens(screens, None));
    Ok(())
}

/// Asks for the game files a preload list names (web/world.txt,
/// web/bodyN.txt), all in one go.
async fn want_listed(url: &str) {
    if let Ok(list) = fetch(url).await {
        crate::web_files::want(
            String::from_utf8_lossy(&list)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(str::to_owned),
        );
    }
}

/// Settings still settling go up as the page closes.
fn watch_page_close() {
    let Some(window) = web_sys::window() else { return };
    let send = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(|| crate::saved::flush(true));
    let _ = window.add_event_listener_with_callback("pagehide", send.as_ref().unchecked_ref());
    send.forget();
}

/// Asks for a map's files (web/maps/<id>.txt), the hero entering it.
pub fn want_map(map_id: u32) {
    wasm_bindgen_futures::spawn_local(async move { want_listed(&format!("assets/maps/{map_id}.txt")).await });
}

/// The page's one event loop: the login screens, then the world.
enum Game {
    Screens(crate::launcher::Launcher, Option<Entering>),
    World(Box<crate::App>),
    /// Between the two, for the moment of the swap.
    Gone,
}

/// A character in, the world not made yet.
struct Entering {
    session: crate::network::NetworkHandle,
    ready: Option<crate::network::ReadyState>,
    started: web_time::Instant,
}

/// How long the game server may take to say where the hero is.
const READY_WITHIN: web_time::Duration = web_time::Duration::from_secs(20);

impl Game {
    /// The world, if every file it reads is here; `Err` when it cannot be
    /// made for another reason than a file on its way.
    fn try_world(ready: &crate::network::ReadyState) -> Result<Option<crate::App>, String> {
        let misses = crate::web_files::misses();
        let built = crate::build_world(
            crate::assets::Assets::from_memory(std::iter::empty()),
            crate::WorldStart {
                map_id: Some(ready.map_id),
                body: ready.body.clamp(1, 4),
                spawn: None,
                check: false,
                online: true,
                network: None,
                server_ready: Some(ready.clone()),
                login_window: None,
            },
        );
        if crate::web_files::misses() != misses {
            return Ok(None);
        }
        built.map_err(|error| error.to_string())
    }

    fn step_entering(&mut self, event_loop: &ActiveEventLoop) {
        let Game::Screens(screens, entering) = self else { return };
        if entering.is_none()
            && let Some(session) = screens.take_entered()
        {
            *entering = Some(Entering { session, ready: None, started: web_time::Instant::now() });
        }
        let Some(state) = entering else { return };
        crate::web_files::pump();
        if state.ready.is_none() {
            match state.session.try_ready() {
                Some(Ok(ready)) => {
                    // As natively: the nobility is asked for again.
                    state.session.request_nobility_status();
                    // The hero's map's files (web/maps/<id>.txt), and its
                    // body's (web/bodyN.txt).
                    want_map(ready.map_id);
                    if (1..=4).contains(&ready.body) {
                        let url = format!("assets/body{}.txt", ready.body);
                        wasm_bindgen_futures::spawn_local(async move { want_listed(&url).await });
                    }
                    state.ready = Some(ready);
                }
                Some(Err(error)) => {
                    screens.fail(error);
                    *entering = None;
                    return;
                }
                None if state.started.elapsed() > READY_WITHIN => {
                    screens.fail("Timed out waiting for the game server.");
                    *entering = None;
                    return;
                }
                None => return,
            }
        }
        if let Some(problem) = crate::web_files::problem() {
            screens.tell(problem);
            return;
        }
        if crate::web_files::busy() {
            let (files, kib) = crate::web_files::fetched();
            screens.tell(format!("Downloading the world... {files} files, {:.1} MB", kib as f64 / 1024.0));
            return;
        }
        let ready = state.ready.as_ref().expect("ready above");
        match Self::try_world(ready) {
            Ok(None) => {}
            Err(error) => {
                log(&format!("Gravak: the world could not open: {error}"));
                screens.fail(format!("The world could not open: {error}"));
                *entering = None;
            }
            Ok(Some(mut app)) => {
                let Game::Screens(screens, Some(state)) = std::mem::replace(self, Game::Gone) else {
                    unreachable!("entering above");
                };
                screens.close();
                app.network = Some(state.session);
                let (files, kib) = crate::web_files::fetched();
                log(&format!("Gravak: the world opens ({files} game files, {kib} KiB)"));
                app.resumed(event_loop);
                // The keys go to the world's canvas, new on the page.
                if let Some(canvas) = app.window.as_ref().and_then(|window| {
                    use winit::platform::web::WindowExtWebSys;
                    window.canvas()
                }) {
                    let _ = canvas.focus();
                }
                *self = Game::World(Box::new(app));
            }
        }
    }
}

impl ApplicationHandler for Game {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        match self {
            Game::Screens(screens, _) => screens.resumed(event_loop),
            Game::World(app) => app.resumed(event_loop),
            Game::Gone => {}
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match self {
            Game::Screens(screens, _) => screens.window_event(event_loop, id, event),
            Game::World(app) => app.window_event(event_loop, id, event),
            Game::Gone => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        crate::saved::flush(false);
        match self {
            Game::Screens(screens, _) => {
                screens.about_to_wait(event_loop);
                crate::web_files::pump();
                self.step_entering(event_loop);
            }
            Game::World(app) => {
                app.about_to_wait(event_loop);
                world_report(app);
                if let Some(error) = &app.error {
                    log(&format!("Gravak: {error}"));
                    show_error(error);
                    event_loop.exit();
                }
            }
            Game::Gone => {}
        }
    }
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
    static REPORTED: Cell<f64> = const { Cell::new(0.0) };
}

/// Every five seconds, what of the world is on the page: the actors the
/// server sent, the models made of them and those still loading, and the
/// game files fetched.
fn world_report(app: &crate::App) {
    let now = js_sys::Date::now();
    if now - REPORTED.with(Cell::get) < 5_000.0 {
        return;
    }
    REPORTED.with(|reported| reported.set(now));
    let (files, kib) = crate::web_files::fetched();
    log(&format!(
        "Gravak world: {} actors; models: {} players, {} NPCs, {} monsters; loading: {} players, {} NPCs, {} monsters; {} files ({} KiB){}",
        app.remote_actors.len(),
        app.remote_models.len(),
        app.remote_npcs.len(),
        app.remote_monsters.len(),
        app.remote_models_loading.len(),
        app.remote_npcs_loading.len(),
        app.remote_monsters_loading.len(),
        files,
        kib,
        if crate::web_files::busy() { ", downloading" } else { "" },
    ));
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
