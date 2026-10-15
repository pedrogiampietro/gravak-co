//! The browser's game files (docs/web-spike.md): one store for every
//! `Assets` of the page, filled from the site's `/game-files`
//! (`crates/web-files`), which hands the pack's entries out decrypted to a
//! signed-in account.
//!
//! Reading stays synchronous: a file not here yet is asked for and the read
//! fails as `Pending`; whoever read it tries again once `misses` stops
//! growing and the store is not `busy`. A file the pack lacks is remembered
//! as absent and fails for good, as it would natively.
//!
//! What comes is kept in the browser's Cache Storage, in a cache named after
//! the pack's version (`X-Gravak-Pack`): the next visit takes its files from
//! there, and a new pack starts a new cache and drops the old.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

const URL: &str = "/game-files";
/// Paths asked in one request (the service takes up to 4096).
const BATCH: usize = 1500;
/// Preloaded paths asked in one request: small, so a preload list of a
/// whole map (tens of MB) never holds the other requests long.
const PRELOAD_BATCH: usize = 150;
const MISSING: u32 = u32::MAX;
const LATER: u32 = u32::MAX - 1;

#[derive(Default)]
struct Store {
    files: HashMap<String, Vec<u8>>,
    absent: HashSet<String>,
    /// Read and missed: asked first, `busy` until they are in.
    urgent: Vec<String>,
    /// From the preload lists (`want`): asked a little at a time beside the
    /// urgent ones; a read of one of them makes it urgent.
    preload: VecDeque<String>,
    asked: HashSet<String>,
    urgent_in_flight: bool,
    preload_in_flight: bool,
    /// The preload request in flight, and those of its files something has
    /// read since: `busy` waits for them as for urgent ones.
    preload_batch: HashSet<String>,
    awaited: HashSet<String>,
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
            store.urgent.push(name);
        } else if let Some(index) = store.preload.iter().position(|queued| *queued == name) {
            // Wanted now, not some day: ahead of the preload.
            store.preload.remove(index);
            store.urgent.push(name);
        } else if store.preload_batch.contains(&name) {
            store.awaited.insert(name);
        }
        Lookup::Pending
    })
}

/// Asks for `names` before anything reads them: the preload list.
pub fn want(names: impl IntoIterator<Item = String>) {
    STORE.with(|store| {
        let mut store = store.borrow_mut();
        for name in names {
            let name = key(&name);
            if !store.files.contains_key(&name) && store.asked.insert(name.clone()) {
                store.preload.push_back(name);
            }
        }
    });
}

/// How many reads have come up empty so far.
pub fn misses() -> u64 {
    STORE.with(|store| store.borrow().misses)
}

/// Whether files something read are still wanted or on their way (the
/// preload lists do not count).
pub fn busy() -> bool {
    STORE.with(|store| {
        let store = store.borrow();
        store.urgent_in_flight || !store.urgent.is_empty() || !store.awaited.is_empty()
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

/// Sends the wanted paths, a batch at a time: the urgent ones and the
/// preloaded ones side by side, one request of each in flight. Called every
/// frame.
pub fn pump() {
    for urgent in [true, false] {
        let batch = STORE.with(|store| {
            let mut store = store.borrow_mut();
            if store.unauthorized {
                return None;
            }
            if urgent {
                if store.urgent_in_flight || store.urgent.is_empty() {
                    return None;
                }
                let count = store.urgent.len().min(BATCH);
                store.urgent_in_flight = true;
                Some(store.urgent.drain(..count).collect::<Vec<_>>())
            } else {
                if store.preload_in_flight || store.preload.is_empty() {
                    return None;
                }
                let count = store.preload.len().min(PRELOAD_BATCH);
                store.preload_in_flight = true;
                let batch: Vec<String> = store.preload.drain(..count).collect();
                store.preload_batch = batch.iter().cloned().collect();
                Some(batch)
            }
        });
        if let Some(batch) = batch {
            wasm_bindgen_futures::spawn_local(fetch_and_keep(batch, urgent));
        }
    }
}

async fn fetch_and_keep(batch: Vec<String>, urgent: bool) {
    let result = fetch_batch(&batch).await;
    STORE.with(|store| {
        let mut store = store.borrow_mut();
        if urgent {
            store.urgent_in_flight = false;
        } else {
            store.preload_in_flight = false;
            store.preload_batch.clear();
        }
        let mut again = Vec::new();
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
                        None => again.push(name),
                    }
                }
            }
            Ok(Answer::Unauthorized) => {
                store.unauthorized = true;
                again = batch;
            }
            Err(error) => {
                crate::web::log(&format!("Gravak: game files: {error}"));
                store.failure = Some(format!("The game files could not be downloaded ({error})."));
                again = batch;
            }
        }
        if urgent {
            store.urgent.extend(again);
        } else {
            // What was read meanwhile and did not come goes ahead.
            let (read, later): (Vec<String>, Vec<String>) =
                again.into_iter().partition(|name| store.awaited.contains(name));
            store.urgent.extend(read);
            store.preload.extend(later);
            store.awaited.clear();
        }
    });
}

