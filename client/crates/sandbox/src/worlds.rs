//! The worlds (game servers) offered on the login screen, and whether each
//! one answers.
//!
//! `servers.ini` beside the executable lists them, one section per world;
//! the updater keeps it current, so a new world needs no new install:
//!
//! ```ini
//! [Gravak]
//! Auth=play.example.com:9958
//! Realm=Gravak
//! ```
//!
//! `Realm` defaults to the section's name. Without the file the client
//! offers the one server named by `gravak.ini` or `--auth`.

use std::{
    net::{TcpStream, ToSocketAddrs},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

pub const FILE_NAME: &str = "servers.ini";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    /// What the login screen shows, and the key saved accounts belong to.
    pub name: String,
    /// The account server, `host:port`.
    pub auth: String,
    pub realm: String,
    /// The server's login key (`Key=`, hex): passwords are sealed to it.
    pub key: Option<[u8; 32]>,
}

impl World {
    pub fn new(name: impl Into<String>, auth: impl Into<String>, realm: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            auth: auth.into(),
            realm: realm.into(),
            key: None,
        }
    }
}

/// The worlds of `servers.ini` beside the executable, or `fallback` alone.
pub fn load(fallback: World) -> Vec<World> {
    let worlds = std::env::current_exe()
        .ok()
        .and_then(|exe| std::fs::read_to_string(exe.parent()?.join(FILE_NAME)).ok())
        .map(|text| parse(&text))
        .unwrap_or_default();
    if worlds.is_empty() { vec![fallback] } else { worlds }
}

pub fn parse(text: &str) -> Vec<World> {
    let mut worlds: Vec<World> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            let name = name.trim();
            worlds.push(World::new(name, "", name));
            continue;
        }
        let (Some(world), Some((key, value))) = (worlds.last_mut(), line.split_once('=')) else {
            continue;
        };
        let value = value.trim();
        match key.trim().to_ascii_lowercase().as_str() {
            "auth" => world.auth = value.to_owned(),
            "realm" if !value.is_empty() => world.realm = value.to_owned(),
            "key" => world.key = gravak_protocol::parse_key(value),
            _ => {}
        }
    }
    worlds.retain(|world| !world.name.is_empty() && !world.auth.is_empty());
    worlds
}

/// How a world's account server last answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Checking,
    /// It accepted a connection, after this many milliseconds.
    Online(u32),
    Offline,
}

/// Knocks on every world's account server now and then, sending
/// `(world index, status)` as each answers.
pub fn watch(worlds: &[World]) -> Receiver<(usize, Status)> {
    let (sender, receiver) = mpsc::channel();
    for (index, world) in worlds.iter().enumerate() {
        let sender = sender.clone();
        let auth = world.auth.clone();
        let _ = thread::Builder::new().name("gravak-world-status".into()).spawn(move || {
            loop {
                if sender.send((index, knock(&auth))).is_err() {
                    return;
                }
                thread::sleep(Duration::from_secs(15));
            }
        });
    }
    receiver
}

fn knock(auth: &str) -> Status {
    let Some(address) = auth.to_socket_addrs().ok().and_then(|mut addresses| addresses.next()) else {
        return Status::Offline;
    };
    let started = Instant::now();
    match TcpStream::connect_timeout(&address, Duration::from_secs(3)) {
        Ok(_) => Status::Online(started.elapsed().as_millis().min(9_999) as u32),
        Err(_) => Status::Offline,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_worlds_in_order() {
        let worlds = parse(
            "; worlds\n[Gravak]\nAuth=10.0.0.2:9958\n\n[Test Realm]\nauth = 127.0.0.1:9958\nRealm=Test\n[No address]\nRealm=X\n",
        );
        assert_eq!(
            worlds,
            vec![
                World::new("Gravak", "10.0.0.2:9958", "Gravak"),
                World::new("Test Realm", "127.0.0.1:9958", "Test"),
            ]
        );
    }
}
