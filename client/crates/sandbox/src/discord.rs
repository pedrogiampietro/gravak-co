//! Discord's Rich Presence: "Playing Gravak-Co" on the player's profile,
//! with the crest, the hero's name, level, class and map, and how long
//! they have played. It talks to the Discord app on this computer over
//! its local pipe (`discord-ipc-0`..`9`): frames of an opcode, a length
//! and JSON; a handshake with the application's id, then `SET_ACTIVITY`.
//! Without Discord running nothing happens; it is tried again later.
//!
//! The application (its name is what "Playing ..." shows) and its art
//! (`logo`, and one per class: `trojan`, `warrior`, `archer`, `taoist`)
//! live in Discord's Developer Portal.

use serde_json::{Value, json};
use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

/// The Discord application's id (Developer Portal, General Information).
/// `GRAVAK_DISCORD_APP_ID` overrides it; empty, the presence stays off.
/// Its art keys: `logo`, `trojan`, `warrior`, `archer`, `taoist`.
const APPLICATION_ID: &str = "1448499712419168288";

/// Discord takes five updates in twenty seconds; one in five is plenty.
const MIN_INTERVAL: Duration = Duration::from_secs(5);
/// How long to wait before looking for Discord again.
const RETRY: Duration = Duration::from_secs(20);

const HANDSHAKE: u32 = 0;
const FRAME: u32 = 1;

/// What the profile shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Activity {
    /// Logging in or choosing a hero.
    Menu,
    /// In the world.
    Playing { name: String, level: u8, class: u8, map: String },
}

/// The presence's thread; dropped, it clears the profile and ends.
pub struct Presence {
    updates: Sender<Activity>,
    last: Option<Activity>,
}

impl Presence {
    /// Starts the presence, or `None` without an application id.
    pub fn start() -> Option<Self> {
        let id = std::env::var("GRAVAK_DISCORD_APP_ID").unwrap_or_else(|_| APPLICATION_ID.to_owned());
        if id.trim().is_empty() {
            return None;
        }
        let (updates, received) = channel();
        let started = crate::hud::unix_now();
        std::thread::Builder::new()
            .name("discord-presence".into())
            .spawn(move || run(id.trim().to_owned(), started, received))
            .ok()?;
        Some(Self { updates, last: None })
    }

    /// Shows `activity`, when it changed.
    pub fn set(&mut self, activity: Activity) {
        if self.last.as_ref() != Some(&activity) {
            self.last = Some(activity.clone());
            let _ = self.updates.send(activity);
        }
    }
}

