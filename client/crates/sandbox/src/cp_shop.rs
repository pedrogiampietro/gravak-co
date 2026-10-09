//! Buying CPs with Pix, from the Store's Buy CPs tab. The website takes the
//! payment (Mercado Pago, `web/src/app/api/pix`): this asks it for the
//! packages, for a Pix charge for the hero, and for the charge's status
//! until the game server credits the CPs (which it says in the chat too).
//! Each request runs on its own thread; the answer comes back as a [`Reply`].

use serde::Deserialize;
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

/// The website, when `gravak.ini` names no update address to take it from.
const DEFAULT_SITE: &str = "https://gravak.fun";
static SITE: OnceLock<String> = OnceLock::new();

/// The website is the update address's scheme and host
/// (`https://gravak.fun/update` -> `https://gravak.fun`).
pub fn set_site(update_url: Option<&str>) {
    let site = update_url.and_then(origin).unwrap_or_else(|| DEFAULT_SITE.to_owned());
    let _ = SITE.set(site);
}

fn site() -> &'static str {
    SITE.get().map_or(DEFAULT_SITE, String::as_str)
}

fn origin(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let host = rest.split('/').next().filter(|host| !host.is_empty())?;
    Some(format!("{scheme}://{host}"))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpPackage {
    pub id: u32,
    pub price_cents: u32,
    pub cps: u32,
}

/// A Pix charge to pay: the copy-and-paste code and its QR image (PNG).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PixCharge {
    pub order: String,
    pub cps: u32,
    pub price_cents: u32,
    pub code: String,
    pub qr_png: Vec<u8>,
}

/// An answer from the website.
#[derive(Debug)]
pub enum Reply {
    Packages(Result<Vec<CpPackage>, String>),
    Charge(Result<PixCharge, String>),
    /// The order's status: "pending", "approved", "credited", "expired"...
    Status(Result<String, String>),
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .user_agent(concat!("Gravak/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn spawn(work: impl FnOnce() -> Reply + Send + 'static) -> Receiver<Reply> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(work());
    });
    receiver
}

/// What a refusal from the website means, for the player.
fn refusal(code: &str) -> String {
    match code {
        "noCharacter" => "The shop could not find your character.".to_owned(),
        "noPackage" => "That package is no longer on sale.".to_owned(),
        "tooMany" => "Too many Pix codes just now. Pay one, or wait a few minutes.".to_owned(),
        _ => "The shop is unavailable right now. Try again in a few minutes.".to_owned(),
    }
}

/// The body as JSON, or the website's refusal as a message.
fn read<T: for<'de> Deserialize<'de>>(response: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<T, String> {
    #[derive(Deserialize)]
    struct Refusal {
        error: Option<String>,
    }
    let mut response = response.map_err(|_| refusal("offline"))?;
    let text = response.body_mut().read_to_string().map_err(|_| refusal("offline"))?;
    if !response.status().is_success() {
        let code = serde_json::from_str::<Refusal>(&text).ok().and_then(|refusal| refusal.error).unwrap_or_default();
        return Err(refusal(&code));
    }
    serde_json::from_str(&text).map_err(|_| refusal("offline"))
}

pub fn request_packages() -> Receiver<Reply> {
    spawn(|| {
        #[derive(Deserialize)]
        struct Packages {
            packages: Vec<CpPackage>,
        }
        let packages = read::<Packages>(agent().get(format!("{}/api/pix", site())).call());
        Reply::Packages(packages.map(|answer| answer.packages))
    })
}

pub fn request_charge(character: String, package: u32) -> Receiver<Reply> {
    spawn(move || {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Charge {
            order: String,
            cps: u32,
            price_cents: u32,
            code: String,
            qr: String,
        }
        let body = serde_json::json!({ "character": character, "package": package }).to_string();
        let charge = read::<Charge>(
            agent()
                .post(format!("{}/api/pix", site()))
                .header("Content-Type", "application/json")
                .send(body),
        );
        Reply::Charge(charge.and_then(|charge| {
            Ok(PixCharge {
                qr_png: base64(&charge.qr).ok_or_else(|| refusal("offline"))?,
                order: charge.order,
                cps: charge.cps,
                price_cents: charge.price_cents,
                code: charge.code,
            })
        }))
    })
}

pub fn request_status(order: String) -> Receiver<Reply> {
    spawn(move || {
        #[derive(Deserialize)]
        struct Status {
            status: String,
        }
        let status = read::<Status>(agent().get(format!("{}/api/pix/{order}", site())).call());
        Reply::Status(status.map(|answer| answer.status))
    })
}

/// "R$ 25,00".
pub fn reais(cents: u32) -> String {
    format!("R$ {},{:02}", crate::store_panel::thousands(u64::from(cents / 100)), cents % 100)
}

/// Standard base64 (the QR image comes as such), None when it is not.
fn base64(text: &str) -> Option<Vec<u8>> {
    let value = |byte: u8| match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let digits: Vec<u8> = text.bytes().filter(|byte| !byte.is_ascii_whitespace() && *byte != b'=').collect();
    let mut out = Vec::with_capacity(digits.len() * 3 / 4);
    for chunk in digits.chunks(4) {
        let mut bits = 0u32;
        for (index, byte) in chunk.iter().enumerate() {
            bits |= u32::from(value(*byte)?) << (18 - 6 * index);
        }
        let bytes = bits.to_be_bytes();
        out.extend_from_slice(&bytes[1..chunk.len()]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_site_is_the_update_address_host() {
        assert_eq!(origin("https://gravak.fun/update").as_deref(), Some("https://gravak.fun"));
        assert_eq!(origin("http://127.0.0.1:8000").as_deref(), Some("http://127.0.0.1:8000"));
        assert_eq!(origin("gravak.fun"), None);
    }

    #[test]
    fn base64_decodes() {
        assert_eq!(base64("R3JhdmFr").as_deref(), Some(&b"Gravak"[..]));
        assert_eq!(base64("R3Jhdg==").as_deref(), Some(&b"Grav"[..]));
        assert_eq!(base64("R3Jhd2E=").as_deref(), Some(&b"Grawa"[..]));
        assert_eq!(base64("not base64!"), None);
    }

    #[test]
    fn reais_have_cents() {
        assert_eq!(reais(2500), "R$ 25,00");
        assert_eq!(reais(1999), "R$ 19,99");
        assert_eq!(reais(1_000_000), "R$ 10.000,00");
    }
}
