//! gravak-web-files: the game files of the browser client (docs/web-spike.md).
//! The players' pack (`gravak.gpk`) stays encrypted on disk; this hands its
//! entries out decrypted, in batches, to whoever carries the site's session
//! cookie (`gravak_session`, signed with the site's `SESSION_SECRET`, see
//! web/src/lib/session.ts). The pack key never reaches the browser.
//!
//!   SESSION_SECRET=... gravak-web-files --pack <gravak.gpk> --key-file <pack.key>
//!                      [--listen 127.0.0.1:9961]
//!
//! Behind nginx at `/game-files`. One request: `POST` with the virtual paths
//! the client wants, one per line. The answer, in the same order, is for each
//! path a little-endian u32 and that many bytes; `MISSING` means the pack has
//! no such file, `LATER` that the answer was full and the path must be asked
//! again. Every answer carries `X-Gravak-Pack`, the pack's version (its
//! size and time), so the browser can keep the files until the pack changes;
//! a request without paths only asks for it.
//!
//! The session is checked by its signature and expiry only: a ban reaches the
//! game through the auth server, which the browser logs in to as well.

use hmac::{Hmac, Mac};
use sandbox_data::{VirtualPath, pack::PackReader, wdf::path_id};
use sha2::Sha256;
use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

const MISSING: u32 = u32::MAX;
const LATER: u32 = u32::MAX - 1;
/// The most paths asked in one request.
const MOST_PATHS: usize = 4096;
/// The most file bytes in one answer; past it the rest is `LATER`.
const MOST_BYTES: usize = 48 * 1024 * 1024;
const MOST_HEAD: usize = 16 * 1024;
const MOST_BODY: usize = 512 * 1024;
const COOKIE: &str = "gravak_session";

struct Options {
    listen: SocketAddr,
    pack: PathBuf,
    key_file: PathBuf,
}

struct Server {
    pack: Mutex<PackReader>,
    secret: Vec<u8>,
    /// `X-Gravak-Pack`: the pack's size and modification time.
    version: String,
}

fn options() -> Result<Options> {
    let mut listen = "127.0.0.1:9961".parse()?;
    let mut pack = None;
    let mut key_file = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} requires a value"));
        match arg.as_str() {
            "--listen" => listen = value()?.parse()?,
            "--pack" => pack = Some(PathBuf::from(value()?)),
            "--key-file" => key_file = Some(PathBuf::from(value()?)),
            "--help" | "-h" => {
                println!("gravak-web-files --pack <gravak.gpk> --key-file <pack.key> [--listen 127.0.0.1:9961]");
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument {arg}; see --help").into()),
        }
    }
    Ok(Options {
        listen,
        pack: pack.ok_or("--pack is required")?,
        key_file: key_file.ok_or("--key-file is required")?,
    })
}

/// The pack key file: 64 hexadecimal digits, as `installer/pack.key`.
fn read_key(path: &PathBuf) -> Result<[u8; 32]> {
    let text = std::fs::read_to_string(path)?;
    let text = text.trim().trim_start_matches('\u{feff}');
    if text.len() != 64 {
        return Err(format!("{} must hold 64 hexadecimal digits", path.display()).into());
    }
    let mut key = [0; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)?;
    }
    Ok(key)
}

#[tokio::main]
async fn main() {
    if let Err(error) = start().await {
        eprintln!("gravak-web-files: {error}");
        std::process::exit(1);
    }
}

