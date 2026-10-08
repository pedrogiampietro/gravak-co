//! Sealed logins: the client seals the password to this server's X25519 key
//! (an ephemeral key of its own, ChaCha20-Poly1305 under a key derived
//! from both), so it never travels readable. The 5065's own RC5 has a
//! published key: anyone listening could read a password sent with it.
//!
//! The key pair is made on the first start and kept beside the server
//! (`keys/login.key`, or `GRAVAK_LOGIN_KEY`); its public half, written to
//! `keys/login.pub`, goes in the players' `servers.ini` (`Key=`).

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::ChaCha20Poly1305;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::OnceLock;
use tq_network::PacketID;
use tq_serde::String16;
use x25519_dalek::{PublicKey, StaticSecret};

/// A login or a registration with the password sealed (custom packet 9010).
#[derive(Debug, Default, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 9010)]
pub struct MsgSecureLogin {
    /// 0 a login (MsgAccount), 1 a registration (MsgRegisterAccount).
    pub kind: u32,
    pub username: String16,
    pub realm: String16,
    /// The client's ephemeral public key.
    pub ephemeral: [u8; 32],
    /// The password, padded to 16 bytes, sealed: 16 bytes and the tag.
    pub sealed: [u8; 32],
}

/// The server's login key pair.
pub struct LoginKey {
    secret: StaticSecret,
    pub public: [u8; 32],
}

/// The key both sides derive from the shared secret.
fn derive(shared: &[u8; 32], ephemeral: &[u8; 32], server: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"gravak-login-v1");
    hasher.update(shared);
    hasher.update(ephemeral);
    hasher.update(server);
    hasher.finalize().into()
}

impl LoginKey {
    pub fn from_secret(secret: [u8; 32]) -> Self {
        let secret = StaticSecret::from(secret);
        let public = PublicKey::from(&secret).to_bytes();
        Self { secret, public }
    }

    /// The password sealed in `sealed`, or `None` when it does not open.
    pub fn open(&self, ephemeral: [u8; 32], sealed: [u8; 32]) -> Option<String> {
        let shared = self.secret.diffie_hellman(&PublicKey::from(ephemeral)).to_bytes();
        let key = derive(&shared, &ephemeral, &self.public);
        let cipher = ChaCha20Poly1305::new(&key.into());
        // Each ephemeral key seals once, so a fixed nonce is safe.
        let padded = cipher.decrypt(&[0u8; 12].into(), sealed.as_slice()).ok()?;
        let end = padded.iter().position(|byte| *byte == 0).unwrap_or(padded.len());
        String::from_utf8(padded[..end].to_vec()).ok()
    }

    /// The key kept beside the server, made on the first start.
    pub fn load_or_create() -> std::io::Result<Self> {
        let path = std::env::var_os("GRAVAK_LOGIN_KEY").map_or_else(|| PathBuf::from("keys/login.key"), PathBuf::from);
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(secret) = <[u8; 32]>::try_from(bytes.as_slice()) {
                return Ok(Self::from_secret(secret));
            }
        }
        let mut secret = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut secret);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, secret)?;
        let key = Self::from_secret(secret);
        std::fs::write(path.with_extension("pub"), hex(&key.public))?;
        tracing::warn!(target: "auth", path = %path.display(), "A new login key was made; give players its public key");
        Ok(key)
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The server's login key, once loaded.
pub fn key() -> Option<&'static LoginKey> {
    KEY.get()
}

static KEY: OnceLock<LoginKey> = OnceLock::new();

/// Loads (or makes) the login key; the public half goes to the log.
pub fn init() -> std::io::Result<&'static LoginKey> {
    let key = LoginKey::load_or_create()?;
    tracing::info!(target: "auth", public = %hex(&key.public), "Login key ready");
    Ok(KEY.get_or_init(|| key))
}

