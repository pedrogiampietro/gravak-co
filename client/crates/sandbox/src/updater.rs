//! Keeps an installed client current: before the login, it reads the
//! update manifest from the address in `gravak.ini` (`Update=`), downloads
//! every file whose size or SHA-256 differs from the one installed, and
//! restarts the client on the new files.
//!
//! The manifest (`manifest.txt`, made by `tools/publish-update.ps1`) sits
//! beside the files it names, which keep their installed paths:
//!
//! ```text
//! version=0.1.1
//! file=Gravak.exe|12563968|<sha256>
//! file=servers.ini|120|<sha256>
//! file=game/gravak.gpk|436431808|<sha256>
//! ```
//!
//! A file is downloaded beside its place (`.download`), checked against
//! its hash, and only then swapped in; the one it replaces is moved aside
//! (`.old`, removed on the next run), which Windows allows even for the
//! running executable and the open pack.

use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant, UNIX_EPOCH},
};

pub const MANIFEST: &str = "manifest.txt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Relative to the install folder, with `/`.
    pub path: String,
    pub size: u64,
    /// Lowercase hexadecimal.
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Manifest {
    pub version: String,
    pub files: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateEvent {
    Checking,
    Downloading {
        version: String,
        file: String,
        /// Bytes of all the files to download.
        done: u64,
        total: u64,
    },
    UpToDate,
    /// New files are in place; the client must start again on them.
    Updated(String),
    Failed(String),
}

/// Starts checking `base` (the folder URL of the manifest) for the client
/// installed in `root`.
pub fn start(base: String, root: PathBuf) -> Receiver<UpdateEvent> {
    let (sender, receiver) = mpsc::channel();
    let _ = std::thread::Builder::new().name("gravak-updater".into()).spawn(move || {
        let _ = sender.send(UpdateEvent::Checking);
        let event = match update(&base, &root, &sender) {
            Ok(Some(version)) => UpdateEvent::Updated(version),
            Ok(None) => UpdateEvent::UpToDate,
            Err(error) => UpdateEvent::Failed(error),
        };
        let _ = sender.send(event);
    });
    receiver
}

/// Starts the client again with the same arguments, for after an update.
pub fn restart() -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    std::process::Command::new(exe).args(std::env::args_os().skip(1)).spawn().map(drop)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_body(Some(Duration::from_secs(60)))
        .user_agent(concat!("Gravak/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn url(base: &str, path: &str) -> String {
    format!("{}/{}", base.trim_end_matches('/'), path)
}

fn update(base: &str, root: &Path, events: &Sender<UpdateEvent>) -> Result<Option<String>, String> {
    let agent = agent();
    let text = agent
        .get(&url(base, MANIFEST))
        .call()
        .map_err(|error| format!("update server unavailable ({error})"))?
        .body_mut()
        .read_to_string()
        .map_err(|error| format!("update server unavailable ({error})"))?;
    let manifest = parse(&text)?;
    let mut cache = HashCache::load(root);
    let mut stale = Vec::new();
    for entry in &manifest.files {
        let place = root.join(&entry.path);
        // What an earlier update moved aside.
        let _ = std::fs::remove_file(aside(&place));
        if cache.hash(&place, entry.size).as_deref() != Some(entry.sha256.as_str()) {
            stale.push(entry);
        }
    }
    cache.save();
    if stale.is_empty() {
        return Ok(None);
    }
    let total = stale.iter().map(|entry| entry.size).sum();
    let mut done = 0;
    let mut downloaded = Vec::new();
    for entry in stale {
        let place = root.join(&entry.path);
        let partial = with_suffix(&place, ".download");
        download(&agent, base, entry, &partial, |bytes| {
            let _ = events.send(UpdateEvent::Downloading {
                version: manifest.version.clone(),
                file: entry.path.clone(),
                done: done + bytes,
                total,
            });
        })?;
        done += entry.size;
        downloaded.push((partial, place));
    }
    // Every file is here and sound: swap them all in.
    for (partial, place) in downloaded {
        if place.exists() {
            let _ = std::fs::remove_file(aside(&place));
            std::fs::rename(&place, aside(&place))
                .map_err(|error| format!("could not replace {} ({error})", place.display()))?;
        }
        std::fs::rename(&partial, &place)
            .map_err(|error| format!("could not install {} ({error})", place.display()))?;
    }
    Ok(Some(manifest.version))
}

fn download(
    agent: &ureq::Agent,
    base: &str,
    entry: &Entry,
    partial: &Path,
    mut progress: impl FnMut(u64),
) -> Result<(), String> {
    let failed = |error: &dyn std::fmt::Display| format!("could not download {} ({error})", entry.path);
    if let Some(dir) = partial.parent() {
        std::fs::create_dir_all(dir).map_err(|error| failed(&error))?;
    }
    let mut response = agent.get(&url(base, &entry.path)).call().map_err(|error| failed(&error))?;
    let mut body = response.body_mut().with_config().limit(entry.size + 1).reader();
    let mut file = std::fs::File::create(partial).map_err(|error| failed(&error))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 256 * 1024];
    let mut read = 0u64;
    let mut reported = Instant::now();
    loop {
        let count = body.read(&mut buffer).map_err(|error| failed(&error))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        file.write_all(&buffer[..count]).map_err(|error| failed(&error))?;
        read += count as u64;
        if reported.elapsed() > Duration::from_millis(100) {
            reported = Instant::now();
            progress(read);
        }
    }
    file.flush().map_err(|error| failed(&error))?;
    drop(file);
    if read != entry.size || hex(&hasher.finalize()) != entry.sha256 {
        let _ = std::fs::remove_file(partial);
        return Err(format!("{} arrived damaged; try again", entry.path));
    }
    progress(read);
    Ok(())
}

pub fn parse(text: &str) -> Result<Manifest, String> {
    let mut manifest = Manifest::default();
    for line in text.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "version" => manifest.version = value.trim().to_owned(),
            "file" => {
                let parts: Vec<&str> = value.split('|').map(str::trim).collect();
                let [path, size, sha256] = parts[..] else {
                    return Err(format!("bad manifest line: {line}"));
                };
                let path = path.replace('\\', "/");
                if !safe(&path) {
                    return Err(format!("manifest names a file outside the game: {path}"));
                }
                let sha256 = sha256.to_ascii_lowercase();
                if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return Err(format!("bad hash for {path}"));
                }
                let size = size.parse().map_err(|_| format!("bad size for {path}"))?;
                manifest.files.push(Entry { path, size, sha256 });
            }
            _ => {}
        }
    }
    if manifest.files.is_empty() {
        return Err("the update manifest lists no files".into());
    }
    Ok(manifest)
}