fn run(id: String, started: u64, updates: Receiver<Activity>) {
    let mut current: Option<Activity> = None;
    let mut pipe: Option<Pipe> = None;
    let mut sent: Option<(Activity, Instant)> = None;
    let mut next_try = Instant::now();
    loop {
        // The latest of what came in, waiting a little when nothing did.
        match updates.recv_timeout(Duration::from_secs(1)) {
            Ok(activity) => current = Some(activity),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        while let Ok(activity) = updates.try_recv() {
            current = Some(activity);
        }
        let Some(activity) = current.clone() else { continue };
        if pipe.is_none() && Instant::now() >= next_try {
            pipe = Pipe::connect(&id);
            sent = None;
            if pipe.is_none() {
                next_try = Instant::now() + RETRY;
            }
        }
        let Some(open) = pipe.as_mut() else { continue };
        let due = sent.as_ref().is_none_or(|(shown, at)| *shown != activity && at.elapsed() >= MIN_INTERVAL);
        if !due {
            continue;
        }
        if open.set_activity(&activity_json(&activity, started)).is_ok() {
            sent = Some((activity, Instant::now()));
        } else {
            // Discord closed: look for it again later.
            pipe = None;
            next_try = Instant::now() + RETRY;
        }
    }
}

/// The class's art key and name.
fn class_art(class: u8) -> (&'static str, &'static str) {
    match class {
        10..=15 => ("trojan", "Trojan"),
        20..=25 => ("warrior", "Warrior"),
        40..=45 => ("archer", "Archer"),
        100..=101 => ("taoist", "Taoist"),
        132..=135 => ("taoist", "Water Taoist"),
        142..=145 => ("taoist", "Fire Taoist"),
        _ => ("logo", "Adventurer"),
    }
}

fn activity_json(activity: &Activity, started: u64) -> Value {
    let mut assets = json!({ "large_image": "logo", "large_text": "Gravak-Co" });
    let (details, state) = match activity {
        Activity::Menu => ("In the menu".to_owned(), "Choosing a hero".to_owned()),
        Activity::Playing { name, level, class, map } => {
            let (art, class_name) = class_art(*class);
            assets["small_image"] = json!(art);
            assets["small_text"] = json!(format!("{class_name}, level {level}"));
            (format!("{name} · Lv {level} {class_name}"), map.clone())
        }
    };
    json!({
        "details": details,
        "state": state,
        "timestamps": { "start": started },
        "assets": assets,
        "instance": false,
    })
}

/// The open pipe to the Discord app.
struct Pipe {
    #[cfg(windows)]
    stream: std::fs::File,
    #[cfg(unix)]
    stream: std::os::unix::net::UnixStream,
    nonce: u64,
}

impl Pipe {
    /// The first Discord pipe that takes the handshake.
    fn connect(id: &str) -> Option<Self> {
        (0..10).find_map(|index| {
            let stream = open(index)?;
            let mut pipe = Self { stream, nonce: 0 };
            pipe.write(HANDSHAKE, &json!({ "v": 1, "client_id": id })).ok()?;
            // Discord answers READY, or closes the pipe on a bad id.
            pipe.read().ok()?;
            Some(pipe)
        })
    }

    fn set_activity(&mut self, activity: &Value) -> std::io::Result<()> {
        self.nonce += 1;
        let command = json!({
            "cmd": "SET_ACTIVITY",
            "args": { "pid": std::process::id(), "activity": activity },
            "nonce": self.nonce.to_string(),
        });
        self.write(FRAME, &command)?;
        self.read().map(|_| ())
    }

    fn write(&mut self, opcode: u32, payload: &Value) -> std::io::Result<()> {
        let body = payload.to_string().into_bytes();
        let mut frame = Vec::with_capacity(8 + body.len());
        frame.extend_from_slice(&opcode.to_le_bytes());
        frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
        frame.extend_from_slice(&body);
        self.stream.write_all(&frame)?;
        self.stream.flush()
    }

    /// One answer: its opcode and JSON.
    fn read(&mut self) -> std::io::Result<(u32, Value)> {
        let mut header = [0u8; 8];
        self.stream.read_exact(&mut header)?;
        let opcode = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        let mut body = vec![0u8; length.min(1 << 20)];
        self.stream.read_exact(&mut body)?;
        // Opcode 2 is Discord closing the pipe (a wrong id, say).
        if opcode == 2 {
            return Err(std::io::Error::other(String::from_utf8_lossy(&body).into_owned()));
        }
        Ok((opcode, serde_json::from_slice(&body).unwrap_or(Value::Null)))
    }
}

#[cfg(windows)]
fn open(index: u32) -> Option<std::fs::File> {
    std::fs::OpenOptions::new().read(true).write(true).open(format!(r"\\.\pipe\discord-ipc-{index}")).ok()
}

#[cfg(unix)]
fn open(index: u32) -> Option<std::os::unix::net::UnixStream> {
    let folder = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
        .iter()
        .find_map(|name| std::env::var(name).ok())
        .unwrap_or_else(|| "/tmp".into());
    std::os::unix::net::UnixStream::connect(format!("{folder}/discord-ipc-{index}")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_profile_shows_the_hero_and_where_they_are() {
        let activity = Activity::Playing { name: "Pedro".into(), level: 130, class: 15, map: "Twin City".into() };
        let json = activity_json(&activity, 1_700_000_000);
        assert_eq!(json["details"], "Pedro · Lv 130 Trojan");
        assert_eq!(json["state"], "Twin City");
        assert_eq!(json["assets"]["large_image"], "logo");
        assert_eq!(json["assets"]["small_image"], "trojan");
        assert_eq!(json["timestamps"]["start"], 1_700_000_000u64);
        assert_eq!(activity_json(&Activity::Menu, 0)["details"], "In the menu");
    }
}