/// The packet a sealed login stands for, with its password opened: its id
/// and its bytes, for the usual handlers. `None` when it does not open.
pub fn unseal(key: &LoginKey, msg: &MsgSecureLogin) -> Option<(u16, bytes::Bytes)> {
    let password = key.open(msg.ephemeral, msg.sealed)?;
    let bytes = match msg.kind {
        0 => tq_serde::to_bytes(&msg_account::MsgAccount {
            username: msg.username.to_string().into(),
            password: password.into(),
            realm: msg.realm.to_string().into(),
            ..Default::default()
        })
        .ok()?,
        1 => tq_serde::to_bytes(&crate::register::MsgRegisterAccount {
            username: msg.username.to_string().into(),
            password: password.into(),
        })
        .ok()?,
        _ => return None,
    };
    let id = if msg.kind == 0 {
        msg_account::MsgAccount::PACKET_ID
    } else {
        crate::register::MsgRegisterAccount::PACKET_ID
    };
    Some((id, bytes.freeze()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the client does (`gravak_protocol::seal_password`), here to
    /// prove the two agree.
    fn seal(server: [u8; 32], password: &str, ephemeral_secret: [u8; 32]) -> ([u8; 32], [u8; 32]) {
        let ephemeral = StaticSecret::from(ephemeral_secret);
        let ephemeral_public = PublicKey::from(&ephemeral).to_bytes();
        let shared = ephemeral.diffie_hellman(&PublicKey::from(server)).to_bytes();
        let key = derive(&shared, &ephemeral_public, &server);
        let mut padded = [0u8; 16];
        padded[..password.len()].copy_from_slice(password.as_bytes());
        let sealed = ChaCha20Poly1305::new(&key.into()).encrypt(&[0u8; 12].into(), padded.as_slice()).unwrap();
        (ephemeral_public, sealed.try_into().unwrap())
    }

    #[test]
    fn a_sealed_password_opens_only_with_the_servers_key() {
        let key = LoginKey::from_secret([7; 32]);
        let (ephemeral, sealed) = seal(key.public, "hunter2", [9; 32]);
        assert_eq!(key.open(ephemeral, sealed).as_deref(), Some("hunter2"));
        let other = LoginKey::from_secret([8; 32]);
        assert_eq!(other.open(ephemeral, sealed), None, "another key cannot read it");
        let mut tampered = sealed;
        tampered[0] ^= 1;
        assert_eq!(key.open(ephemeral, tampered), None, "a changed byte is refused");
    }

    #[test]
    fn the_client_seals_what_the_server_opens() {
        // Sealed by the client's `gravak_protocol::seal_password` to the key
        // of secret [7; 32]: the two sides agree.
        let bytes = |hex: &str| -> [u8; 32] {
            let mut out = [0u8; 32];
            for (index, byte) in out.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
            }
            out
        };
        let key = LoginKey::from_secret([7; 32]);
        let ephemeral = bytes("91ed638aec717a611f96d137c3cdd80b45c55d4dabf5a2b07e26b304eebe4401");
        let sealed = bytes("7a1f7423525604f0def56c11c172020fddd94007585edae294c098563e121717");
        assert_eq!(key.open(ephemeral, sealed).as_deref(), Some("hunter2"));
    }

    #[test]
    fn a_sealed_login_becomes_the_usual_packet() {
        let key = LoginKey::from_secret([7; 32]);
        let (ephemeral, sealed) = seal(key.public, "hunter2", [3; 32]);
        let msg = MsgSecureLogin {
            kind: 0,
            username: "pedro".to_owned().into(),
            realm: "Gravak".to_owned().into(),
            ephemeral,
            sealed,
        };
        let (id, bytes) = unseal(&key, &msg).expect("it opens");
        assert_eq!(id, msg_account::MsgAccount::PACKET_ID);
        let account: msg_account::MsgAccount = tq_serde::from_bytes(&bytes).expect("it decodes");
        assert_eq!(account.username.to_string(), "pedro");
        assert_eq!(account.password.to_string(), "hunter2");
    }
}
