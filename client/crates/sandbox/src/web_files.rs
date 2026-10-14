//! The browser's game files (docs/web-spike.md): one store for every
//! `Assets` of the page, filled from the site's `/game-files`
//! (`crates/web-files`), which hands the pack's entries out decrypted to a
//! signed-in account.
//!
//! Reading stays synchronous: a file not here yet is asked for and the read
//! fails as `Pending`; whoever read it tries again once `misses` stops
//! growing and the store is not `busy`. A file the pack lacks is remembered
//! as absent and fails for good, as it would natively.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

const URL: &str = "/game-files";
/// Paths asked in one request (the service takes up to 4096).
const BATCH: usize = 1500;
const MISSING: u32 = u32::MAX;
const LATER: u32 = u32::MAX - 1;

#[derive(Default)]
struct Store {
    files: HashMap<String, Vec<u8>>,
    absent: HashSet<String>,
    wanted: Vec<String>,
    asked: HashSet<String>,
    in_flight: bool,
    /// Reads that found nothing yet, ever: a job that raised it must wait.
    misses: u64,
    fetched_files: usize,
    fetched_bytes: usize,
    /// The site said the page is not signed in.
    unauthorized: bool,
    failure: Option<String>,
}

thread_local! {
    static STORE: RefCell<Store> = RefCell::new(Store::default());
}

pub enum Lookup {
    Found(Vec<u8>),
    Absent,
    Pending,
}

fn key(name: &str) -> String {
    name.replace('\\', "/").to_ascii_lowercase()
}

pub fn insert(name: &str, bytes: Vec<u8>) {
    STORE.with(|store| {
        store.borrow_mut().files.insert(key(name), bytes);
    });
}

pub fn read(name: &str) -> Lookup {
    let name = key(name);
    STORE.with(|store| {
        let mut store = store.borrow_mut();
        if let Some(bytes) = store.files.get(&name) {
            return Lookup::Found(bytes.clone());
        }
        if store.absent.contains(&name) {
            return Lookup::Absent;
        }
        store.misses += 1;
        if store.asked.insert(name.clone()) {
            store.wanted.push(name);
        }
        Lookup::Pending
    })
}

/// How many reads have come up empty so far.
pub fn misses() -> u64 {
    STORE.with(|store| store.borrow().misses)
}

/// Whether files are still wanted or on their way.
pub fn busy() -> bool {
    STORE.with(|store| {
        let store = store.borrow();
        store.in_flight || !store.wanted.is_empty()
    })
}

/// What stops the files coming, for the screen.
pub fn problem() -> Option<String> {
    STORE.with(|store| {
        let store = store.borrow();
        if store.unauthorized {
            Some("Sign in on the site to play in the browser.".into())
        } else {
            store.failure.clone()
        }
    })
}

/// Files and KiB fetched so far.
pub fn fetched() -> (usize, usize) {
    STORE.with(|store| {
        let store = store.borrow();
        (store.fetched_files, store.fetched_bytes / 1024)
    })
}

/// Sends the wanted paths, a batch at a time; called every frame.
pub fn pump() {
    let batch = STORE.with(|store| {
        let mut store = store.borrow_mut();
        if store.in_flight || store.wanted.is_empty() || store.unauthorized {
            return None;
        }
        let count = store.wanted.len().min(BATCH);
        let batch: Vec<String> = store.wanted.drain(..count).collect();
        store.in_flight = true;
        Some(batch)
    });
    let Some(batch) = batch else { return };
    wasm_bindgen_futures::spawn_local(async move {
        let result = request(&batch).await;
        STORE.with(|store| {
            let mut store = store.borrow_mut();
            store.in_flight = false;
            match result {
                Ok(Answer::Files(answers)) => {
                    store.failure = None;
                    for (name, answer) in batch.into_iter().zip(answers) {
                        match answer {
                            Some(Some(bytes)) => {
                                store.fetched_files += 1;
                                store.fetched_bytes += bytes.len();
                                store.files.insert(name, bytes);
                            }
                            Some(None) => {
                                store.absent.insert(name);
                            }
                            // Over the answer's size: asked again.
                            None => store.wanted.push(name),
                        }
                    }
                }
                Ok(Answer::Unauthorized) => {
                    store.unauthorized = true;
                    store.wanted.extend(batch);
                }
                Err(error) => {
                    crate::web::log(&format!("Gravak: game files: {error}"));
                    store.failure = Some(format!("The game files could not be downloaded ({error})."));
                    store.wanted.extend(batch);
                }
            }
        });
    });
}

enum Answer {
    /// Per path: the bytes, `Some(None)` absent, `None` ask again.
    Files(Vec<Option<Option<Vec<u8>>>>),
    Unauthorized,
}

async fn request(batch: &[String]) -> Result<Answer, String> {
    let failed = |error: JsValue| format!("{error:?}");
    let window = web_sys::window().ok_or("no window")?;
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_body(&JsValue::from_str(&batch.join("\n")));
    init.set_credentials(web_sys::RequestCredentials::SameOrigin);
    let response: web_sys::Response = JsFuture::from(window.fetch_with_str_and_init(URL, &init))
        .await
        .map_err(failed)?
        .dyn_into()
        .map_err(failed)?;
    if response.status() == 401 {
        return Ok(Answer::Unauthorized);
    }
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    let buffer = JsFuture::from(response.array_buffer().map_err(failed)?)
        .await
        .map_err(failed)?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
    parse(&bytes, batch.len()).map(Answer::Files)
}

fn parse(mut bytes: &[u8], count: usize) -> Result<Vec<Option<Option<Vec<u8>>>>, String> {
    let mut answers = Vec::with_capacity(count);
    for _ in 0..count {
        if bytes.len() < 4 {
            return Err("the answer is cut short".into());
        }
        let length = u32::from_le_bytes(bytes[..4].try_into().expect("4 bytes"));
        bytes = &bytes[4..];
        answers.push(match length {
            MISSING => Some(None),
            LATER => None,
            length => {
                let length = length as usize;
                if bytes.len() < length {
                    return Err("the answer is cut short".into());
                }
                let (file, rest) = bytes.split_at(length);
                bytes = rest;
                Some(Some(file.to_vec()))
            }
        });
    }
    Ok(answers)
}