async fn start() -> Result<()> {
    let options = options()?;
    let secret = std::env::var("SESSION_SECRET").map_err(|_| "SESSION_SECRET is not set")?;
    let key = read_key(&options.key_file)?;
    let pack = PackReader::open(&options.pack, &key)
        .map_err(|error| format!("{}: {error}", options.pack.display()))?;
    let entries = pack.len();
    let metadata = std::fs::metadata(&options.pack)?;
    let modified = metadata.modified()?.duration_since(UNIX_EPOCH).map(|time| time.as_secs()).unwrap_or(0);
    let version = format!("{}-{modified}", metadata.len());
    let server = Arc::new(Server { pack: Mutex::new(pack), secret: secret.into_bytes(), version });
    let listener = TcpListener::bind(options.listen)
        .await
        .map_err(|error| format!("cannot listen on {}: {error}", options.listen))?;
    println!("gravak-web-files on {}: {} entries of {}", options.listen, entries, options.pack.display());
    loop {
        let Ok((stream, peer)) = listener.accept().await else { continue };
        let server = server.clone();
        tokio::spawn(async move {
            if let Err(error) = serve(stream, &server).await {
                println!("{peer}: {error}");
            }
        });
    }
}

struct Request {
    method: String,
    head: String,
    body: Vec<u8>,
}

async fn read_request(stream: &mut TcpStream) -> Result<Option<Request>> {
    let mut bytes = Vec::with_capacity(4096);
    let mut chunk = [0; 8192];
    let end = loop {
        if let Some(at) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break at + 4;
        }
        if bytes.len() > MOST_HEAD {
            return Ok(None);
        }
        let count = stream.read(&mut chunk).await?;
        if count == 0 {
            return Err("closed before the request ended".into());
        }
        bytes.extend_from_slice(&chunk[..count]);
    };
    let head = String::from_utf8_lossy(&bytes[..end]).into_owned();
    let method = head.split_whitespace().next().unwrap_or_default().to_owned();
    let length = header(&head, "content-length").and_then(|value| value.parse::<usize>().ok()).unwrap_or(0);
    if length > MOST_BODY {
        return Ok(None);
    }
    let mut body = bytes[end..].to_vec();
    while body.len() < length {
        let count = stream.read(&mut chunk).await?;
        if count == 0 {
            return Err("closed before the body ended".into());
        }
        body.extend_from_slice(&chunk[..count]);
    }
    body.truncate(length);
    Ok(Some(Request { method, head, body }))
}

fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

/// The account of a valid session cookie: "<account>.<expiry>.<mac>", the
/// mac base64url (no padding) of HMAC-SHA256("<account>.<expiry>").
fn session_account(head: &str, secret: &[u8], now: u64) -> Option<u64> {
    let cookies = head
        .lines()
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .filter(|(key, _)| key.trim().eq_ignore_ascii_case("cookie"))
        .flat_map(|(_, value)| value.split(';'));
    let value = cookies
        .filter_map(|cookie| cookie.trim().split_once('='))
        .find_map(|(name, value)| (name == COOKIE).then_some(value))?;
    let mut parts = value.split('.');
    let (id, expires, mac) = (parts.next()?, parts.next()?, parts.next()?);
    let account = id.parse::<u64>().ok()?;
    if expires.parse::<u64>().ok()? < now {
        return None;
    }
    let mut signer = Hmac::<Sha256>::new_from_slice(secret).ok()?;
    signer.update(format!("{id}.{expires}").as_bytes());
    let expected = base64url(&signer.finalize().into_bytes());
    let same = expected.len() == mac.len()
        && expected.bytes().zip(mac.bytes()).fold(0, |difference, (a, b)| difference | (a ^ b)) == 0;
    same.then_some(account)
}

fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for index in 0..=chunk.len() {
            out.push(ALPHABET[(n >> (18 - 6 * index) & 63) as usize] as char);
        }
    }
    out
}

async fn respond(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> Result<()> {
    respond_with(stream, status, kind, "", body).await
}

async fn respond_with(stream: &mut TcpStream, status: &str, kind: &str, extra: &str, body: &[u8]) -> Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n{extra}Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body).await?;
    Ok(())
}