/// A relative path that stays inside the install folder.
fn safe(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path).components().all(|component| matches!(component, Component::Normal(_)))
        && !path.contains(':')
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn aside(path: &Path) -> PathBuf {
    with_suffix(path, ".old")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The installed files' hashes by size and modification time
/// (`settings/files.cache`), so the 400 MB pack is hashed once, not on
/// every start.
struct HashCache {
    path: PathBuf,
    entries: std::collections::HashMap<String, (u64, u64, String)>,
}

impl HashCache {
    fn load(root: &Path) -> Self {
        let path = root.join("settings").join("files.cache");
        let mut entries = std::collections::HashMap::new();
        for line in std::fs::read_to_string(&path).unwrap_or_default().lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if let [name, size, modified, sha] = parts[..]
                && let (Ok(size), Ok(modified)) = (size.parse(), modified.parse())
            {
                entries.insert(name.to_owned(), (size, modified, sha.to_owned()));
            }
        }
        Self { path, entries }
    }

    /// The file's SHA-256, or `None` when it is missing or not `size` long.
    fn hash(&mut self, file: &Path, size: u64) -> Option<String> {
        let meta = std::fs::metadata(file).ok()?;
        if meta.len() != size {
            return None;
        }
        let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
        let key = file.to_string_lossy().into_owned();
        if let Some((cached_size, cached_modified, sha)) = self.entries.get(&key)
            && *cached_size == size
            && *cached_modified == modified
        {
            return Some(sha.clone());
        }
        let mut reader = std::fs::File::open(file).ok()?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut reader, &mut hasher).ok()?;
        let sha = hex(&hasher.finalize());
        self.entries.insert(key, (size, modified, sha.clone()));
        Some(sha)
    }

    fn save(&self) {
        let text: String = self
            .entries
            .iter()
            .map(|(name, (size, modified, sha))| format!("{name}|{size}|{modified}|{sha}\n"))
            .collect();
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&self.path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn reads_the_manifest_and_refuses_paths_outside() {
        let manifest = parse(&format!("; made by publish-update\nversion=0.1.1\nfile=Gravak.exe|0|{HASH}\nfile=game\\gravak.gpk|0|{}\n", HASH.to_uppercase())).unwrap();
        assert_eq!(manifest.version, "0.1.1");
        assert_eq!(manifest.files[1].path, "game/gravak.gpk");
        assert_eq!(manifest.files[1].sha256, HASH);
        for bad in ["../evil.exe", "C:/Windows/x.dll", "/etc/x", "game/../../x"] {
            assert!(parse(&format!("file={bad}|0|{HASH}")).is_err(), "{bad}");
        }
        assert!(parse("version=1").is_err());
    }

    /// Serves `files` over HTTP on a local port, for as many requests as
    /// asked.
    fn serve(files: Vec<(String, Vec<u8>)>, requests: usize) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(requests) {
                let mut stream = stream.unwrap();
                let mut request = [0; 2048];
                let count = stream.read(&mut request).unwrap();
                let line = String::from_utf8_lossy(&request[..count]);
                let path = line.split_whitespace().nth(1).unwrap_or("/").trim_start_matches("/up/").to_owned();
                match files.iter().find(|(name, _)| *name == path) {
                    Some((_, body)) => {
                        let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                        let _ = stream.write_all(body);
                    }
                    None => {
                        let _ = write!(stream, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    }
                }
            }
        });
        format!("http://{address}/up/")
    }

    fn sha(bytes: &[u8]) -> String {
        hex(&Sha256::digest(bytes))
    }

    #[test]
    fn downloads_what_changed_and_swaps_it_in() {
        let root = std::env::temp_dir().join(format!("gravak-updater-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("game")).unwrap();
        std::fs::write(root.join("servers.ini"), b"[Gravak]\nAuth=old\n").unwrap();
        std::fs::write(root.join("game/gravak.gpk"), b"same pack").unwrap();

        let servers = b"[Gravak]\nAuth=new:9958\n".to_vec();
        let manifest = format!(
            "version=0.2.0\nfile=servers.ini|{}|{}\nfile=game/gravak.gpk|9|{}\n",
            servers.len(),
            sha(&servers),
            sha(b"same pack")
        );
        let base = serve(vec![(MANIFEST.into(), manifest.into_bytes()), ("servers.ini".into(), servers.clone())], 2);
        let (sender, events) = mpsc::channel();
        assert_eq!(update(&base, &root, &sender), Ok(Some("0.2.0".into())));
        assert_eq!(std::fs::read(root.join("servers.ini")).unwrap(), servers);
        assert!(root.join("servers.ini.old").exists());
        // Only the changed file came down.
        let progress: Vec<_> = events.try_iter().collect();
        assert!(progress.iter().all(|event| matches!(event, UpdateEvent::Downloading { file, .. } if file == "servers.ini")));

        // A damaged download is refused and the installed file kept.
        let manifest = format!("version=0.3.0\nfile=servers.ini|3|{HASH}\n");
        let base = serve(vec![(MANIFEST.into(), manifest.into_bytes()), ("servers.ini".into(), b"bad".to_vec())], 2);
        assert!(update(&base, &root, &sender).unwrap_err().contains("damaged"));
        assert_eq!(std::fs::read(root.join("servers.ini")).unwrap(), servers);
        assert!(!root.join("servers.ini.old").exists(), "the next run clears what was moved aside");
        let _ = std::fs::remove_dir_all(&root);
    }
}