/// The browser's cache of the files, once known.
enum Kept {
    Unknown,
    Cache(web_sys::Cache),
    /// No Cache Storage (an insecure page, a private window that refuses).
    Nothing,
}

thread_local! {
    static KEPT: RefCell<Kept> = const { RefCell::new(Kept::Unknown) };
}

const CACHE_PREFIX: &str = "gravak-files-";

fn cache_key(name: &str) -> String {
    format!("/game-files/kept/{}", String::from(js_sys::encode_uri(name)))
}

/// The cache of this pack's version, the older ones dropped.
async fn open_cache(version: &str) -> Option<web_sys::Cache> {
    let caches = web_sys::window()?.caches().ok()?;
    let name = format!("{CACHE_PREFIX}{version}");
    if let Ok(keys) = JsFuture::from(caches.keys()).await {
        for key in js_sys::Array::from(&keys).iter().filter_map(|key| key.as_string()) {
            if key.starts_with(CACHE_PREFIX) && key != name {
                let _ = JsFuture::from(caches.delete(&key)).await;
            }
        }
    }
    JsFuture::from(caches.open(&name)).await.ok()?.dyn_into().ok()
}

/// The files of `batch`: those kept from before, the rest from the site
/// (and kept for next time).
async fn fetch_batch(batch: &[String]) -> Result<Answer, String> {
    if KEPT.with(|kept| matches!(*kept.borrow(), Kept::Unknown)) {
        let (answer, version) = request(&[]).await?;
        if let Answer::Unauthorized = answer {
            return Ok(answer);
        }
        let cache = match version {
            Some(version) => open_cache(&version).await,
            None => None,
        };
        KEPT.with(|kept| *kept.borrow_mut() = cache.map_or(Kept::Nothing, Kept::Cache));
    }
    let cache = KEPT.with(|kept| match &*kept.borrow() {
        Kept::Cache(cache) => Some(cache.clone()),
        _ => None,
    });
    let mut answers: Vec<Option<Option<Vec<u8>>>> = vec![None; batch.len()];
    if let Some(cache) = &cache {
        let lookups = js_sys::Array::new();
        for name in batch {
            lookups.push(&cache.match_with_str(&cache_key(name)));
        }
        if let Ok(found) = JsFuture::from(js_sys::Promise::all(&lookups)).await {
            let reads = js_sys::Array::new();
            for found in js_sys::Array::from(&found).iter() {
                match found.dyn_into::<web_sys::Response>() {
                    Ok(response) => match response.array_buffer() {
                        Ok(promise) => reads.push(&promise),
                        Err(_) => reads.push(&JsValue::UNDEFINED),
                    },
                    Err(_) => reads.push(&JsValue::UNDEFINED),
                };
            }
            if let Ok(buffers) = JsFuture::from(js_sys::Promise::all(&reads)).await {
                for (index, buffer) in js_sys::Array::from(&buffers).iter().enumerate() {
                    if buffer.is_instance_of::<js_sys::ArrayBuffer>() {
                        answers[index] = Some(Some(js_sys::Uint8Array::new(&buffer).to_vec()));
                    }
                }
            }
        }
    }
    let wanted: Vec<usize> = (0..batch.len()).filter(|&index| answers[index].is_none()).collect();
    if cache.is_some() && wanted.len() < batch.len() {
        crate::web::log(&format!("Gravak: {} game files from the browser's cache", batch.len() - wanted.len()));
    }
    if wanted.is_empty() {
        return Ok(Answer::Files(answers));
    }
    let names: Vec<String> = wanted.iter().map(|&index| batch[index].clone()).collect();
    match request(&names).await?.0 {
        Answer::Unauthorized => Ok(Answer::Unauthorized),
        Answer::Files(fetched) => {
            for (index, answer) in wanted.into_iter().zip(fetched) {
                if let (Some(cache), Some(Some(bytes))) = (&cache, &answer) {
                    let mut copy = bytes.clone();
                    if let Ok(response) = web_sys::Response::new_with_opt_u8_array(Some(&mut copy)) {
                        // Kept in the background; a full disk only costs the keeping.
                        let _ = cache.put_with_str(&cache_key(&batch[index]), &response);
                    }
                }
                answers[index] = answer;
            }
            Ok(Answer::Files(answers))
        }
    }
}

enum Answer {
    /// Per path: the bytes, `Some(None)` absent, `None` ask again.
    Files(Vec<Option<Option<Vec<u8>>>>),
    Unauthorized,
}

/// The site's answer to `batch`, and the pack's version it names.
async fn request(batch: &[String]) -> Result<(Answer, Option<String>), String> {
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
        return Ok((Answer::Unauthorized, None));
    }
    let version = response.headers().get("X-Gravak-Pack").ok().flatten();
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    let buffer = JsFuture::from(response.array_buffer().map_err(failed)?)
        .await
        .map_err(failed)?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
    parse(&bytes, batch.len()).map(|files| (Answer::Files(files), version))
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