async fn serve(mut stream: TcpStream, server: &Arc<Server>) -> Result<()> {
    let Some(request) = read_request(&mut stream).await? else {
        return respond(&mut stream, "413 Payload Too Large", "text/plain", b"too large").await;
    };
    if request.method != "POST" {
        return respond(&mut stream, "405 Method Not Allowed", "text/plain", b"POST the paths").await;
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|time| time.as_secs()).unwrap_or(0);
    let Some(account) = session_account(&request.head, &server.secret, now) else {
        return respond(&mut stream, "401 Unauthorized", "text/plain", b"sign in on the site").await;
    };
    let paths: Vec<String> = String::from_utf8_lossy(&request.body)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    if paths.len() > MOST_PATHS {
        return respond(&mut stream, "413 Payload Too Large", "text/plain", b"too many paths").await;
    }
    let count = paths.len();
    let asked = paths.clone();
    let version = format!("X-Gravak-Pack: {}\r\n", server.version);
    let server = server.clone();
    let (body, missing) = tokio::task::spawn_blocking(move || answer(&server, &paths)).await?;
    if count > 0 {
        println!("account {account}: {count} files, {missing} missing, {} KiB", body.len() / 1024);
    }
    // GRAVAK_LOG_PATHS=1: every path asked, to make the browser's preload
    // list (client/web/world.txt) from a session.
    if std::env::var_os("GRAVAK_LOG_PATHS").is_some() {
        for path in &asked {
            println!("PATH {path}");
        }
    }
    respond_with(&mut stream, "200 OK", "application/octet-stream", &version, &body).await
}

/// The answer to `paths`, and how many of them the pack lacks.
fn answer(server: &Server, paths: &[String]) -> (Vec<u8>, usize) {
    let mut body = Vec::new();
    let mut missing = 0;
    let mut pack = server.pack.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    for path in paths {
        if body.len() > MOST_BYTES {
            body.extend_from_slice(&LATER.to_le_bytes());
            continue;
        }
        match read(&mut pack, path) {
            Some(bytes) if bytes.len() < LATER as usize => {
                body.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                body.extend_from_slice(&bytes);
            }
            _ => {
                missing += 1;
                body.extend_from_slice(&MISSING.to_le_bytes());
            }
        }
    }
    (body, missing)
}

/// A name first, then the id of the original archives: the client's own
/// order (sandbox/src/assets.rs, `Assets::read`).
fn read(pack: &mut PackReader, path: &str) -> Option<Vec<u8>> {
    let virtual_path = VirtualPath::try_from(path).ok()?;
    let ordinal = pack
        .find_name(virtual_path.as_str())
        .or_else(|| pack.find_id(path_id(&virtual_path).ok()?))?;
    pack.read(ordinal).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed(secret: &[u8], payload: &str) -> String {
        let mut signer = Hmac::<Sha256>::new_from_slice(secret).unwrap();
        signer.update(payload.as_bytes());
        format!("{payload}.{}", base64url(&signer.finalize().into_bytes()))
    }

    #[test]
    fn base64url_has_no_padding() {
        assert_eq!(base64url(b"f"), "Zg");
        assert_eq!(base64url(b"fo"), "Zm8");
        assert_eq!(base64url(b"foo"), "Zm9v");
        assert_eq!(base64url(&[0xfb, 0xff]), "-_8");
    }

    #[test]
    fn the_signature_is_the_sites() {
        // node -e 'require("crypto").createHmac("sha256","secret").update("42.2000").digest("base64url")'
        assert_eq!(signed(b"secret", "42.2000"), "42.2000.Wy-rBdkEa-7jjv6O0A2jLe6zpWRl0Z9h2SMCjTI3aT8");
    }

    #[test]
    fn only_a_signed_unexpired_session_passes() {
        let cookie = signed(b"secret", "42.2000");
        let head = format!("POST /game-files HTTP/1.1\r\nCookie: a=b; {COOKIE}={cookie}\r\n\r\n");
        assert_eq!(session_account(&head, b"secret", 1000), Some(42));
        assert_eq!(session_account(&head, b"secret", 3000), None, "expired");
        assert_eq!(session_account(&head, b"other", 1000), None, "another secret");
        let forged = head.replace("42.2000", "43.2000");
        assert_eq!(session_account(&forged, b"secret", 1000), None);
        assert_eq!(session_account("POST / HTTP/1.1\r\n\r\n", b"secret", 0), None);
    }
}
